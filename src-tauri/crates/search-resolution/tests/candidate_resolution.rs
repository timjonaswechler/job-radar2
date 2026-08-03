use search_resolution::{
    resolve_source_candidates, CompiledSearchRequirements, RequirementsCompilationFailure,
    ResolutionCeilings, ResolutionCompletion, ResolutionFailure, ResolutionLimitDimension,
    ScriptedDiscoveryBatch, ScriptedDiscoveryOutcome, ScriptedSourceDiscoveryExecution, SearchRule,
    SearchRuleKind, SearchRuleTarget, SourceDiscovery, SourceResolution, SourceResolutionError,
    SourceResolutionRequest,
};
use serde_json::json;
use source_engine::test_support::{
    compile_source, AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource,
    CandidateDetailFailure, CompileSourceOutcome, CompiledSource, DetailField, DetailPatch,
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, DiscoveryHint, HintUse,
    PhaseCancellationReason, PhaseCancelled, PhaseCompletion, PhaseExecutionFailure,
    PhaseExecutionReport, PhaseLimits, PhaseUsage, PostingOccurrence, PostingOccurrenceIdentity,
    PostingReference, ProfileCompilerInput, ProviderValues, RequestedDetailFields,
    RequestedFieldDisposition, RuntimeCancellation, ScriptedBrowserAcquisition,
    ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
    ScriptedSourceDetailExecution, SourceBehavior, SourceDetailFailure, SourceDetailOutcome,
    SourceDetailPhaseEvidence, SourceDetailRequestSnapshot, SourceProfileDocument,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct NeverCancelled;
impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}
struct Cancelled;
impl RuntimeCancellation for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

struct SamePlace;
impl geo::GeoResolver for SamePlace {
    fn resolve<'a>(&'a self, input: &'a str) -> geo::GeoResolveFuture<'a> {
        Box::pin(async move {
            Ok(vec![geo::ResolvedLocation {
                input: input.into(),
                label: input.into(),
                point: geo::GeoPoint {
                    latitude: 52.52,
                    longitude: 13.405,
                },
            }])
        })
    }
}

struct FailingGeo(&'static str);
impl geo::GeoResolver for FailingGeo {
    fn resolve<'a>(&'a self, _input: &'a str) -> geo::GeoResolveFuture<'a> {
        Box::pin(async move { Err(self.0.to_string()) })
    }
}

struct CancelOnCheck {
    after: usize,
    checks: AtomicUsize,
}
impl RuntimeCancellation for CancelOnCheck {
    fn is_cancelled(&self) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst) >= self.after
    }
}

fn compiled_source() -> CompiledSource {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "fixture_profile", "name": "Fixture", "kind": "generic",
        "support": { "level": "experimental", "summary": "Candidate Resolution fixture" },
        "sourceConfigSchema": { "type": "object", "required": ["feedUrl"], "properties": { "feedUrl": { "type": "string" } }, "additionalProperties": false },
        "accessPaths": [{
            "key": "default", "name": "Default",
            "discovery": { "policy": { "type": "first_accepted" }, "strategies": [{
                "key": "discovery", "fetch": { "mode": "http", "method": "GET", "url": "{{sourceConfig:feedUrl}}", "timeoutMs": 10000 },
                "parse": { "type": "json" }, "select": { "type": "json_path", "jsonPath": "$.jobs" },
                "extract": { "reference": { "url": { "type": "json_path", "jsonPath": "$.url" } }, "providerValues": { "title": { "type": "json_path", "jsonPath": "$.title" }, "company": { "type": "json_path", "jsonPath": "$.company" } } }
            }] },
            "detail": { "policy": { "type": "first_accepted" }, "strategies": [{
                "key": "detail", "fetch": { "mode": "http", "method": "GET", "url": "https://example.test/detail", "timeoutMs": 10000 },
                "parse": { "type": "json" }, "select": { "type": "document" },
                "extract": { "fields": {
                    "title": { "type": "json_path", "jsonPath": "$.title" },
                    "company": { "type": "json_path", "jsonPath": "$.company" },
                    "locations": { "type": "json_path", "jsonPath": "$.locations" }
                } }
            }] }
        }]
    })).unwrap();
    let source: SourceBehavior = serde_json::from_value(json!({
        "key": "fixture_source", "name": "Fixture source",
        "sourceConfig": { "feedUrl": "https://example.test/feed" },
        "selectedAccessPath": { "type": "profile_access_path", "profileKey": "fixture_profile", "pathKey": "default" }
    })).unwrap();
    match compile_source(&source, &ProfileCompilerInput::new(&[profile])) {
        CompileSourceOutcome::Compiled { source, .. } => source,
        other => panic!("fixture did not compile: {other:?}"),
    }
}

fn occurrence(id: &str, title: Option<&str>, company: Option<&str>) -> PostingOccurrence {
    PostingOccurrence {
        identity: PostingOccurrenceIdentity::ProviderPostingId {
            source_key: "fixture_source".into(),
            provider_posting_id: id.into(),
        },
        reference: PostingReference {
            provider_url: format!("https://example.test/jobs/{id}"),
            provider_posting_id: Some(id.into()),
        },
        provider_values: ProviderValues {
            title: title.map(Into::into),
            company: company.map(Into::into),
            locations: vec!["  Berlin  ".into()],
            description_text: None,
        },
        hints: Default::default(),
        posting_meta: [("secret".into(), "must-not-escape".into())].into(),
    }
}

fn hinted_occurrence(
    id: &str,
    title: Option<&str>,
    hint_use: Option<HintUse>,
) -> PostingOccurrence {
    let mut occurrence = occurrence(id, title, Some("ACME"));
    occurrence.hints.insert(
        "title".into(),
        DiscoveryHint {
            value: "Accountant".into(),
            hint_use,
        },
    );
    occurrence
}

fn report(requests: u64) -> PhaseExecutionReport {
    PhaseExecutionReport {
        usage: PhaseUsage {
            requests,
            response_bytes: requests * 10,
            ..Default::default()
        },
        completion: PhaseCompletion::Accepted,
    }
}
fn candidate_failure_report() -> PhaseExecutionReport {
    PhaseExecutionReport {
        usage: PhaseUsage {
            requests: 1,
            response_bytes: 10,
            ..Default::default()
        },
        completion: PhaseCompletion::PolicyUnsatisfied,
    }
}
fn terminal_report(completion: PhaseCompletion) -> PhaseExecutionReport {
    PhaseExecutionReport {
        usage: PhaseUsage {
            requests: 1,
            ..Default::default()
        },
        completion,
    }
}
fn budget_report() -> PhaseExecutionReport {
    terminal_report(PhaseCompletion::BudgetExhausted {
        exhaustion: AllowanceExhaustion {
            dimension: AllowanceDimension::Requests,
            requested: 1,
            remaining: 0,
            limit_sources: vec![AllowanceLimitSource::Caller],
        },
    })
}
fn ceilings() -> ResolutionCeilings {
    ResolutionCeilings {
        max_batch_size: 100,
        max_discovery_batches: 10,
        max_discovered_items: 100,
        max_detail_candidates: 100,
        phase: PhaseLimits::BACKEND,
    }
}
fn rule(kind: SearchRuleKind, value: &str) -> SearchRule {
    SearchRule {
        target: SearchRuleTarget::Title,
        kind,
        value: value.into(),
    }
}

fn requirements() -> CompiledSearchRequirements<'static> {
    rule_requirements(&[rule(SearchRuleKind::Text, "engineer")], &[])
}

fn rule_requirements(
    include: &[SearchRule],
    exclude: &[SearchRule],
) -> CompiledSearchRequirements<'static> {
    CompiledSearchRequirements::compile(include, exclude, &[], None).unwrap()
}
fn discovery_limits(requests_used: u64, maximum: u64) -> PhaseLimits {
    PhaseLimits {
        max_requests: PhaseLimits::BACKEND.max_requests - requests_used,
        max_response_bytes: PhaseLimits::BACKEND.max_response_bytes - requests_used * 10,
        max_produced_items: maximum,
        ..PhaseLimits::BACKEND
    }
}

fn batch(
    occurrences: Vec<PostingOccurrence>,
    exhausted: bool,
    remaining: Option<u64>,
    continuation: Option<&str>,
) -> ScriptedDiscoveryBatch {
    expected_batch(
        occurrences,
        exhausted,
        remaining,
        continuation,
        None,
        0,
        100,
    )
}

fn successful_detail(
    fields: DetailPatch,
    produced: impl IntoIterator<Item = DetailField>,
) -> SourceDetailOutcome {
    SourceDetailOutcome::Completed {
        fields,
        dispositions: produced
            .into_iter()
            .map(|field| RequestedFieldDisposition::Produced { field })
            .collect(),
        phase_evidence: Some(SourceDetailPhaseEvidence {
            complete_budget_report: report(1),
            diagnostics: vec![],
        }),
    }
}

async fn resolve_fixture_result(
    requirements: &CompiledSearchRequirements<'_>,
    occurrences: Vec<PostingOccurrence>,
    detail: &ScriptedSourceDetailExecution,
) -> Result<SourceResolution, SourceResolutionError> {
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(occurrences, true, Some(0), None)],
    );
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements,
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail,
    })
    .await;
    discovery.assert_finished();
    result
}

async fn resolve_fixture(
    requirements: &CompiledSearchRequirements<'_>,
    occurrences: Vec<PostingOccurrence>,
    detail: &ScriptedSourceDetailExecution,
) -> SourceResolution {
    resolve_fixture_result(requirements, occurrences, detail)
        .await
        .unwrap()
}

fn expected_batch(
    occurrences: Vec<PostingOccurrence>,
    exhausted: bool,
    remaining: Option<u64>,
    continuation: Option<&str>,
    expected_continuation: Option<&str>,
    requests_used: u64,
    maximum: u64,
) -> ScriptedDiscoveryBatch {
    ScriptedDiscoveryBatch {
        expected_continuation: expected_continuation.map(Into::into),
        expected_maximum: maximum,
        expected_limits: discovery_limits(requests_used, maximum),
        occurrences,
        exhausted,
        remaining,
        continuation: continuation.map(Into::into),
        continuation_source_key: None,
        complete_budget_report: report(1),
        diagnostics: vec![],
    }
}

#[tokio::test]
async fn resolves_normalized_final_only_values_and_exact_counts() {
    let detail = ScriptedSourceDetailExecution::new([]);
    let result = resolve_fixture(
        &requirements(),
        vec![occurrence(
            "1",
            Some("  Software   Engineer "),
            Some(" ACME "),
        )],
        &detail,
    )
    .await;
    assert_eq!(result.completion, ResolutionCompletion::Complete);
    assert_eq!((result.counts.discovered, result.counts.finalized), (1, 1));
    assert_eq!(result.remaining, Some(0));
    assert_eq!(result.finalized[0].title(), "Software Engineer");
    assert_eq!(result.finalized[0].company(), "ACME");
    assert_eq!(result.finalized[0].locations(), vec!["Berlin"]);
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("must-not-escape"));
    assert!(!serialized.contains("postingMeta"));
}

#[tokio::test]
async fn canonical_title_rejections_avoid_detail_and_preserve_rule_semantics() {
    let cases = [
        (
            "include-text-nonmatch",
            "Accountant",
            rule_requirements(&[rule(SearchRuleKind::Text, "engineer")], &[]),
        ),
        (
            "exclusion-text-match",
            "Data Engineer",
            rule_requirements(
                &[rule(SearchRuleKind::Text, "engineer")],
                &[rule(SearchRuleKind::Text, "data")],
            ),
        ),
        (
            "include-regex-is-case-sensitive",
            "engineer",
            rule_requirements(&[rule(SearchRuleKind::Regex, "^Engineer$")], &[]),
        ),
        (
            "exclusion-regex-is-case-insensitive",
            "Engineer",
            rule_requirements(
                &[rule(SearchRuleKind::Text, "engineer")],
                &[rule(SearchRuleKind::Regex, "^engineer$")],
            ),
        ),
    ];

    for (id, title, requirements) in cases {
        let detail = ScriptedSourceDetailExecution::new([]);
        let result = resolve_fixture(
            &requirements,
            vec![occurrence(id, Some(title), None)],
            &detail,
        )
        .await;

        assert_eq!(result.counts.rejected, 1, "{id}");
        assert_eq!(result.report.usage.requests, 1, "{id}");
    }
}

#[tokio::test]
async fn canonical_title_wins_while_hints_remain_rejection_only() {
    let candidates = vec![
        hinted_occurrence(
            "provider-wins",
            Some("Engineer"),
            Some(HintUse::SearchPrefilter),
        ),
        hinted_occurrence("authorized-rejects", None, Some(HintUse::SearchPrefilter)),
        hinted_occurrence("inert-needs-title", None, None),
        occurrence("matching-needs-company", Some("Engineer"), None),
    ];
    let detail = ScriptedSourceDetailExecution::new([
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                candidates[2].identity.clone(),
                RequestedDetailFields::new([DetailField::Title]).unwrap(),
            ),
            Ok(successful_detail(
                DetailPatch {
                    title: Some("Engineer".into()),
                    ..Default::default()
                },
                [DetailField::Title],
            )),
        ),
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                candidates[3].identity.clone(),
                RequestedDetailFields::new([DetailField::Company]).unwrap(),
            ),
            Ok(successful_detail(
                DetailPatch {
                    company: Some("ACME".into()),
                    ..Default::default()
                },
                [DetailField::Company],
            )),
        ),
    ]);

    let result = resolve_fixture(&requirements(), candidates, &detail).await;

    assert_eq!((result.counts.finalized, result.counts.rejected), (3, 1));
    assert!(result
        .finalized
        .iter()
        .all(|value| value.title() == "Engineer"));
    detail.assert_finished();
}

#[tokio::test]
async fn final_commit_cancellation_releases_finalized_values() {
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(
            vec![occurrence("1", Some("Engineer"), Some("ACME"))],
            true,
            Some(0),
            None,
        )],
    );
    let detail = ScriptedSourceDetailExecution::new([]);
    let cancellation = CancelOnCheck {
        after: 2,
        checks: AtomicUsize::new(0),
    };

    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &cancellation,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;

    assert_eq!(result, Err(SourceResolutionError::Cancelled));
}

#[tokio::test]
async fn continuation_protocol_and_remaining_recurrence_are_checked() {
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [
            batch(
                vec![
                    occurrence("1", Some("Engineer"), Some("A")),
                    occurrence("2", Some("Engineer"), Some("B")),
                ],
                false,
                Some(2),
                Some("next"),
            ),
            expected_batch(
                vec![occurrence("3", Some("Engineer"), Some("C"))],
                true,
                Some(0),
                None,
                Some("next"),
                1,
                98,
            ),
        ],
    );
    let detail = ScriptedSourceDetailExecution::new([]);
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(
        discovery.recorded_continuations(),
        vec![None, Some("next".into())]
    );
    assert_eq!(
        result.remaining, None,
        "2 - 1 != 0 permanently degrades remaining"
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "discovery_remaining_inconsistent")
            .count(),
        1
    );
    assert_eq!(result.counts.finalized, 3);
}

#[tokio::test]
async fn duplicate_occurrence_aborts_without_resolution() {
    let repeated = occurrence("same", Some("Engineer"), Some("A"));
    let detail = ScriptedSourceDetailExecution::new([]);
    let result =
        resolve_fixture_result(&requirements(), vec![repeated.clone(), repeated], &detail).await;
    assert!(matches!(
        result,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::ProtocolInvariant,
            ..
        })
    ));
}

#[tokio::test]
async fn resolver_failure_that_looks_like_authored_input_remains_geo_resolution_failure() {
    let geo = FailingGeo("Search Request location could not be resolved: resolver unavailable");
    let requirements = CompiledSearchRequirements::compile_with_geo(
        &[rule(SearchRuleKind::Text, "engineer")],
        &[],
        &["Berlin".into()],
        Some(25),
        &geo,
    )
    .await
    .expect("resolver failures remain owned by Source Resolution");
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", []);
    let detail = ScriptedSourceDetailExecution::new([]);

    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements,
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;

    assert!(matches!(
        result,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::GeoResolution,
            ..
        })
    ));
}

#[tokio::test]
async fn matching_title_with_required_missing_location_requests_only_location_detail() {
    let mut candidate = occurrence("location-only", Some("Engineer"), Some("ACME"));
    candidate.provider_values.locations.clear();
    let detail = ScriptedSourceDetailExecution::new([(
        SourceDetailRequestSnapshot::new(
            "fixture_source",
            candidate.identity.clone(),
            RequestedDetailFields::new([DetailField::Locations]).unwrap(),
        ),
        Ok(successful_detail(
            DetailPatch {
                locations: Some(vec!["Berlin".into()]),
                ..Default::default()
            },
            [DetailField::Locations],
        )),
    )]);
    let geo = SamePlace;
    let requirements = CompiledSearchRequirements::compile_with_geo(
        &[rule(SearchRuleKind::Text, "engineer")],
        &[],
        &["Berlin".into()],
        Some(25),
        &geo,
    )
    .await
    .unwrap();

    let result = resolve_fixture(&requirements, vec![candidate], &detail).await;

    assert_eq!(result.counts.finalized, 1);
    assert_eq!(result.finalized[0].locations(), ["Berlin"]);
    detail.assert_finished();
}

#[tokio::test]
async fn authored_locations_without_radius_keep_not_applied_diagnostic() {
    let requirements = CompiledSearchRequirements::compile(
        &[rule(SearchRuleKind::Text, "engineer")],
        &[],
        &["Berlin".into()],
        None,
    )
    .unwrap();
    let detail = ScriptedSourceDetailExecution::new([]);

    let result = resolve_fixture(
        &requirements,
        vec![occurrence("missing-radius", Some("Engineer"), Some("ACME"))],
        &detail,
    )
    .await;

    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "location_filter_not_applied_missing_radius_km"));
}

#[tokio::test]
async fn cancellation_releases_no_resolution_and_radius_is_rejected() {
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", []);
    let detail = ScriptedSourceDetailExecution::new([]);
    let cancelled = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &Cancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;
    assert_eq!(cancelled, Err(SourceResolutionError::Cancelled));
    assert_eq!(
        discovery.recorded_continuations(),
        Vec::<Option<String>>::new()
    );
    assert_eq!(
        CompiledSearchRequirements::compile(&[], &[], &["Berlin".into()], Some(10)).unwrap_err(),
        RequirementsCompilationFailure::RadiusRequiresGeoResolver
    );
}

#[tokio::test]
async fn malformed_batch_shapes_and_foreign_continuations_abort() {
    let source = compiled_source();
    let cases = vec![
        ScriptedDiscoveryBatch {
            expected_continuation: None,
            expected_maximum: 100,
            expected_limits: discovery_limits(0, 100),
            occurrences: vec![],
            exhausted: false,
            remaining: None,
            continuation: Some("next".into()),
            continuation_source_key: None,
            complete_budget_report: report(1),
            diagnostics: vec![],
        },
        ScriptedDiscoveryBatch {
            expected_continuation: None,
            expected_maximum: 100,
            expected_limits: discovery_limits(0, 100),
            occurrences: vec![occurrence("1", Some("Engineer"), Some("A"))],
            exhausted: true,
            remaining: Some(0),
            continuation: Some("impossible".into()),
            continuation_source_key: None,
            complete_budget_report: report(1),
            diagnostics: vec![],
        },
        ScriptedDiscoveryBatch {
            expected_continuation: None,
            expected_maximum: 100,
            expected_limits: discovery_limits(0, 100),
            occurrences: vec![occurrence("1", Some("Engineer"), Some("A"))],
            exhausted: false,
            remaining: Some(1),
            continuation: Some("foreign".into()),
            continuation_source_key: Some("other_source".into()),
            complete_budget_report: report(1),
            diagnostics: vec![],
        },
    ];
    for malformed in cases {
        let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", [malformed]);
        let detail = ScriptedSourceDetailExecution::new([]);
        let result = resolve_source_candidates(SourceResolutionRequest {
            compiled_source: &source,
            requirements: &requirements(),
            ceilings: ceilings(),
            cancellation: &NeverCancelled,
            discovery: SourceDiscovery::scripted(&discovery),
            detail: &detail,
        })
        .await;
        assert!(matches!(
            result,
            Err(SourceResolutionError::Failed {
                failure: ResolutionFailure::ProtocolInvariant,
                ..
            })
        ));
    }

    let mut low = ceilings();
    low.max_batch_size = 1;
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [expected_batch(
            vec![
                occurrence("1", Some("Engineer"), Some("A")),
                occurrence("2", Some("Engineer"), Some("B")),
            ],
            true,
            Some(0),
            None,
            None,
            0,
            1,
        )],
    );
    let detail = ScriptedSourceDetailExecution::new([]);
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: low,
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;
    assert!(matches!(
        result,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::ProtocolInvariant,
            ..
        })
    ));
}

#[tokio::test]
async fn exact_detail_candidate_bound_returns_partial_with_current_unresolved_and_later_skipped() {
    let source = compiled_source();
    let candidates = vec![
        occurrence("1", None, None),
        occurrence("2", None, None),
        occurrence("3", None, None),
    ];
    let requested = RequestedDetailFields::new([DetailField::Title, DetailField::Company]).unwrap();
    let detail = ScriptedSourceDetailExecution::new([(
        SourceDetailRequestSnapshot::new(
            "fixture_source",
            candidates[0].identity.clone(),
            requested,
        ),
        Ok(successful_detail(
            DetailPatch {
                title: Some("Engineer".into()),
                company: Some("A".into()),
                ..Default::default()
            },
            [DetailField::Title, DetailField::Company],
        )),
    )]);
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(candidates, true, Some(0), None)],
    );
    let mut limits = ceilings();
    limits.max_detail_candidates = 1;
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: limits,
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(
        result.completion,
        ResolutionCompletion::Partial {
            limit_reached: ResolutionLimitDimension::DetailCandidates
        }
    );
    assert_eq!(
        (
            result.counts.finalized,
            result.counts.unresolved,
            result.counts.budget_skipped,
            result.counts.processed
        ),
        (1, 1, 1, 2)
    );
    assert_eq!(result.counts.discovered, 3);
    detail.assert_finished();
}

#[tokio::test]
async fn source_detail_terminal_mapping_covers_conflicted_no_progress_abort_and_cancellation() {
    let candidate = occurrence("mapping", Some("Engineer"), None);
    let requested = RequestedDetailFields::new([DetailField::Company]).unwrap();
    let snapshot =
        SourceDetailRequestSnapshot::new("fixture_source", candidate.identity.clone(), requested);

    let detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::Completed {
            fields: DetailPatch::default(),
            dispositions: vec![RequestedFieldDisposition::Conflicted {
                field: DetailField::Company,
            }],
            phase_evidence: Some(SourceDetailPhaseEvidence {
                complete_budget_report: terminal_report(PhaseCompletion::PolicyUnsatisfied),
                diagnostics: vec![],
            }),
        }),
    )]);
    let unresolved = resolve_fixture(&requirements(), vec![candidate.clone()], &detail).await;
    assert_eq!(unresolved.counts.unresolved, 1);
    assert_eq!(unresolved.counts.finalized, 0);

    let mismatch_detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::SourceMismatch),
    )]);
    let mismatch =
        resolve_fixture_result(&requirements(), vec![candidate.clone()], &mismatch_detail).await;
    assert!(matches!(
        mismatch,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::SourceMismatch,
            ..
        })
    ));

    let detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::BudgetExhausted {
            complete_budget_report: budget_report(),
            diagnostics: vec![],
        }),
    )]);
    let partial = resolve_fixture(&requirements(), vec![candidate.clone()], &detail).await;
    assert_eq!(
        partial.completion,
        ResolutionCompletion::Partial {
            limit_reached: ResolutionLimitDimension::Requests
        }
    );
    assert_eq!(partial.counts.unresolved, 1);

    let source_abort_diagnostic = Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_detail_abort".into(),
        message: "Source Detail aborted".into(),
        severity: DiagnosticSeverity::Error,
        path: "/detail".into(),
        strategy_key: None,
        details: None,
    };
    let detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::SourceExecutionFailed {
            typed_failure: SourceDetailFailure::PhaseExecution {
                failure: PhaseExecutionFailure::Internal,
            },
            complete_budget_report: Some(terminal_report(PhaseCompletion::ExecutionFailed)),
            diagnostics: vec![source_abort_diagnostic.clone()],
        }),
    )]);
    let aborted = resolve_fixture_result(&requirements(), vec![candidate.clone()], &detail).await;
    assert_eq!(
        aborted,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::SourceDetailExecution,
            diagnostics: vec![source_abort_diagnostic],
        })
    );

    let detail = ScriptedSourceDetailExecution::new([(
        snapshot,
        Err(PhaseCancelled {
            complete_budget_report: terminal_report(PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }),
            diagnostics: vec![],
        }),
    )]);
    let cancelled = resolve_fixture_result(&requirements(), vec![candidate], &detail).await;
    assert_eq!(cancelled, Err(SourceResolutionError::Cancelled));
}

#[tokio::test]
async fn detail_diagnostics_are_appended_in_execution_order() {
    let source = compiled_source();
    let candidates = vec![
        occurrence("diagnostic-completed", Some("Engineer"), None),
        occurrence("diagnostic-budget", Some("Engineer"), None),
    ];
    let diagnostic = |code: &str| Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: code.into(),
        severity: DiagnosticSeverity::Warning,
        path: "/candidate-resolution".into(),
        strategy_key: None,
        details: None,
    };
    let discovery_diagnostic = diagnostic("discovery");
    let completed_diagnostic = diagnostic("detail_completed");
    let budget_diagnostic = diagnostic("detail_budget");
    let requested = RequestedDetailFields::new([DetailField::Company]).unwrap();
    let detail = ScriptedSourceDetailExecution::new([
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                candidates[0].identity.clone(),
                requested.clone(),
            ),
            Ok(SourceDetailOutcome::Completed {
                fields: DetailPatch::default(),
                dispositions: vec![RequestedFieldDisposition::Unavailable {
                    field: DetailField::Company,
                }],
                phase_evidence: Some(SourceDetailPhaseEvidence {
                    complete_budget_report: terminal_report(PhaseCompletion::PolicyUnsatisfied),
                    diagnostics: vec![completed_diagnostic.clone()],
                }),
            }),
        ),
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                candidates[1].identity.clone(),
                requested,
            ),
            Ok(SourceDetailOutcome::BudgetExhausted {
                complete_budget_report: budget_report(),
                diagnostics: vec![budget_diagnostic.clone()],
            }),
        ),
    ]);
    let mut discovery_batch = batch(candidates, true, Some(0), None);
    discovery_batch.diagnostics = vec![discovery_diagnostic.clone()];
    let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", [discovery_batch]);

    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();

    assert_eq!(
        result.diagnostics,
        vec![
            discovery_diagnostic,
            completed_diagnostic,
            budget_diagnostic
        ]
    );
    assert_eq!(result.counts.unresolved, 2);
    assert!(matches!(
        result.completion,
        ResolutionCompletion::Partial { .. }
    ));
}

async fn failed_candidate_resolution(count: usize) -> SourceResolution {
    let occurrences = (0..count)
        .map(|index| occurrence(&format!("sample-{count}-{index}"), Some("Engineer"), None))
        .collect::<Vec<_>>();
    let script = occurrences.iter().map(|occurrence| {
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                occurrence.identity.clone(),
                RequestedDetailFields::new([DetailField::Company]).unwrap(),
            ),
            Ok(SourceDetailOutcome::CandidateExecutionFailed {
                typed_failure: CandidateDetailFailure::IncludesExecutionFailure,
                complete_budget_report: candidate_failure_report(),
                diagnostics: vec![Diagnostic {
                    category: DiagnosticCategory::Runtime,
                    code: "provider-secret-code".into(),
                    message: "secret payload".into(),
                    severity: DiagnosticSeverity::Error,
                    path: "/secret".into(),
                    strategy_key: None,
                    details: Some(json!({"secret":"payload"})),
                }],
            }),
        )
    });
    let detail = ScriptedSourceDetailExecution::new(script);
    resolve_fixture(&requirements(), occurrences, &detail).await
}

#[tokio::test]
async fn candidate_sampling_boundaries_keep_nine_ten_and_only_first_ten_of_larger_stream() {
    for (count, expected_samples, expected_omitted) in [(9, 9, 0), (10, 10, 0), (25, 10, 15)] {
        let result = failed_candidate_resolution(count).await;
        assert_eq!(result.candidate_diagnostics.samples.len(), expected_samples);
        assert_eq!(
            result.candidate_diagnostics.candidate_diagnostics_omitted,
            expected_omitted
        );
        assert_eq!(result.candidate_diagnostics.sample_limit, 10);
        assert_eq!(result.counts.failed, count as u64);
        assert_eq!(
            result.candidate_diagnostics.counts_by_code["candidate_detail_execution_failed"],
            count as u64
        );
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("secret payload"));
    }
}

#[tokio::test]
async fn cumulative_child_duration_above_parent_ceiling_is_an_invariant_failure() {
    let source = compiled_source();
    let mut first = batch(
        vec![occurrence("duration-1", Some("Engineer"), Some("A"))],
        false,
        Some(1),
        Some("next"),
    );
    first.complete_budget_report.usage.duration_ms = 70_000;
    let mut second = expected_batch(
        vec![occurrence("duration-2", Some("Engineer"), Some("B"))],
        true,
        Some(0),
        None,
        Some("next"),
        1,
        99,
    );
    second.complete_budget_report.usage.duration_ms = 70_000;
    let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", [first, second]);
    let detail = ScriptedSourceDetailExecution::new([]);

    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;

    assert!(matches!(
        result,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::ReportAboveAllowance,
            ..
        })
    ));
}

#[tokio::test]
async fn source_detail_failure_report_presence_must_match_the_typed_failure() {
    let candidate = occurrence("bad-evidence", Some("Engineer"), None);
    let requested = RequestedDetailFields::new([DetailField::Company]).unwrap();
    let detail = ScriptedSourceDetailExecution::new([(
        SourceDetailRequestSnapshot::new("fixture_source", candidate.identity.clone(), requested),
        Ok(SourceDetailOutcome::SourceExecutionFailed {
            typed_failure: SourceDetailFailure::PhaseExecution {
                failure: PhaseExecutionFailure::Internal,
            },
            complete_budget_report: None,
            diagnostics: vec![],
        }),
    )]);
    let result = resolve_fixture_result(&requirements(), vec![candidate], &detail).await;

    assert!(matches!(
        result,
        Err(SourceResolutionError::Failed {
            failure: ResolutionFailure::ProtocolInvariant,
            ..
        })
    ));
}

#[tokio::test]
async fn production_one_shot_discovery_adapter_uses_true_effects_without_slicing() {
    let source = compiled_source();
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/feed".into(),
        headers: vec![],
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({"jobs":[{"title":"Engineer","company":"ACME","url":"https://example.test/jobs/production"}]})
                .to_string()
                .into_bytes(),
        )],
        content_length: None,
    }]);
    let acquisition = ScriptedBrowserAcquisition::new([]);
    let discovery = SourceDiscovery::source_engine(&fetcher, &acquisition);
    let detail = ScriptedSourceDetailExecution::new([]);
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery,
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(result.completion, ResolutionCompletion::Complete);
    assert_eq!(result.counts.finalized, 1);
    assert_eq!(result.remaining, Some(0));
    assert_eq!(fetcher.requests().len(), 1);
}

#[tokio::test]
async fn ceilings_are_tighten_only_against_existing_backend_dimensions() {
    let valid = ceilings();
    assert_eq!(valid.validate(), Ok(valid));

    let mut cases = Vec::new();
    let mut raised = valid;
    raised.phase.max_requests = PhaseLimits::BACKEND.max_requests + 1;
    cases.push(raised);
    let mut raised = valid;
    raised.max_batch_size = PhaseLimits::BACKEND.max_produced_items + 1;
    cases.push(raised);
    let mut raised = valid;
    raised.max_discovery_batches = PhaseLimits::BACKEND.max_pages + 1;
    cases.push(raised);
    let mut raised = valid;
    raised.max_discovered_items = PhaseLimits::BACKEND.max_produced_items + 1;
    cases.push(raised);
    let mut raised = valid;
    raised.max_detail_candidates = PhaseLimits::BACKEND.max_fan_out + 1;
    cases.push(raised);

    for raised in cases {
        assert_eq!(raised.validate(), Err(ResolutionFailure::InvalidInput));
    }
}

#[tokio::test]
async fn child_reports_are_checked_against_exact_tightened_batch_limits() {
    let source = compiled_source();
    for usage in [
        PhaseUsage {
            produced_items: 2,
            ..Default::default()
        },
        PhaseUsage {
            duration_ms: PhaseLimits::BACKEND.max_duration_ms + 1,
            ..Default::default()
        },
    ] {
        let mut scripted = expected_batch(
            vec![occurrence("over", Some("Engineer"), Some("A"))],
            true,
            Some(0),
            None,
            None,
            0,
            1,
        );
        scripted.complete_budget_report = PhaseExecutionReport {
            usage,
            completion: PhaseCompletion::Accepted,
        };
        let discovery = ScriptedSourceDiscoveryExecution::new("fixture_source", [scripted]);
        let detail = ScriptedSourceDetailExecution::new([]);
        let mut limits = ceilings();
        limits.max_batch_size = 1;
        let result = resolve_source_candidates(SourceResolutionRequest {
            compiled_source: &source,
            requirements: &requirements(),
            ceilings: limits,
            cancellation: &NeverCancelled,
            discovery: SourceDiscovery::scripted(&discovery),
            detail: &detail,
        })
        .await;
        assert!(matches!(
            result,
            Err(SourceResolutionError::Failed {
                failure: ResolutionFailure::ReportAboveAllowance,
                ..
            })
        ));
    }
}

#[tokio::test]
async fn discovery_budget_exhaustion_is_partial_and_preserves_prior_batch_and_report() {
    let source = compiled_source();
    let terminal_diagnostic = Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "discovery_budget_exhausted".into(),
        message: "Discovery budget exhausted".into(),
        severity: DiagnosticSeverity::Warning,
        path: "/discovery".into(),
        strategy_key: None,
        details: None,
    };
    let terminal_report = PhaseExecutionReport {
        usage: PhaseUsage {
            requests: 2,
            response_bytes: 20,
            ..Default::default()
        },
        completion: budget_report().completion,
    };
    let discovery = ScriptedSourceDiscoveryExecution::new_outcomes(
        "fixture_source",
        [
            ScriptedDiscoveryOutcome::Batch(batch(
                vec![occurrence("kept", Some("Engineer"), Some("A"))],
                false,
                Some(2),
                Some("next"),
            )),
            ScriptedDiscoveryOutcome::BudgetExhausted {
                expected_continuation: Some("next".into()),
                expected_maximum: 99,
                expected_limits: discovery_limits(1, 99),
                complete_budget_report: terminal_report,
                diagnostics: vec![terminal_diagnostic.clone()],
            },
        ],
    );
    let detail = ScriptedSourceDetailExecution::new([]);
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(result.finalized.len(), 1);
    assert_eq!(
        result.finalized[0].identity(),
        &occurrence("kept", None, None).identity
    );
    assert_eq!(result.counts.discovered, 1);
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.response_bytes, 30);
    assert_eq!(result.diagnostics, vec![terminal_diagnostic]);
    assert_eq!(
        result.completion,
        ResolutionCompletion::Partial {
            limit_reached: ResolutionLimitDimension::Requests
        }
    );
    discovery.assert_finished();
}

#[tokio::test]
async fn fully_processed_batches_are_retained_at_the_next_batch_boundary() {
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [
            batch(
                vec![occurrence("one", Some("Engineer"), Some("A"))],
                false,
                Some(2),
                Some("second"),
            ),
            expected_batch(
                vec![occurrence("two", Some("Engineer"), Some("B"))],
                false,
                Some(1),
                Some("third"),
                Some("second"),
                1,
                99,
            ),
        ],
    );
    let detail = ScriptedSourceDetailExecution::new([]);
    let mut limits = ceilings();
    limits.max_discovery_batches = 2;
    let result = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: limits,
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(result.counts.discovered, 2);
    assert_eq!(result.counts.finalized, 2);
    assert_eq!(result.finalized.len(), 2);
    assert_eq!(result.remaining, Some(1));
    assert_eq!(
        result.completion,
        ResolutionCompletion::Partial {
            limit_reached: ResolutionLimitDimension::DiscoveryBatches
        }
    );
    discovery.assert_finished();
}

#[tokio::test]
async fn unchanged_and_reused_continuations_abort_before_another_batch() {
    let source = compiled_source();
    for continuations in [["same", "same", "unused"], ["a", "b", "a"]] {
        let discovery = ScriptedSourceDiscoveryExecution::new(
            "fixture_source",
            [
                batch(
                    vec![occurrence("1", Some("Engineer"), Some("A"))],
                    false,
                    Some(3),
                    Some(continuations[0]),
                ),
                expected_batch(
                    vec![occurrence("2", Some("Engineer"), Some("B"))],
                    false,
                    Some(2),
                    Some(continuations[1]),
                    Some(continuations[0]),
                    1,
                    99,
                ),
                expected_batch(
                    vec![occurrence("3", Some("Engineer"), Some("C"))],
                    false,
                    Some(1),
                    Some(continuations[2]),
                    Some(continuations[1]),
                    2,
                    98,
                ),
            ],
        );
        let detail = ScriptedSourceDetailExecution::new([]);
        let result = resolve_source_candidates(SourceResolutionRequest {
            compiled_source: &source,
            requirements: &requirements(),
            ceilings: ceilings(),
            cancellation: &NeverCancelled,
            discovery: SourceDiscovery::scripted(&discovery),
            detail: &detail,
        })
        .await;
        assert!(matches!(
            result,
            Err(SourceResolutionError::Failed {
                failure: ResolutionFailure::ProtocolInvariant,
                ..
            })
        ));
    }
}
