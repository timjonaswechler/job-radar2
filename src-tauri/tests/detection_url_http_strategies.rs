use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use job_radar_lib::{
    compile_detection_plan, execute_detection_operation, DetectionAttempt,
    DetectionProfileCompletion, DetectionProfileExecutionFailureKind,
    DetectionProfileRejectionKind, DetectionRunStatus, PhaseBrowser, PhaseCompletion,
    ProfileHttpFailureKind, RuntimeCancellation, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, SourceProfileDocument, SupportLevel,
};
use serde_json::{json, Value};

struct Cancellation(AtomicBool);
impl Cancellation {
    fn active() -> Self {
        Self(AtomicBool::new(false))
    }
    fn cancelled() -> Self {
        Self(AtomicBool::new(true))
    }
    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl RuntimeCancellation for Cancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn profile(strategies: Value, required: &[&str]) -> SourceProfileDocument {
    let mut value: Value =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    value["key"] = json!("fixture");
    value["name"] = json!("Fixture");
    value["sourceConfigSchema"] = json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": {
            "tenant": { "type": "string" },
            "startUrl": { "type": "string", "format": "uri" }
        }
    });
    value["accessPaths"].as_array_mut().unwrap().truncate(1);
    value["accessPaths"][0]["key"] = json!("api");
    value["accessPaths"][0]["name"] = json!("API");
    value["detection"] = json!({
        "recommendedAccessPathKey": "api",
        "policy": { "type": "all_required" },
        "strategies": strategies
    });
    serde_json::from_value(value).unwrap()
}

fn response(status: u16, body: impl Into<Vec<u8>>) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status,
        final_url: "https://example.test/probe".into(),
        headers: vec![],
        body: vec![ScriptedHttpBodyEvent::Chunk(body.into())],
        content_length: None,
    }
}

#[test]
fn compiler_requires_exact_all_required_url_first_and_compiles_patterns_before_io() {
    let malformed = profile(
        json!([
            { "type": "url", "key": "url", "input": {
                "type": "pattern_alternatives",
                "alternatives": [{ "pattern": "(?<other>.+)", "captures": ["tenant"] }]
            }}
        ]),
        &["tenant"],
    );
    let errors = compile_detection_plan(&malformed).unwrap_err();
    assert_eq!(errors[0].code, "invalid_detection_capture_pattern");
    assert_eq!(
        errors[0].path,
        "/detection/strategies/0/input/alternatives/0/pattern"
    );

    let mut wrong_policy = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } }
        ]),
        &["startUrl"],
    );
    wrong_policy.detection.as_mut().unwrap().policy =
        Some(job_radar_lib::StrategyPolicy::FirstAccepted);
    assert_eq!(
        compile_detection_plan(&wrong_policy).unwrap_err()[0].code,
        "invalid_detection_policy"
    );

    let invalid_key = profile(
        json!([
            { "type": "url", "key": "bad key", "input": { "type": "absolute_url" } }
        ]),
        &["startUrl"],
    );
    assert_eq!(
        compile_detection_plan(&invalid_key).unwrap_err()[0].code,
        "invalid_detection_strategy_key"
    );
}

#[test]
fn direct_serde_rejects_partial_or_mixed_final_detection_shapes() {
    let mut value: Value =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    value["detection"] = json!({
        "policy": { "type": "all_required" },
        "strategies": [
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } }
        ],
        "httpChecks": [
            { "key": "legacy", "url": "https://example.test", "timeoutMs": 1000 }
        ]
    });
    assert!(serde_json::from_value::<SourceProfileDocument>(value.clone()).is_err());

    value["detection"] = json!({ "policy": { "type": "all_required" } });
    assert!(serde_json::from_value::<SourceProfileDocument>(value.clone()).is_err());

    value["detection"] = json!({
        "policy": { "type": "all_required" },
        "strategies": [
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "url": "https://example.test", "timeoutMs": 1000
            }, "captures": [] }
        ]
    });
    assert!(serde_json::from_value::<SourceProfileDocument>(value.clone()).is_err());

    value["detection"] = json!({
        "policy": { "type": "all_required" },
        "strategies": [
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "probe", "fetch": {
                "mode": "browser", "url": "https://example.test", "timeoutMs": 1000
            } }
        ]
    });
    assert!(serde_json::from_value::<SourceProfileDocument>(value).is_err());
}

#[test]
fn later_templates_reject_capture_not_guaranteed_by_every_url_alternative() {
    let profile = profile(
        json!([
            { "type": "url", "key": "url", "input": {
                "type": "pattern_alternatives",
                "alternatives": [
                    { "pattern": "^https://a\\.test/(?<tenant>[^/]+)$", "captures": ["tenant"] },
                    { "pattern": "^https://b\\.test/(?<region>[^/]+)$", "captures": ["region"] }
                ]
            }},
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "url": "https://api.test/{{capture:tenant}}", "timeoutMs": 1000
            }}
        ]),
        &["startUrl"],
    );
    let errors = compile_detection_plan(&profile).unwrap_err();
    assert_eq!(errors[0].code, "unknown_template_key");
    assert_eq!(errors[0].path, "/detection/strategies/1/fetch/url");
}

#[tokio::test]
async fn alternatives_feed_latest_reconciled_capture_to_http_template_and_preserve_non_2xx() {
    let profile = profile(
        json!([
            { "type": "url", "key": "url", "input": {
                "type": "pattern_alternatives",
                "alternatives": [
                    { "pattern": "^https://never\\.test/(?<tenant>[^/]+)$", "captures": ["tenant"] },
                    { "pattern": "^https://example\\.test/(?<tenant>[^/]+)$", "captures": ["tenant"] }
                ]
            }},
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "method": "GET",
                "url": "https://api.example.test/{{capture:tenant}}", "timeoutMs": 1000
              },
              "expectStatus": 404,
              "contains": "known tenant",
              "regex": "tenant=(?<confirmed>[a-z]+)",
              "captures": ["confirmed"],
              "evidence": "probe accepted"
            }
        ]),
        &["tenant"],
    );
    let mut profile = profile;
    let detection = profile.detection.as_mut().unwrap();
    detection.source_config = Some(
        json!({ "tenant": "{{capture:tenant}}" })
            .as_object()
            .unwrap()
            .clone(),
    );
    detection.key_candidates = Some(vec!["{{capture:tenant}}".into()]);
    detection.name_candidates = Some(vec!["Tenant {{capture:tenant}}".into()]);
    let plan = compile_detection_plan(&profile).unwrap();
    let client =
        ScriptedProfileHttpClient::new([response(404, b"known tenant; tenant=acme".to_vec())]);
    let result = execute_detection_operation(
        "  https://example.test/acme  ",
        &[plan],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;

    assert_eq!(result.report.completion, PhaseCompletion::Accepted);
    assert_eq!(result.report.usage.response_bytes, 25);
    assert_eq!(result.run_result.status, DetectionRunStatus::Matched);
    assert_eq!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::Matched
    );
    assert_eq!(client.requests()[0].url, "https://api.example.test/acme");
    let DetectionAttempt::Matched(proposal) = &result.attempts[0] else {
        panic!("expected proposal")
    };
    assert_eq!(
        proposal.captures.get("tenant").map(String::as_str),
        Some("acme")
    );
    assert_eq!(
        proposal.captures.get("confirmed").map(String::as_str),
        Some("acme")
    );
    assert_eq!(
        proposal.provenance.captures["tenant"][0].strategy_key(),
        "url"
    );
    assert_eq!(
        proposal.provenance.captures["confirmed"][0].strategy_key(),
        "probe"
    );
    assert_eq!(proposal.source_config["tenant"], "acme");
    assert_eq!(proposal.key_candidates, ["acme"]);
    assert_eq!(proposal.name_candidates, ["Tenant acme"]);
    assert!(proposal
        .evidence
        .iter()
        .any(|item| item.kind == job_radar_lib::DetectionEvidenceKind::Http));

    let serialized = serde_json::to_value(&result).unwrap();
    assert_eq!(serialized["report"], serde_json::to_value(&result.report).unwrap());
    assert_eq!(serialized["runResult"], serde_json::to_value(&result.run_result).unwrap());
    assert_eq!(
        serde_json::from_value::<job_radar_lib::DetectionOperationResult>(serialized).unwrap(),
        result
    );
}

#[tokio::test]
async fn equal_and_conflicting_http_captures_use_d01_order_and_stop_later_io() {
    let strategies = json!([
        { "type": "url", "key": "url", "input": {
            "type": "pattern_alternatives",
            "alternatives": [
                { "pattern": "^https://example\\.test/(?<tenant>[^/]+)$", "captures": ["tenant"] }
            ]
        }},
        { "type": "http", "key": "probe", "fetch": {
            "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
          }, "regex": "tenant=(?<tenant>[a-z]+)", "captures": ["tenant"] },
        { "type": "http", "key": "later", "fetch": {
            "mode": "http", "url": "https://example.test/later", "timeoutMs": 1000
        }}
    ]);
    let profile = profile(strategies.clone(), &["tenant"]);
    let equal_client = ScriptedProfileHttpClient::new([
        response(200, b"tenant=acme".to_vec()),
        response(200, Vec::new()),
    ]);
    let equal = execute_detection_operation(
        "https://example.test/acme",
        &[compile_detection_plan(&profile).unwrap()],
        &equal_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    let proposal = &equal.run_result.proposals[0];
    let origins = &proposal.provenance.captures["tenant"];
    assert_eq!(origins.len(), 2);
    assert_eq!(origins[0].strategy_key(), "url");
    assert_eq!(origins[1].strategy_key(), "probe");
    assert_eq!(proposal.evidence.len(), 3);
    assert_eq!(equal_client.request_count(), 2);

    let conflict_client = ScriptedProfileHttpClient::new([response(200, b"tenant=other".to_vec())]);
    let conflict = execute_detection_operation(
        "https://example.test/acme",
        &[compile_detection_plan(&profile).unwrap()],
        &conflict_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(conflict.run_result.status, DetectionRunStatus::Failed);
    assert!(conflict.run_result.proposals.is_empty());
    assert_eq!(conflict_client.request_count(), 1);
    assert_eq!(
        conflict.profile_outcomes[0].completion,
        DetectionProfileCompletion::ExecutionFailed {
            strategy_key: Some("probe".into()),
            kind: DetectionProfileExecutionFailureKind::Reconciliation,
        }
    );
}

#[tokio::test]
async fn absent_expected_status_allows_non_2xx_body_acceptance_and_preserves_profile_order() {
    let strategies = json!([
        { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
        { "type": "http", "key": "probe", "fetch": {
            "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
          }, "contains": "usable"
        }
    ]);
    let mut unsupported = profile(strategies.clone(), &["startUrl"]);
    unsupported.key = "unsupported".into();
    unsupported.support.level = SupportLevel::Unsupported;
    let mut supported = profile(strategies, &["startUrl"]);
    supported.key = "supported".into();
    let client = ScriptedProfileHttpClient::new([
        response(503, b"usable response".to_vec()),
        response(503, b"usable response".to_vec()),
    ]);
    let result = execute_detection_operation(
        "https://example.test/start",
        &[
            compile_detection_plan(&unsupported).unwrap(),
            compile_detection_plan(&supported).unwrap(),
        ],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;

    assert!(
        matches!(&result.attempts[0], DetectionAttempt::Unsupported(value) if value.profile_key == "unsupported")
    );
    assert!(
        matches!(&result.attempts[1], DetectionAttempt::Matched(value) if value.profile_key == "supported")
    );
    assert_eq!(client.request_count(), 2);
    assert_eq!(result.run_result.status, DetectionRunStatus::Matched);
    assert_eq!(result.profile_outcomes.len(), 2);
    assert_eq!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::Unsupported
    );
    assert_eq!(
        result.profile_outcomes[1].completion,
        DetectionProfileCompletion::Matched
    );
}

#[tokio::test]
async fn pass_through_emits_no_synthetic_url_data_and_all_required_stops_after_rejection() {
    let profile = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "reject", "fetch": {
                "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
              }, "contains": "required" },
            { "type": "http", "key": "must_not_run", "fetch": {
                "mode": "http", "url": "https://example.test/later", "timeoutMs": 1000
              }}
        ]),
        &["startUrl"],
    );
    let plan = compile_detection_plan(&profile).unwrap();
    let client = ScriptedProfileHttpClient::new([response(503, b"bounded but rejected".to_vec())]);
    let result = execute_detection_operation(
        "https://example.test/start",
        &[plan],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;

    assert_eq!(result.report.completion, PhaseCompletion::Accepted);
    assert_eq!(client.request_count(), 1);
    let DetectionAttempt::Failed(diagnostics) = &result.attempts[0] else {
        panic!("expected failure")
    };
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "strategy_policy_all_required_unsatisfied"));
    assert!(!diagnostics.iter().any(|d| d.code == "fallback_exhausted"));
    assert_eq!(result.run_result.status, DetectionRunStatus::Failed);
    assert_eq!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::Rejected {
            strategy_key: "reject".into(),
            kind: DetectionProfileRejectionKind::Contains,
        }
    );
}

#[tokio::test]
async fn aggregation_reports_ambiguity_and_all_unsupported_without_reordering() {
    let strategies = json!([
        { "type": "url", "key": "url", "input": { "type": "absolute_url" } }
    ]);
    let mut first = profile(strategies.clone(), &["startUrl"]);
    first.key = "first".into();
    let mut second = profile(strategies.clone(), &["startUrl"]);
    second.key = "second".into();
    let empty_client = ScriptedProfileHttpClient::new([]);
    let ambiguous = execute_detection_operation(
        "https://example.test",
        &[
            compile_detection_plan(&first).unwrap(),
            compile_detection_plan(&second).unwrap(),
        ],
        &empty_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(ambiguous.run_result.status, DetectionRunStatus::Ambiguous);
    assert_eq!(ambiguous.run_result.proposals.len(), 2);
    assert_eq!(ambiguous.profile_outcomes[0].profile_key, "first");
    assert_eq!(ambiguous.profile_outcomes[1].profile_key, "second");

    first.support.level = SupportLevel::Unsupported;
    second.support.level = SupportLevel::Unsupported;
    let unsupported = execute_detection_operation(
        "https://example.test",
        &[
            compile_detection_plan(&first).unwrap(),
            compile_detection_plan(&second).unwrap(),
        ],
        &empty_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(
        unsupported.run_result.status,
        DetectionRunStatus::Unsupported
    );
    assert!(unsupported.run_result.proposals.is_empty());
    assert_eq!(unsupported.run_result.unsupported_profiles.len(), 2);
}

#[tokio::test]
async fn transport_and_decode_failures_have_typed_profile_projection() {
    let strategies = json!([
        { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
        { "type": "http", "key": "probe", "fetch": {
            "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
        }}
    ]);
    let mut transport = profile(strategies.clone(), &["startUrl"]);
    transport.key = "transport".into();
    let mut decode = profile(strategies, &["startUrl"]);
    decode.key = "decode".into();
    let client = ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test".into(),
            headers: vec![],
            body: vec![ScriptedHttpBodyEvent::Failure(
                ProfileHttpFailureKind::BodyStream,
            )],
            content_length: None,
        },
        response(200, vec![0xff]),
    ]);
    let result = execute_detection_operation(
        "https://example.test",
        &[
            compile_detection_plan(&transport).unwrap(),
            compile_detection_plan(&decode).unwrap(),
        ],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(result.run_result.status, DetectionRunStatus::Failed);
    assert!(matches!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::ExecutionFailed {
            kind: DetectionProfileExecutionFailureKind::Acquisition(
                ProfileHttpFailureKind::BodyStream
            ),
            ..
        }
    ));
    assert!(matches!(
        result.profile_outcomes[1].completion,
        DetectionProfileCompletion::ExecutionFailed {
            kind: DetectionProfileExecutionFailureKind::Acquisition(
                ProfileHttpFailureKind::MalformedText
            ),
            ..
        }
    ));
}

#[tokio::test]
async fn mid_stream_cancellation_is_operation_global_and_payload_free() {
    let profile = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
            }}
        ]),
        &["startUrl"],
    );
    let plan = compile_detection_plan(&profile).unwrap();
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test".into(),
            headers: vec![],
            body: vec![
                ScriptedHttpBodyEvent::Chunk(b"prefix".to_vec()),
                ScriptedHttpBodyEvent::Gate("body".into()),
            ],
            content_length: None,
        },
    ]));
    let cancellation = Arc::new(Cancellation::active());
    let task_client = Arc::clone(&client);
    let task_cancellation = Arc::clone(&cancellation);
    let task = tokio::spawn(async move {
        execute_detection_operation(
            "https://example.test",
            &[plan],
            task_client.as_ref(),
            PhaseBrowser::BrowserFree,
            task_cancellation.as_ref(),
        )
        .await
    });
    while !client.gate_is_waiting("body") {
        tokio::task::yield_now().await;
    }
    cancellation.cancel();
    let result = task.await.unwrap();
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Cancelled { .. }
    ));
    assert_eq!(result.report.usage.response_bytes, 6);
    assert!(result.attempts.is_empty());
    assert!(result.profile_outcomes.is_empty());
    assert!(result.run_result.proposals.is_empty());
    assert!(result.run_result.unsupported_profiles.is_empty());
    assert_eq!(result.run_result.status, DetectionRunStatus::Cancelled);
}

#[tokio::test]
async fn one_cumulative_64_mib_allowance_accepts_exact_boundary_and_blocks_later_work() {
    const LIMIT: usize = 67_108_864;
    let exact_profile = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
            }}
        ]),
        &["startUrl"],
    );
    let exact_client = ScriptedProfileHttpClient::new([response(200, vec![b'x'; LIMIT])]);
    let exact = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&exact_profile).unwrap()],
        &exact_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(exact.report.completion, PhaseCompletion::Accepted);
    assert_eq!(exact.report.usage.response_bytes, LIMIT as u64);

    let over_profile = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "first", "fetch": {
                "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
            }},
            { "type": "http", "key": "over", "fetch": {
                "mode": "http", "url": "https://example.test/over", "timeoutMs": 1000
            }}
        ]),
        &["startUrl"],
    );
    let over_client = ScriptedProfileHttpClient::new([
        response(200, vec![b'x'; LIMIT]),
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/over".into(),
            headers: vec![],
            body: vec![ScriptedHttpBodyEvent::Chunk(vec![b'y'])],
            content_length: Some(1),
        },
    ]);
    let over = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&over_profile).unwrap()],
        &over_client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert!(matches!(
        over.report.completion,
        PhaseCompletion::BudgetExhausted { .. }
    ));
    assert_eq!(over.report.usage.response_bytes, LIMIT as u64);
    assert!(over.attempts.is_empty());
    assert!(over.profile_outcomes.is_empty());
    assert!(over.run_result.proposals.is_empty());
    assert!(over.run_result.unsupported_profiles.is_empty());
    assert_eq!(over.run_result.status, DetectionRunStatus::BudgetExhausted);
    assert_eq!(over_client.request_count(), 2);
}

#[tokio::test]
async fn invalid_input_and_cancellation_start_no_http_work() {
    let profile = profile(
        json!([
            { "type": "url", "key": "url", "input": { "type": "absolute_url" } },
            { "type": "http", "key": "probe", "fetch": {
                "mode": "http", "url": "{{inputUrl}}", "timeoutMs": 1000
            }}
        ]),
        &["startUrl"],
    );
    let plan = compile_detection_plan(&profile).unwrap();
    let client = ScriptedProfileHttpClient::new([]);
    let invalid = execute_detection_operation(
        "relative",
        &[plan.clone()],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::active(),
    )
    .await;
    assert_eq!(invalid.report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(client.request_count(), 0);

    let cancelled = execute_detection_operation(
        "https://example.test",
        &[plan],
        &client,
        PhaseBrowser::BrowserFree,
        &Cancellation::cancelled(),
    )
    .await;
    assert!(matches!(
        cancelled.report.completion,
        PhaseCompletion::Cancelled { .. }
    ));
    assert_eq!(cancelled.run_result.status, DetectionRunStatus::Cancelled);
    assert!(cancelled.profile_outcomes.is_empty());
    assert!(cancelled.run_result.proposals.is_empty());
    assert_eq!(client.request_count(), 0);
}

#[test]
fn built_in_profiles_compile_only_the_final_detection_strategy_shape() {
    for (key, document) in [
        ("greenhouse", include_str!("../resources/profiles/greenhouse.json")),
        ("workday", include_str!("../resources/profiles/workday.json")),
        ("successfactors", include_str!("../resources/profiles/successfactors.json")),
    ] {
        let profile: SourceProfileDocument = serde_json::from_str(document)
            .unwrap_or_else(|error| panic!("{key} final Detection document must deserialize: {error}"));
        compile_detection_plan(&profile)
            .unwrap_or_else(|diagnostics| panic!("{key} final Detection plan must compile: {diagnostics:?}"));
    }
}
