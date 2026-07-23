mod support;

use job_radar_lib::{
    resolve_source_candidates, AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource,
    CompileSourceOutcome, CompiledSearchRequirements, DetailField, DetailPatch,
    PhaseCancellationReason, PhaseCancelled, PhaseCompletion, PhaseExecutionFailure,
    PhaseExecutionReport, PhaseLimits, PhaseUsage, PostingOccurrence, PostingOccurrenceIdentity,
    PostingReference, ProviderValues, RequestedFieldDisposition, ResolutionCeilings,
    ResolutionCompletion, ResolutionFailure, RuntimeCancellation, ScriptedBrowserAcquisition,
    ScriptedDiscoveryBatch, ScriptedDiscoveryOutcome, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, ScriptedSourceDetailExecution, ScriptedSourceDiscoveryExecution,
    SearchRule, SearchRuleKind, SearchRuleTarget, SourceDetailFailure, SourceDetailOutcome,
    SourceDetailRequestSnapshot, SourceDiscovery, SourceDocument, SourceProfileDocument,
    SourceResolutionError, SourceResolutionRequest, CANDIDATE_DIAGNOSTIC_SAMPLE_LIMIT,
};
use serde_json::json;
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

struct CancelOnCheck {
    after: usize,
    checks: AtomicUsize,
}
impl RuntimeCancellation for CancelOnCheck {
    fn is_cancelled(&self) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst) >= self.after
    }
}

fn compiled_source() -> job_radar_lib::CompiledSource {
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
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "fixture_source", "name": "Fixture source", "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/feed" },
        "selectedAccessPath": { "type": "profile_access_path", "profileKey": "fixture_profile", "pathKey": "default" }
    })).unwrap();
    match support::compile_test_source(&source, Some(profile)) {
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
fn requirements() -> CompiledSearchRequirements<'static> {
    CompiledSearchRequirements::compile(
        &[SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Text,
            value: "engineer".into(),
        }],
        &[],
        &[],
        None,
    )
    .unwrap()
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
    let source = compiled_source();
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(
            vec![occurrence(
                "1",
                Some("  Software   Engineer "),
                Some(" ACME "),
            )],
            true,
            Some(0),
            None,
        )],
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
    assert_eq!(result.completion, ResolutionCompletion::Complete);
    assert_eq!((result.counts.discovered, result.counts.finalized), (1, 1));
    assert_eq!(result.remaining, Some(0));
    assert_eq!(result.finalized[0].title(), "Software Engineer");
    assert_eq!(result.finalized[0].company(), "ACME");
    assert_eq!(result.finalized[0].locations(), vec!["Berlin"]);
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains("must-not-escape"));
    assert!(!serialized.contains("postingMeta"));
    discovery.assert_finished();
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
    let source = compiled_source();
    let repeated = occurrence("same", Some("Engineer"), Some("A"));
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![repeated.clone(), repeated], true, Some(0), None)],
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

#[tokio::test]
async fn detail_failures_continue_and_sampling_is_fixed_at_ten() {
    let source = compiled_source();
    let occurrences = (0..11)
        .map(|i| occurrence(&i.to_string(), Some("Engineer"), None))
        .collect::<Vec<_>>();
    let script = occurrences.iter().map(|o| {
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                o.identity.clone(),
                job_radar_lib::RequestedDetailFields::new([DetailField::Company]).unwrap(),
            ),
            Ok(SourceDetailOutcome::CandidateExecutionFailed {
                typed_failure: job_radar_lib::CandidateDetailFailure::IncludesExecutionFailure,
                complete_budget_report: candidate_failure_report(),
                diagnostics: vec![job_radar_lib::Diagnostic {
                    category: job_radar_lib::DiagnosticCategory::Runtime,
                    code: "provider-secret-code".into(),
                    message: "secret payload".into(),
                    severity: job_radar_lib::DiagnosticSeverity::Error,
                    path: "/secret".into(),
                    strategy_key: None,
                    details: Some(json!({"secret":"payload"})),
                }],
            }),
        )
    });
    let detail = ScriptedSourceDetailExecution::new(script);
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(occurrences, true, Some(0), None)],
    );
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
    assert_eq!(result.counts.failed, 11);
    assert_eq!(
        result.candidate_diagnostics.samples.len(),
        CANDIDATE_DIAGNOSTIC_SAMPLE_LIMIT
    );
    assert_eq!(
        result.candidate_diagnostics.candidate_diagnostics_omitted,
        1
    );
    assert_eq!(
        result.candidate_diagnostics.counts_by_code["candidate_detail_execution_failed"],
        11
    );
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("secret payload"));
    detail.assert_finished();
}

#[tokio::test]
async fn minimal_multi_field_detail_patch_finalizes_and_accounting_is_exact() {
    let source = compiled_source();
    let candidate = occurrence("1", None, None);
    let requested =
        job_radar_lib::RequestedDetailFields::new([DetailField::Title, DetailField::Company])
            .unwrap();
    let detail = ScriptedSourceDetailExecution::new([(
        SourceDetailRequestSnapshot::new("fixture_source", candidate.identity.clone(), requested),
        Ok(SourceDetailOutcome::Completed {
            fields: DetailPatch {
                title: Some("Engineer".into()),
                company: Some("ACME".into()),
                ..Default::default()
            },
            dispositions: vec![
                RequestedFieldDisposition::Produced {
                    field: DetailField::Title,
                },
                RequestedFieldDisposition::Produced {
                    field: DetailField::Company,
                },
            ],
            phase_evidence: Some(job_radar_lib::SourceDetailPhaseEvidence {
                complete_budget_report: report(2),
                diagnostics: vec![],
            }),
        }),
    )]);
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate], true, Some(0), None)],
    );
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
    assert_eq!(result.counts.finalized, 1);
    assert_eq!(result.report.usage.requests, 3);
    assert_eq!(result.report.usage.response_bytes, 30);
    detail.assert_finished();
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
        job_radar_lib::RequirementsCompilationFailure::RadiusRequiresGeoResolver
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
    let requested =
        job_radar_lib::RequestedDetailFields::new([DetailField::Title, DetailField::Company])
            .unwrap();
    let detail = ScriptedSourceDetailExecution::new([(
        SourceDetailRequestSnapshot::new(
            "fixture_source",
            candidates[0].identity.clone(),
            requested,
        ),
        Ok(SourceDetailOutcome::Completed {
            fields: DetailPatch {
                title: Some("Engineer".into()),
                company: Some("A".into()),
                ..Default::default()
            },
            dispositions: vec![
                RequestedFieldDisposition::Produced {
                    field: DetailField::Title,
                },
                RequestedFieldDisposition::Produced {
                    field: DetailField::Company,
                },
            ],
            phase_evidence: Some(job_radar_lib::SourceDetailPhaseEvidence {
                complete_budget_report: report(1),
                diagnostics: vec![],
            }),
        }),
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
            limit_reached: job_radar_lib::ResolutionLimitDimension::DetailCandidates
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
    let source = compiled_source();
    let candidate = occurrence("mapping", Some("Engineer"), None);
    let requested = job_radar_lib::RequestedDetailFields::new([DetailField::Company]).unwrap();
    let snapshot =
        SourceDetailRequestSnapshot::new("fixture_source", candidate.identity.clone(), requested);

    let detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::Completed {
            fields: DetailPatch::default(),
            dispositions: vec![RequestedFieldDisposition::Conflicted {
                field: DetailField::Company,
            }],
            phase_evidence: Some(job_radar_lib::SourceDetailPhaseEvidence {
                complete_budget_report: terminal_report(PhaseCompletion::PolicyUnsatisfied),
                diagnostics: vec![],
            }),
        }),
    )]);
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate.clone()], true, Some(0), None)],
    );
    let unresolved = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap();
    assert_eq!(unresolved.counts.unresolved, 1);
    assert_eq!(unresolved.counts.finalized, 0);

    let mismatch_detail = ScriptedSourceDetailExecution::new([(
        snapshot.clone(),
        Ok(SourceDetailOutcome::SourceMismatch),
    )]);
    let mismatch_discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate.clone()], true, Some(0), None)],
    );
    let mismatch = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&mismatch_discovery),
        detail: &mismatch_detail,
    })
    .await;
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
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate.clone()], true, Some(0), None)],
    );
    let partial = resolve_source_candidates(SourceResolutionRequest {
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
        partial.completion,
        ResolutionCompletion::Partial {
            limit_reached: job_radar_lib::ResolutionLimitDimension::Requests
        }
    );
    assert_eq!(partial.counts.unresolved, 1);

    let source_abort_diagnostic = job_radar_lib::Diagnostic {
        category: job_radar_lib::DiagnosticCategory::Runtime,
        code: "source_detail_abort".into(),
        message: "Source Detail aborted".into(),
        severity: job_radar_lib::DiagnosticSeverity::Error,
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
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate.clone()], true, Some(0), None)],
    );
    let aborted = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;
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
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate], true, Some(0), None)],
    );
    let cancelled = resolve_source_candidates(SourceResolutionRequest {
        compiled_source: &source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await;
    assert_eq!(cancelled, Err(SourceResolutionError::Cancelled));
}

#[tokio::test]
async fn detail_diagnostics_are_appended_in_execution_order() {
    let source = compiled_source();
    let candidates = vec![
        occurrence("diagnostic-completed", Some("Engineer"), None),
        occurrence("diagnostic-budget", Some("Engineer"), None),
    ];
    let diagnostic = |code: &str| job_radar_lib::Diagnostic {
        category: job_radar_lib::DiagnosticCategory::Runtime,
        code: code.into(),
        message: code.into(),
        severity: job_radar_lib::DiagnosticSeverity::Warning,
        path: "/candidate-resolution".into(),
        strategy_key: None,
        details: None,
    };
    let discovery_diagnostic = diagnostic("discovery");
    let completed_diagnostic = diagnostic("detail_completed");
    let budget_diagnostic = diagnostic("detail_budget");
    let requested = job_radar_lib::RequestedDetailFields::new([DetailField::Company]).unwrap();
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
                phase_evidence: Some(job_radar_lib::SourceDetailPhaseEvidence {
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

async fn failed_candidate_resolution(
    source: &job_radar_lib::CompiledSource,
    count: usize,
) -> job_radar_lib::SourceResolution {
    let occurrences = (0..count)
        .map(|index| occurrence(&format!("sample-{count}-{index}"), Some("Engineer"), None))
        .collect::<Vec<_>>();
    let script = occurrences.iter().map(|occurrence| {
        (
            SourceDetailRequestSnapshot::new(
                "fixture_source",
                occurrence.identity.clone(),
                job_radar_lib::RequestedDetailFields::new([DetailField::Company]).unwrap(),
            ),
            Ok(SourceDetailOutcome::CandidateExecutionFailed {
                typed_failure: job_radar_lib::CandidateDetailFailure::IncludesExecutionFailure,
                complete_budget_report: candidate_failure_report(),
                diagnostics: vec![],
            }),
        )
    });
    let detail = ScriptedSourceDetailExecution::new(script);
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(occurrences, true, Some(0), None)],
    );
    resolve_source_candidates(SourceResolutionRequest {
        compiled_source: source,
        requirements: &requirements(),
        ceilings: ceilings(),
        cancellation: &NeverCancelled,
        discovery: SourceDiscovery::scripted(&discovery),
        detail: &detail,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn candidate_sampling_boundaries_keep_nine_ten_and_only_first_ten_of_larger_stream() {
    let source = compiled_source();
    for (count, expected_samples, expected_omitted) in [(9, 9, 0), (10, 10, 0), (25, 10, 15)] {
        let result = failed_candidate_resolution(&source, count).await;
        assert_eq!(result.candidate_diagnostics.samples.len(), expected_samples);
        assert_eq!(
            result.candidate_diagnostics.candidate_diagnostics_omitted,
            expected_omitted
        );
        assert_eq!(result.candidate_diagnostics.sample_limit, 10);
        assert_eq!(
            result.candidate_diagnostics.counts_by_code["candidate_detail_execution_failed"],
            count as u64
        );
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
    let source = compiled_source();
    let candidate = occurrence("bad-evidence", Some("Engineer"), None);
    let requested = job_radar_lib::RequestedDetailFields::new([DetailField::Company]).unwrap();
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
    let discovery = ScriptedSourceDiscoveryExecution::new(
        "fixture_source",
        [batch(vec![candidate], true, Some(0), None)],
    );

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
    let discovery = SourceDiscovery::profile_dsl(&fetcher, &acquisition);
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
    let terminal_diagnostic = job_radar_lib::Diagnostic {
        category: job_radar_lib::DiagnosticCategory::Runtime,
        code: "discovery_budget_exhausted".into(),
        message: "Discovery budget exhausted".into(),
        severity: job_radar_lib::DiagnosticSeverity::Warning,
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
            limit_reached: job_radar_lib::ResolutionLimitDimension::Requests
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
            limit_reached: job_radar_lib::ResolutionLimitDimension::DiscoveryBatches
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
