#[path = "support/mod.rs"]
mod support;

use job_radar_lib::{
    CandidateDetailFailure, CompileSourceOutcome, DetailField, Diagnostic, DiagnosticCategory,
    DiagnosticSeverity, PhaseCompletion, PhaseExecutionFailure, PhaseExecutionReport, PhaseLimits,
    PhaseOutcome, PhasePreStartFailure, PhaseRunError, PhaseUsage, PolicyOutcome,
    PolicyUnsatisfiedCause, ProfileDslSourceDetailExecution, ProfileHttpFailureKind,
    ProviderValues, RequestedDetailFields, RequestedFieldDisposition, RuntimeCancellation,
    RuntimeExecutionContext, ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
    ScriptedSourceDetailExecution, SourceDetailExecution, SourceDetailFailure, SourceDetailOutcome,
    SourceDetailRequest, SourceDetailRequestSnapshot, SourceDocument, SourceProfileDocument,
    ScriptedBrowserAcquisition, PhaseBrowser, __test_execute_detail_phase,
};
use serde_json::{json, Value};

#[test]
fn compiler_derives_canonical_finite_capabilities_from_all_executable_strategies() {
    let compiled = compiled_source(vec![
        detail_strategy(
            "first",
            json!({
                "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
            }),
        ),
        detail_strategy(
            "second",
            json!({
                "title": { "type": "json_path", "jsonPath": "$.title" },
                "locations": { "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" }
            }),
        ),
    ]);

    assert_eq!(
        compiled.detail_capabilities().iter().collect::<Vec<_>>(),
        vec![
            DetailField::Title,
            DetailField::Locations,
            DetailField::DescriptionText
        ]
    );
}

#[test]
fn compiler_rejects_empty_detail_capability_shape() {
    let profile: SourceProfileDocument = serde_json::from_value(fixture_profile_value(
        vec![detail_strategy("empty", json!({}))],
        json!({ "type": "first_accepted" }),
    ))
    .unwrap();

    let outcome = support::compile_test_source(&fixture_source(), Some(profile));
    let CompileSourceOutcome::Rejected { diagnostics } = outcome else {
        panic!("empty executable Detail capability shape must reject compilation");
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .path
            .ends_with("/detail/strategies/0/extract/fields")
            && diagnostic.message.contains("at least one canonical field")
    }));
}

#[test]
fn authored_dynamic_detail_capability_name_is_rejected_by_closed_serde_shape() {
    let profile = fixture_profile_value(
        vec![detail_strategy(
            "dynamic",
            json!({
                "salary": { "type": "json_path", "jsonPath": "$.salary" }
            }),
        )],
        json!({ "type": "first_accepted" }),
    );

    let error = serde_json::from_value::<SourceProfileDocument>(profile).unwrap_err();
    assert!(error.to_string().contains("unknown field `salary`"));
}

fn browser_free_acquisition() -> &'static ScriptedBrowserAcquisition {
    Box::leak(Box::new(ScriptedBrowserAcquisition::new([])))
}

#[tokio::test]
async fn source_mismatch_precedes_reuse_routing_and_io() {
    let compiled = compiled_source(vec![detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    )]);
    let occurrence = occurrence("another_source", Some("already present"));
    let fetcher = ScriptedProfileHttpClient::new([]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    assert_eq!(outcome, SourceDetailOutcome::SourceMismatch);
    assert!(fetcher.requests().is_empty());
}

#[tokio::test]
async fn reused_and_unsupported_fields_complete_canonically_without_phase_evidence_or_io() {
    let compiled = compiled_source(vec![detail_strategy(
        "title_only",
        json!({
            "title": { "type": "json_path", "jsonPath": "$.title" }
        }),
    )]);
    let occurrence = occurrence("fixture_source", None);
    let fetcher = ScriptedProfileHttpClient::new([]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::new([
                DetailField::DescriptionText,
                DetailField::Company,
                DetailField::Title,
                DetailField::Company,
            ])
            .unwrap(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::Completed {
        fields,
        dispositions,
        phase_evidence,
    } = outcome
    else {
        panic!("expected completed Source Detail outcome");
    };
    assert_eq!(fields.title.as_deref(), Some("Occurrence title"));
    assert_eq!(fields.company.as_deref(), Some("Occurrence company"));
    assert_eq!(fields.description_text, None);
    assert_eq!(
        dispositions,
        vec![
            RequestedFieldDisposition::Reused {
                field: DetailField::Title
            },
            RequestedFieldDisposition::Reused {
                field: DetailField::Company
            },
            RequestedFieldDisposition::Unsupported {
                field: DetailField::DescriptionText
            },
        ]
    );
    assert_eq!(phase_evidence, None);
    assert!(fetcher.requests().is_empty());
}

#[tokio::test]
async fn one_unchanged_policy_invocation_can_produce_multiple_requested_fields() {
    let compiled = compiled_source(vec![
        detail_strategy(
            "first",
            json!({
                "company": { "type": "json_path", "jsonPath": "$.company" },
                "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
            }),
        ),
        detail_strategy(
            "fallback",
            json!({
                "descriptionText": { "type": "json_path", "jsonPath": "$.fallback" }
            }),
        ),
    ]);
    let mut occurrence = occurrence("fixture_source", None);
    occurrence.provider_values.company = None;
    let fetcher = response_client(json!({
        "company": "Produced company",
        "description": "Produced description"
    }));
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::new([
                DetailField::DescriptionText,
                DetailField::Company,
            ])
            .unwrap(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::Completed {
        fields,
        dispositions,
        phase_evidence: Some(evidence),
    } = outcome
    else {
        panic!("expected started completed Source Detail outcome");
    };
    assert_eq!(fields.company.as_deref(), Some("Produced company"));
    assert_eq!(
        fields.description_text.as_deref(),
        Some("Produced description")
    );
    assert_eq!(
        dispositions,
        vec![
            RequestedFieldDisposition::Produced {
                field: DetailField::Company
            },
            RequestedFieldDisposition::Produced {
                field: DetailField::DescriptionText
            }
        ]
    );
    assert!(evidence.diagnostics.is_empty());
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(fetcher.requests()[0].url, "https://example.test/detail");
}

#[tokio::test]
async fn accepted_quarantined_patch_is_reported_as_conflicted_without_releasing_a_value() {
    let compiled = compiled_source_with_policy(
        vec![
            detail_strategy(
                "first",
                json!({
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
                }),
            ),
            detail_strategy(
                "second",
                json!({
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
                }),
            ),
        ],
        json!({ "type": "collect_all", "minAccepted": 1 }),
    );
    let occurrence = occurrence("fixture_source", None);
    let fetcher = ScriptedProfileHttpClient::new([
        response_event(json!({ "description": "First description" })),
        response_event(json!({ "description": "Conflicting description" })),
    ]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::Completed {
        fields,
        dispositions,
        phase_evidence: Some(_),
    } = outcome
    else {
        panic!("expected completed conflicted Source Detail");
    };
    assert_eq!(fields.description_text, None);
    assert_eq!(
        dispositions,
        vec![RequestedFieldDisposition::Conflicted {
            field: DetailField::DescriptionText
        }]
    );
    assert_eq!(fetcher.requests().len(), 2);
}

#[tokio::test]
async fn rejected_only_is_an_ordinary_completed_unavailable_result_with_phase_evidence() {
    let mut strategy = detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    );
    strategy["acceptWhen"] = json!({ "requiredFields": ["descriptionText"] });
    let compiled = compiled_source(vec![strategy]);
    let occurrence = occurrence("fixture_source", None);
    let fetcher = response_client(json!({}));
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::Completed {
        fields,
        dispositions,
        phase_evidence: Some(evidence),
    } = outcome
    else {
        panic!("expected unavailable completed Source Detail");
    };
    assert_eq!(fields.description_text, None);
    assert_eq!(
        dispositions,
        vec![RequestedFieldDisposition::Unavailable {
            field: DetailField::DescriptionText
        }]
    );
    let expected_report = PhaseExecutionReport {
        usage: PhaseUsage {
            duration_ms: evidence.complete_budget_report.usage.duration_ms,
            strategy_attempts: 1,
            requests: 1,
            response_bytes: 2,
            ..PhaseUsage::default()
        },
        completion: PhaseCompletion::PolicyUnsatisfied,
    };
    let expected_diagnostics = vec![
        Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "acceptance_required_field_missing".to_string(),
            message: "Detail patch is missing a required field".to_string(),
            severity: DiagnosticSeverity::Error,
            path: "/detail/strategies/0/acceptWhen/requiredFields".to_string(),
            strategy_key: Some("detail".to_string()),
            details: Some(json!({ "field": "descriptionText" })),
        },
        Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "fallback_exhausted".to_string(),
            message: "detail fallback strategies were exhausted without an accepted result"
                .to_string(),
            severity: DiagnosticSeverity::Error,
            path: "/detail/strategies".to_string(),
            strategy_key: None,
            details: Some(json!({})),
        },
    ];
    assert_eq!(evidence.complete_budget_report, expected_report);
    assert_eq!(evidence.diagnostics, expected_diagnostics);
}

#[tokio::test]
async fn budget_exhaustion_stays_distinct_and_releases_no_fields_or_dispositions() {
    let compiled = compiled_source_with_policy(
        vec![
            detail_strategy(
                "first",
                json!({
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
                }),
            ),
            detail_strategy(
                "second",
                json!({
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
                }),
            ),
        ],
        json!({ "type": "collect_all", "minAccepted": 1 }),
    );
    let occurrence = occurrence("fixture_source", None);
    let raw_fetcher = ScriptedProfileHttpClient::new([response_event(json!({
        "description": "First description"
    }))]);
    let limits = PhaseLimits {
        max_requests: 1,
        ..PhaseLimits::BACKEND
    };
    let (mut expected_report, expected_diagnostics) = match __test_execute_detail_phase(
        &compiled.execution_plan,
        &Default::default(),
        &occurrence,
        RequestedDetailFields::description_text(),
        &raw_fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable().with_limits(limits),
    )
    .await
    {
        Ok(PhaseOutcome::BudgetExhausted {
            complete_budget_report,
            diagnostics,
        }) => (complete_budget_report, diagnostics),
        other => panic!("expected lower-phase budget exhaustion, got {other:?}"),
    };
    let fetcher = ScriptedProfileHttpClient::new([response_event(json!({
        "description": "First description"
    }))]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable().with_limits(limits),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::BudgetExhausted {
        mut complete_budget_report,
        diagnostics,
    } = outcome
    else {
        panic!("expected Source Detail budget exhaustion");
    };
    assert!(complete_budget_report.usage.duration_ms <= limits.max_duration_ms);
    assert!(expected_report.usage.duration_ms <= limits.max_duration_ms);
    complete_budget_report.usage.duration_ms = 0;
    expected_report.usage.duration_ms = 0;
    assert_eq!(complete_budget_report, expected_report);
    assert_eq!(diagnostics, expected_diagnostics);
    assert_eq!(fetcher.requests().len(), 1);
}

#[tokio::test]
async fn invalid_caller_limits_are_source_execution_failure_with_exact_phase_evidence() {
    let compiled = compiled_source(vec![detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    )]);
    let occurrence = occurrence("fixture_source", None);
    let fetcher = ScriptedProfileHttpClient::new([]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
                max_requests: 0,
                ..PhaseLimits::BACKEND
            }),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::SourceExecutionFailed {
        typed_failure:
            SourceDetailFailure::PhaseExecution {
                failure: PhaseExecutionFailure::InvalidCallerLimits,
            },
        complete_budget_report: Some(report),
        diagnostics,
    } = outcome
    else {
        panic!("expected started Source execution failure");
    };
    assert_eq!(
        report,
        PhaseExecutionReport {
            usage: PhaseUsage::default(),
            completion: PhaseCompletion::ExecutionFailed,
        }
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "invalid_caller_phase_limits");
    assert_eq!(
        diagnostics[0].message,
        "Caller phase limits must be positive, may only tighten compiled limits, and must preserve the Browser teardown reserve"
    );
    assert_eq!(diagnostics[0].path, "/detail/limits");
    assert!(fetcher.requests().is_empty());
}

#[tokio::test]
async fn pre_start_request_mismatch_has_no_fabricated_budget_report() {
    let mut strategy = detail_strategy(
        "detail",
        json!({
            "title": { "type": "json_path", "jsonPath": "$.title" },
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    );
    strategy["acceptWhen"] = json!({ "requiredFields": ["title"] });
    let compiled = compiled_source(vec![strategy]);
    let occurrence = occurrence("fixture_source", None);
    let fetcher = ScriptedProfileHttpClient::new([]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::SourceExecutionFailed {
        typed_failure:
            SourceDetailFailure::PhasePreStart {
                failure: PhasePreStartFailure::RequestMismatch,
            },
        complete_budget_report,
        diagnostics,
    } = outcome
    else {
        panic!("expected pre-start Source execution failure");
    };
    assert_eq!(complete_budget_report, None);
    assert!(!diagnostics.is_empty());
    assert!(fetcher.requests().is_empty());
}

#[tokio::test]
async fn strategy_execution_failure_is_candidate_scoped_and_releases_no_fields() {
    let compiled = compiled_source(vec![detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    )]);
    let occurrence = occurrence("fixture_source", None);
    let failed_response = || ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/detail".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Failure(
            ProfileHttpFailureKind::Connect,
        )],
        content_length: None,
    };
    let raw_fetcher = ScriptedProfileHttpClient::new([failed_response()]);
    let (expected_report, expected_diagnostics) = match __test_execute_detail_phase(
        &compiled.execution_plan,
        &Default::default(),
        &occurrence,
        RequestedDetailFields::description_text(),
        &raw_fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    {
        Ok(PhaseOutcome::Completed {
            policy_outcome:
                PolicyOutcome::PolicyUnsatisfied {
                    cause: PolicyUnsatisfiedCause::IncludesExecutionFailure,
                },
            complete_budget_report,
            diagnostics,
        }) => (complete_budget_report, diagnostics),
        other => panic!("expected lower-phase execution failure, got {other:?}"),
    };
    let fetcher = ScriptedProfileHttpClient::new([failed_response()]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let outcome = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await
        .unwrap();

    let SourceDetailOutcome::CandidateExecutionFailed {
        typed_failure: CandidateDetailFailure::IncludesExecutionFailure,
        complete_budget_report,
        diagnostics,
    } = outcome
    else {
        panic!("expected candidate-scoped execution failure");
    };
    assert_eq!(complete_budget_report, expected_report);
    assert_eq!(diagnostics, expected_diagnostics);
}

#[tokio::test]
async fn cancellation_is_external_and_preserves_started_phase_evidence_without_result() {
    let compiled = compiled_source(vec![detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    )]);
    let occurrence = occurrence("fixture_source", None);
    let fetcher = ScriptedProfileHttpClient::new([]);
    let execution =
        ProfileDslSourceDetailExecution::new(&fetcher, browser_free_acquisition());

    let raw_cancelled = match __test_execute_detail_phase(
        &compiled.execution_plan,
        &Default::default(),
        &occurrence,
        RequestedDetailFields::description_text(),
        &fetcher,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::with_cancellation(&AlwaysCancelled),
    )
    .await
    {
        Err(PhaseRunError::Cancelled(cancelled)) => cancelled,
        other => panic!("expected exact lower-phase Cancellation, got {other:?}"),
    };
    let cancelled = execution
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: RequestedDetailFields::description_text(),
            context: RuntimeExecutionContext::with_cancellation(&AlwaysCancelled),
        })
        .await
        .expect_err("Cancellation must not release a Source Detail outcome");

    assert_eq!(cancelled, raw_cancelled);
    assert!(fetcher.requests().is_empty());
}

#[tokio::test]
async fn scripted_execution_records_exact_canonical_snapshot_and_rejects_no_contract_work() {
    let compiled = compiled_source(vec![detail_strategy(
        "detail",
        json!({
            "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
        }),
    )]);
    let occurrence = occurrence("fixture_source", None);
    let requested = RequestedDetailFields::description_text();
    let expected = SourceDetailRequestSnapshot::new(
        "fixture_source",
        occurrence.identity.clone(),
        requested.clone(),
    );
    let scripted = ScriptedSourceDetailExecution::new([(
        expected.clone(),
        Ok(SourceDetailOutcome::SourceMismatch),
    )]);

    let result = scripted
        .execute(SourceDetailRequest {
            compiled_source: &compiled,
            occurrence: &occurrence,
            requested_fields: requested,
            context: RuntimeExecutionContext::uncancellable(),
        })
        .await;

    assert_eq!(result.unwrap(), SourceDetailOutcome::SourceMismatch);
    assert_eq!(scripted.recorded_calls(), vec![expected]);
    scripted.assert_finished();
}

fn compiled_source(strategies: Vec<Value>) -> job_radar_lib::CompiledSource {
    compiled_source_with_policy(strategies, json!({ "type": "first_accepted" }))
}

fn compiled_source_with_policy(
    strategies: Vec<Value>,
    policy: Value,
) -> job_radar_lib::CompiledSource {
    let profile: SourceProfileDocument =
        serde_json::from_value(fixture_profile_value(strategies, policy)).unwrap();
    let source = fixture_source();
    match support::compile_test_source(&source, Some(profile)) {
        CompileSourceOutcome::Compiled {
            source: compiled,
            diagnostics,
        } if diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != job_radar_lib::DiagnosticSeverity::Error) =>
        {
            compiled
        }
        other => panic!("expected compiled fixture Source, got {other:?}"),
    }
}

fn fixture_profile_value(strategies: Vec<Value>, policy: Value) -> Value {
    json!({
        "schemaVersion": 3,
        "key": "fixture_profile",
        "name": "Fixture profile",
        "kind": "generic",
        "support": { "level": "experimental", "summary": "S01 fixture" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "default",
            "name": "Default",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "discovery",
                    "fetch": {
                        "mode": "http", "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}", "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "reference": {
                            "url": { "type": "json_path", "jsonPath": "$.url" }
                        },
                        "providerValues": {
                            "title": { "type": "json_path", "jsonPath": "$.title" },
                            "company": { "type": "json_path", "jsonPath": "$.company" }
                        }
                    }
                }]
            },
            "detail": {
                "policy": policy,
                "strategies": strategies
            }
        }]
    })
}

fn fixture_source() -> SourceDocument {
    serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "fixture_source",
        "name": "Fixture source",
        "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/feed" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "fixture_profile",
            "pathKey": "default"
        }
    }))
    .unwrap()
}

fn detail_strategy(key: &str, fields: Value) -> Value {
    json!({
        "key": key,
        "fetch": {
            "mode": "http", "method": "GET",
            "url": "https://example.test/detail", "timeoutMs": 10000
        },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": { "fields": fields }
    })
}

fn occurrence(
    source_key: &str,
    description_text: Option<&str>,
) -> job_radar_lib::PostingOccurrence {
    let (reference, identity) = job_radar_lib::validate_posting_reference(
        source_key,
        "https://example.test/jobs/42",
        Some("42".to_string()),
    )
    .unwrap();
    job_radar_lib::PostingOccurrence {
        identity,
        reference,
        provider_values: ProviderValues {
            title: Some("Occurrence title".to_string()),
            company: Some("Occurrence company".to_string()),
            locations: Vec::new(),
            description_text: description_text.map(str::to_string),
        },
        hints: Default::default(),
        posting_meta: Default::default(),
    }
}

struct AlwaysCancelled;

impl RuntimeCancellation for AlwaysCancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

fn response_event(body: Value) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/detail".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(body.to_string().into_bytes())],
        content_length: None,
    }
}

fn response_client(body: Value) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new([response_event(body)])
}
