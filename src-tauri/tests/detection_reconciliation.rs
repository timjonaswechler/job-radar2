use job_radar_lib::{
    aggregate_detection_attempts, DetectionAttempt, DetectionConfigContribution,
    DetectionContribution, DetectionEvidenceContribution, DetectionOrigin, DetectionProfileContext,
    DetectionReconciliationError, DetectionRunStatus, DetectionStateConflictKind,
    ReconciledDetectionState, SourceProfileDocument,
};
use serde_json::{json, Map, Value};

fn profile() -> SourceProfileDocument {
    let mut profile: SourceProfileDocument =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    profile.key = "example".into();
    profile.name = "Example".into();
    profile.access_paths.truncate(1);
    profile.access_paths[0].key = "api".into();
    profile.access_paths[0].name = "API".into();
    profile.source_config_schema = Some(
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tenant": { "type": "string" },
                "startUrl": { "type": "string", "format": "uri" },
                "api": { "type": "object" },
                "nullable": { "type": "null" }
            },
            "required": ["tenant", "startUrl"]
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    profile.access_paths[0].source_config_schema = None;
    let detection = profile.detection.as_mut().unwrap();
    detection.recommended_access_path_key = Some("api".into());
    detection.evidence = None;
    profile
}

fn origin(strategy: &str, path: &str) -> DetectionOrigin {
    DetectionOrigin::new(strategy, path).unwrap()
}

fn contribution(origin: DetectionOrigin) -> DetectionContribution {
    DetectionContribution::new(origin)
}

#[test]
fn reducer_unions_equal_values_and_rejects_conflicts_without_leaking_values() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let state = context.initial_state();
    let state = context
        .apply(
            &state,
            contribution(origin("url", "/detection/strategies/0"))
                .with_capture("tenant", "secret-sentinel")
                .with_recommendation("api")
                .with_config(
                    DetectionConfigContribution::new("/tenant", json!("secret-sentinel")).unwrap(),
                ),
        )
        .unwrap();
    let state = context
        .apply(
            &state,
            contribution(origin("http", "/detection/strategies/1"))
                .with_capture("tenant", "secret-sentinel")
                .with_recommendation("api")
                .with_config(
                    DetectionConfigContribution::new("/tenant", json!("secret-sentinel")).unwrap(),
                ),
        )
        .unwrap();

    assert_eq!(state.captures()[0].origins().len(), 2);
    assert_eq!(state.source_config()[0].origins().len(), 2);
    assert_eq!(state.recommendation().unwrap().origins().len(), 2);

    let error = context
        .apply(
            &state,
            contribution(origin("browser", "/detection/strategies/2"))
                .with_capture("tenant", "other-private-value"),
        )
        .unwrap_err();
    let serialized = serde_json::to_string(&error.diagnostics()).unwrap();
    assert!(matches!(
        error,
        DetectionReconciliationError::Conflict(ref conflict)
            if conflict.kind() == DetectionStateConflictKind::Capture
    ));
    assert!(!serialized.contains("secret-sentinel"));
    assert!(!serialized.contains("other-private-value"));
}

#[test]
fn canonical_atomic_pointers_preserve_present_empty_values_and_conflict_on_overlap() {
    assert!(DetectionConfigContribution::new("", json!(1)).is_err());
    assert!(DetectionConfigContribution::new("tenant", json!(1)).is_err());
    assert!(DetectionConfigContribution::new("/bad~2escape", json!(1)).is_err());

    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("first", "/detection/strategies/0"))
                .with_capture("empty", "")
                .with_config(DetectionConfigContribution::new("/api", json!({})).unwrap())
                .with_config(DetectionConfigContribution::new("/nullable", Value::Null).unwrap()),
        )
        .unwrap();
    assert_eq!(state.captures()[0].value(), "");
    assert_eq!(state.source_config()[1].value(), &Value::Null);

    for pointer in ["/api/base", "/api/base/url"] {
        let error = context
            .apply(
                &state,
                contribution(origin("overlap", "/detection/strategies/1")).with_config(
                    DetectionConfigContribution::new(pointer, json!("private")).unwrap(),
                ),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            DetectionReconciliationError::Conflict(ref conflict)
                if conflict.kind() == DetectionStateConflictKind::SourceConfigOverlap
        ));
    }

    let descendant_first = context
        .apply(
            &context.initial_state(),
            contribution(origin("descendant", "/detection/strategies/0")).with_config(
                DetectionConfigContribution::new("/api/base", json!("value")).unwrap(),
            ),
        )
        .unwrap();
    let reverse = context
        .apply(
            &descendant_first,
            contribution(origin("ancestor", "/detection/strategies/1")).with_config(
                DetectionConfigContribution::new("/api", json!({ "base": "value" })).unwrap(),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        reverse,
        DetectionReconciliationError::Conflict(ref conflict)
            if conflict.kind() == DetectionStateConflictKind::SourceConfigOverlap
    ));

    let unequal_exact = context
        .apply(
            &state,
            contribution(origin("exact", "/detection/strategies/3")).with_config(
                DetectionConfigContribution::new("/nullable", json!("not-null")).unwrap(),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        unequal_exact,
        DetectionReconciliationError::Conflict(ref conflict)
            if conflict.kind() == DetectionStateConflictKind::SourceConfigValue
    ));
}

#[test]
fn evidence_identity_uses_kind_and_descriptor_path_and_keeps_origin_order() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let first = DetectionEvidenceContribution::new(
        job_radar_lib::DetectionEvidenceKind::Http,
        "/detection/strategies/0/evidence",
        "same message",
    )
    .unwrap();
    let second = DetectionEvidenceContribution::new(
        job_radar_lib::DetectionEvidenceKind::Http,
        "/detection/strategies/1/evidence",
        "same message",
    )
    .unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("one", "/detection/strategies/0")).with_evidence(first.clone()),
        )
        .unwrap();
    let state = context
        .apply(
            &state,
            contribution(origin("two", "/detection/strategies/1"))
                .with_evidence(first)
                .with_evidence(second),
        )
        .unwrap();
    assert_eq!(state.evidence().len(), 2);
    assert_eq!(state.evidence()[0].origins().len(), 2);
    assert_eq!(state.evidence()[1].message(), "same message");
}

#[test]
fn incremental_validation_defers_required_values_but_rejects_available_invalid_values() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("url", "/detection/strategies/0"))
                .with_config(DetectionConfigContribution::new("/tenant", json!("acme")).unwrap()),
        )
        .unwrap();
    assert!(
        context.complete(&state).is_err(),
        "startUrl is required finally"
    );

    let error = context
        .apply(
            &state,
            contribution(origin("http", "/detection/strategies/1")).with_config(
                DetectionConfigContribution::new("/startUrl", json!("not a url")).unwrap(),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        DetectionReconciliationError::InvalidState(_)
    ));
}

#[test]
fn proposal_preparation_uses_one_reducer_and_serializes_complete_provenance() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("url", "/detection/strategies/0")).with_capture("tenant", "acme"),
        )
        .unwrap();
    let mut template = Map::new();
    template.insert("nullable".into(), Value::Null);
    let output = context
        .prepare_proposal(
            &state,
            "HTTPS://EXAMPLE.COM/jobs",
            Some(template),
            vec!["acme".into()],
            vec!["Acme".into()],
        )
        .unwrap();
    let proposal = output.proposal().unwrap();
    assert_eq!(
        proposal.source_config,
        json!({
            "nullable": null,
            "tenant": "acme",
            "startUrl": "https://example.com/jobs"
        })
    );
    assert_eq!(proposal.recommended_access_path_key, "api");
    assert_eq!(proposal.provenance.source_config.len(), 3);
    assert_eq!(
        proposal.provenance.evidence,
        Vec::<Vec<DetectionOrigin>>::new()
    );
    assert_eq!(
        serde_json::to_value(&proposal.provenance).unwrap(),
        json!({
            "captures": {"tenant": [{"strategyKey":"url","schemaPath":"/detection/strategies/0"}]},
            "sourceConfig": {
                "/nullable": [{"strategyKey":"proposal_preparation","schemaPath":"/detection/sourceConfig/nullable"}],
                "/startUrl": [{"strategyKey":"proposal_preparation","schemaPath":"/inputUrl"}],
                "/tenant": [
                    {"strategyKey":"url","schemaPath":"/detection/strategies/0"},
                    {"strategyKey":"proposal_preparation","schemaPath":"/sourceConfigSchema/properties/tenant"}
                ]
            },
            "recommendation": [{"strategyKey":"proposal_preparation","schemaPath":"/detection/recommendedAccessPathKey"}],
            "evidence": []
        })
    );
}

#[test]
fn independent_profile_conflict_does_not_hide_matches_but_control_terminals_do() {
    let proposal = DetectionProfileContext::compile(&profile())
        .unwrap()
        .prepare_proposal(
            &DetectionProfileContext::compile(&profile())
                .unwrap()
                .initial_state(),
            "https://example.com/jobs",
            Some({
                let mut values = Map::new();
                values.insert("tenant".into(), json!("acme"));
                values
            }),
            vec![],
            vec![],
        )
        .unwrap()
        .proposal()
        .unwrap()
        .clone();
    let failed = aggregate_detection_attempts(vec![
        DetectionAttempt::Failed(vec![]),
        DetectionAttempt::Matched(proposal.clone()),
    ]);
    assert_eq!(failed.status, DetectionRunStatus::Matched);
    assert_eq!(failed.proposals, vec![proposal.clone()]);

    let mut unsupported_profile = profile();
    unsupported_profile.support.level = job_radar_lib::SupportLevel::Unsupported;
    let unsupported_context = DetectionProfileContext::compile(&unsupported_profile).unwrap();
    let unsupported = match unsupported_context
        .prepare_proposal(
            &unsupported_context.initial_state(),
            "https://example.com/jobs",
            Some({
                let mut values = Map::new();
                values.insert("tenant".into(), json!("private-capture"));
                values
            }),
            vec![],
            vec![],
        )
        .unwrap()
    {
        job_radar_lib::PreparedDetectionOutput::Unsupported(unsupported) => unsupported,
        other => panic!("expected unsupported output, got {other:?}"),
    };

    let cancelled = aggregate_detection_attempts(vec![
        DetectionAttempt::Matched(proposal.clone()),
        DetectionAttempt::Unsupported(unsupported.clone()),
        DetectionAttempt::Cancelled(vec![]),
    ]);
    assert_eq!(cancelled.status, DetectionRunStatus::Cancelled);
    assert!(cancelled.proposals.is_empty());
    assert!(cancelled.unsupported_profiles.is_empty());

    let exhausted = aggregate_detection_attempts(vec![
        DetectionAttempt::Matched(proposal),
        DetectionAttempt::Unsupported(unsupported),
        DetectionAttempt::BudgetExhausted(vec![]),
    ]);
    assert_eq!(exhausted.status, DetectionRunStatus::BudgetExhausted);
    assert!(exhausted.proposals.is_empty());
    assert!(exhausted.unsupported_profiles.is_empty());
}

#[test]
fn selected_access_path_contract_validates_every_available_value_incrementally() {
    let mut profile = profile();
    profile.access_paths[0].source_config_schema = Some(
        json!({
            "type": "object",
            "properties": {
                "pathOnly": { "type": "string", "pattern": "^valid$" }
            },
            "required": ["pathOnly"]
        })
        .as_object()
        .unwrap()
        .clone(),
    );
    let context = DetectionProfileContext::compile(&profile).unwrap();
    let invalid = context
        .apply(
            &context.initial_state(),
            contribution(origin("url", "/detection/strategies/0")).with_config(
                DetectionConfigContribution::new("/pathOnly", json!("invalid")).unwrap(),
            ),
        )
        .unwrap_err();
    assert!(matches!(
        invalid,
        DetectionReconciliationError::InvalidState(_)
    ));

    let valid = context
        .apply(
            &context.initial_state(),
            contribution(origin("url", "/detection/strategies/0")).with_config(
                DetectionConfigContribution::new("/pathOnly", json!("valid")).unwrap(),
            ),
        )
        .unwrap();
    assert!(
        context.complete(&valid).is_err(),
        "profile requirements remain final"
    );
}

#[test]
fn authored_recommendation_conflicts_with_a_strategy_recommendation() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("strategy", "/detection/strategies/0"))
                .with_recommendation("other")
                .with_capture("tenant", "acme"),
        )
        .unwrap_err();
    assert!(matches!(
        state,
        DetectionReconciliationError::InvalidState(_)
    ));

    let mut multiple = profile();
    let mut other = multiple.access_paths[0].clone();
    other.key = "other".into();
    other.name = "Other".into();
    multiple.access_paths.push(other);
    let context = DetectionProfileContext::compile(&multiple).unwrap();
    let state = context
        .apply(
            &context.initial_state(),
            contribution(origin("strategy", "/detection/strategies/0"))
                .with_recommendation("other")
                .with_capture("tenant", "acme"),
        )
        .unwrap();
    let error = context
        .prepare_proposal(&state, "https://example.com/jobs", None, vec![], vec![])
        .unwrap_err();
    assert!(matches!(
        error,
        DetectionReconciliationError::Conflict(ref conflict)
            if conflict.kind() == DetectionStateConflictKind::Recommendation
    ));
}

#[test]
fn profile_evidence_is_seeded_once_and_strategy_recommendations_select_a_path() {
    let mut profile = profile();
    profile.detection.as_mut().unwrap().evidence = Some(vec![job_radar_lib::DetectionEvidence {
        kind: job_radar_lib::DetectionEvidenceKind::Url,
        message: "profile metadata".into(),
        path: None,
    }]);
    let mut second = profile.access_paths[0].clone();
    second.key = "browser".into();
    second.name = "Browser".into();
    profile.access_paths.push(second);
    profile
        .detection
        .as_mut()
        .unwrap()
        .recommended_access_path_key = None;

    let context = DetectionProfileContext::compile(&profile).unwrap();
    let state = context.initial_state();
    assert_eq!(state.evidence().len(), 1);
    let state = context
        .apply(
            &state,
            contribution(origin("browser", "/detection/strategies/1"))
                .with_recommendation("browser")
                .with_capture("tenant", "acme"),
        )
        .unwrap();
    let output = context
        .prepare_proposal(&state, "https://example.com/jobs", None, vec![], vec![])
        .unwrap();
    let proposal = output.proposal().unwrap();
    assert_eq!(proposal.recommended_access_path_key, "browser");
    assert_eq!(proposal.evidence.len(), 1);
    assert_eq!(proposal.provenance.evidence.len(), 1);
}

#[test]
fn immutable_snapshots_leave_the_previous_state_unchanged() {
    let context = DetectionProfileContext::compile(&profile()).unwrap();
    let before = ReconciledDetectionState::default();
    let after = context
        .apply(
            &before,
            contribution(origin("url", "/detection/strategies/0")).with_capture("tenant", "acme"),
        )
        .unwrap();
    assert!(before.captures().is_empty());
    assert_eq!(after.captures().len(), 1);
}
