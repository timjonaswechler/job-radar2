use std::sync::atomic::{AtomicBool, Ordering};

use job_radar_lib::{
    compile_source, AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource,
    BrowserAcquisitionFailure, BrowserAcquisitionFailureKind, BrowserAcquisitionRequestSnapshot,
    CompileSourceOutcome, DetailBrowserAdapter, DetailField, ExecutionPlanBrowserInteraction,
    ExecutionPlanBrowserWait, PhaseBrowser, PhaseCompletion, PhaseExecutionFailure,
    PhaseExecutionReport, PhaseLimits, PhaseOutcome, PhasePreStartFailure, PhaseRunError,
    PhaseUsage, PolicyOutcome, PostingOccurrence, PostingOccurrenceIdentity, PostingReference,
    ProviderValues, RegistrySourceProfile, RequestedDetailFields, RuntimeCancellation,
    RuntimeExecutionContext, ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument, SourceProfileRegistrySnapshot,
    __test_execute_detail_with_browser_adapter as execute_detail,
};
use serde_json::json;

fn compiled_plan(browser: bool) -> SourceExecutionPlan {
    let fetch = if browser {
        json!({
            "mode": "browser", "url": "{{posting:url}}", "timeoutMs": 30000,
            "waits": [{ "type": "selector", "selector": "main", "timeoutMs": 1000 }],
            "interactions": [{ "type": "click_until_gone", "selector": ".banner", "maxCount": 1 }]
        })
    } else {
        json!({ "mode": "http", "method": "GET", "url": "{{posting:url}}", "timeoutMs": 30000 })
    };
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "adapter", "name": "Adapter", "kind": "generic",
        "support": { "level": "experimental", "summary": "fixture" },
        "sourceConfigSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        "accessPaths": [{
            "key": "main", "name": "Main",
            "discovery": { "policy": { "type": "first_accepted" }, "strategies": [{
                "key": "discovery", "fetch": { "mode": "http", "method": "GET", "url": "https://example.test/discovery", "timeoutMs": 1000 },
                "parse": { "type": "json" }, "select": { "type": "json_path", "jsonPath": "$.jobs" },
                "extract": { "reference": { "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" } } }
            }]},
            "detail": {
                "policy": { "type": "first_accepted" },
                "acceptWhen": { "requiredFields": ["descriptionText"] },
                "strategies": [{
                    "key": "rendered", "fetch": fetch,
                    "parse": { "type": "html" }, "select": { "type": "css", "selector": "main" },
                    "extract": { "fields": { "descriptionText": { "type": "css_text", "selector": ".description", "cardinality": "one" } } }
                }]
            }
        }]
    })).unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "source", "name": "Source", "status": "active", "sourceConfig": {},
        "selectedAccessPath": { "type": "profile_access_path", "profileKey": "adapter", "pathKey": "main" }
    })).unwrap();
    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile,
        }],
        sources: vec![],
        diagnostics: vec![],
    };
    match compile_source(&source, &registry) {
        CompileSourceOutcome::Compiled { source, .. } => source.execution_plan,
        other => panic!("expected plan, got {other:?}"),
    }
}

fn posting() -> PostingOccurrence {
    PostingOccurrence {
        identity: PostingOccurrenceIdentity::ProviderPostingId {
            source_key: "source".into(),
            provider_posting_id: "1".into(),
        },
        reference: PostingReference {
            provider_url: "https://example.test/jobs/1".into(),
            provider_posting_id: Some("1".into()),
        },
        provider_values: ProviderValues::default(),
        hints: Default::default(),
        posting_meta: Default::default(),
    }
}

fn snapshot(remaining: u64) -> BrowserAcquisitionRequestSnapshot {
    BrowserAcquisitionRequestSnapshot {
        target: "https://example.test/jobs/1".into(),
        timeout_ms: 30000,
        waits: vec![ExecutionPlanBrowserWait::Selector {
            selector: "main".into(),
            timeout_ms: 1000,
        }],
        interactions: vec![ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector: ".banner".into(),
            max_count: 1,
            wait_after_ms: None,
        }],
        browser_rendered_bytes_remaining: remaining,
    }
}

fn html() -> String {
    "<main><section class=\"description\">Rendered detail.</section></main>".into()
}

struct Cancellation(AtomicBool);

impl RuntimeCancellation for Cancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn accepted_detail_maps_candidate_context_and_preserves_exact_report() {
    let plan = compiled_plan(true);
    let body = html();
    let mut limits = PhaseLimits::BACKEND;
    limits.max_browser_rendered_bytes = body.len() as u64;
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(body.len() as u64),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 0,
            },
            ScriptedBrowserAcquisitionEvent::Content(body.clone()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::new([DetailField::DescriptionText]).unwrap(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable().with_limits(limits),
    )
    .await
    .unwrap();
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::Accepted { reduced_payload },
        complete_budget_report,
        diagnostics,
    } = result
    else {
        panic!("expected accepted")
    };
    assert_eq!(
        reduced_payload.patch.description_text.as_deref(),
        Some("Rendered detail.")
    );
    assert!(diagnostics.is_empty());
    assert_eq!(
        complete_budget_report,
        PhaseExecutionReport {
            usage: PhaseUsage {
                strategy_attempts: 1,
                requests: 1,
                produced_items: 1,
                duration_ms: complete_budget_report.usage.duration_ms,
                pages: 0,
                browser_actions: 0,
                fan_out: 0,
                response_bytes: 0,
                browser_rendered_bytes: body.len() as u64,
            },
            completion: PhaseCompletion::Accepted,
        }
    );
    assert!(acquisition.expectations_satisfied());
}

#[tokio::test]
async fn infrastructure_failure_is_phase_fatal_and_exposes_no_patch() {
    let plan = compiled_plan(true);
    let body = html();
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 0,
            },
            ScriptedBrowserAcquisitionEvent::Content(body),
        ],
        finalization: ScriptedBrowserFinalization::InfrastructureFailure {
            message: "private teardown path token=secret".into(),
        },
    }]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    .unwrap();
    let PhaseOutcome::ExecutionFailed {
        typed_failure,
        diagnostics,
        complete_budget_report,
    } = result
    else {
        panic!("expected fatal")
    };
    assert_eq!(typed_failure, PhaseExecutionFailure::Internal);
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::ExecutionFailed
    );
    assert_eq!(diagnostics[0].code, "browser_infrastructure_failure");
    let serialized = serde_json::to_string(&(diagnostics, complete_budget_report)).unwrap();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("\"patch\":"));
}

#[tokio::test]
async fn ordinary_failure_is_policy_attempt_failure_without_private_message() {
    let plan = compiled_plan(true);
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![ScriptedBrowserAcquisitionEvent::Failure(
            BrowserAcquisitionFailure::new(
                BrowserAcquisitionFailureKind::Navigation,
                "private URL token=secret",
            ),
        )],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    .unwrap();
    let PhaseOutcome::Completed { diagnostics, .. } = result else {
        panic!("ordinary failure must remain attempt-scoped")
    };
    assert_eq!(diagnostics[0].code, "browser_navigation_failed");
    assert!(!serde_json::to_string(&diagnostics)
        .unwrap()
        .contains("secret"));
}

#[tokio::test]
async fn browser_free_detail_executes_http_with_zero_browser_calls_and_mismatch_is_prestart() {
    let plan = compiled_plan(false);
    let body = html().into_bytes();
    let http = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs/1".into(),
        headers: vec![("content-type".into(), b"text/html; charset=utf-8".to_vec())],
        content_length: Some(body.len() as u64),
        body: vec![ScriptedHttpBodyEvent::Chunk(body)],
    }]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &http,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )
    .await;
    assert!(matches!(
        result,
        Ok(PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::Accepted { .. },
            ..
        })
    ));

    let empty = ScriptedBrowserAcquisition::new([]);
    let mismatch = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &http,
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&empty)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await;
    assert!(matches!(
        mismatch,
        Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            ..
        })
    ));
    assert!(empty.requests().is_empty());

    let browser_plan = compiled_plan(true);
    let missing = execute_detail(
        &browser_plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &http,
        PhaseBrowser::BrowserFree,
        RuntimeExecutionContext::uncancellable(),
    )
    .await;
    assert!(matches!(
        missing,
        Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            ..
        })
    ));
}

#[tokio::test]
async fn rendered_detail_is_not_a_patch_until_acceptance() {
    let plan = compiled_plan(true);
    let body = "<main></main>".to_string();
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 0,
            },
            ScriptedBrowserAcquisitionEvent::Content(body.clone()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    .unwrap();
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { .. },
        complete_budget_report,
        ..
    } = result
    else {
        panic!("rendered input must remain payload-free before acceptance")
    };
    assert_eq!(
        complete_budget_report.usage.browser_rendered_bytes,
        body.len() as u64
    );
}

#[tokio::test]
async fn cumulative_detail_one_over_consumes_remaining_and_exposes_no_patch() {
    let mut plan = compiled_plan(true);
    let mut fallback = plan.detail.as_ref().unwrap().strategies[0].clone();
    fallback.key = "fallback".into();
    plan.detail.as_mut().unwrap().strategies.push(fallback);
    let first = "<main></main>".to_string();
    let second = "12345".to_string();
    let mut limits = PhaseLimits::BACKEND;
    limits.max_browser_rendered_bytes = first.len() as u64 + 2;
    let acquisition = ScriptedBrowserAcquisition::new([
        ScriptedBrowserAcquisitionExpectation {
            request: snapshot(limits.max_browser_rendered_bytes),
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
                ScriptedBrowserAcquisitionEvent::Interaction {
                    interaction_index: 0,
                    attempted_clicks: 0,
                },
                ScriptedBrowserAcquisitionEvent::Content(first),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        },
        ScriptedBrowserAcquisitionExpectation {
            request: snapshot(2),
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
                ScriptedBrowserAcquisitionEvent::Interaction {
                    interaction_index: 0,
                    attempted_clicks: 0,
                },
                ScriptedBrowserAcquisitionEvent::Content(second),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        },
    ]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable().with_limits(limits),
    )
    .await
    .unwrap();
    let PhaseOutcome::BudgetExhausted {
        complete_budget_report,
        diagnostics,
    } = result
    else {
        panic!("expected Browser byte exhaustion")
    };
    assert_eq!(
        complete_budget_report,
        PhaseExecutionReport {
            usage: PhaseUsage {
                strategy_attempts: 2,
                requests: 2,
                produced_items: 0,
                duration_ms: complete_budget_report.usage.duration_ms,
                pages: 0,
                browser_actions: 0,
                fan_out: 0,
                response_bytes: 0,
                browser_rendered_bytes: limits.max_browser_rendered_bytes,
            },
            completion: PhaseCompletion::BudgetExhausted {
                exhaustion: AllowanceExhaustion {
                    dimension: AllowanceDimension::BrowserRenderedBytes,
                    requested: 3,
                    remaining: 0,
                    limit_sources: vec![AllowanceLimitSource::Caller],
                },
            },
        }
    );
    let serialized = serde_json::to_string(&(complete_budget_report, diagnostics)).unwrap();
    assert!(!serialized.contains("reducedPayload"));
    assert!(!serialized.contains("12345"));
}

#[tokio::test]
async fn cumulative_request_exhaustion_is_payload_free_with_ordered_diagnostics() {
    let mut plan = compiled_plan(true);
    let mut fallback = plan.detail.as_ref().unwrap().strategies[0].clone();
    fallback.key = "fallback".into();
    plan.detail.as_mut().unwrap().strategies.push(fallback);
    let first = "<main></main>".to_string();
    let mut limits = PhaseLimits::BACKEND;
    limits.max_requests = 1;
    let acquisition = ScriptedBrowserAcquisition::new([
        ScriptedBrowserAcquisitionExpectation {
            request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
                ScriptedBrowserAcquisitionEvent::Interaction {
                    interaction_index: 0,
                    attempted_clicks: 0,
                },
                ScriptedBrowserAcquisitionEvent::Content(first.clone()),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        },
        ScriptedBrowserAcquisitionExpectation {
            request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes - first.len() as u64),
            events: vec![ScriptedBrowserAcquisitionEvent::Navigate],
            finalization: ScriptedBrowserFinalization::default(),
        },
    ]);
    let result = execute_detail(
        &plan,
        &Default::default(),
        &posting(),
        RequestedDetailFields::description_text(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable().with_limits(limits),
    )
    .await
    .unwrap();
    let PhaseOutcome::BudgetExhausted {
        complete_budget_report,
        diagnostics,
    } = result
    else {
        panic!("expected request exhaustion")
    };
    assert_eq!(
        complete_budget_report,
        PhaseExecutionReport {
            usage: PhaseUsage {
                strategy_attempts: 2,
                requests: 1,
                produced_items: 0,
                duration_ms: complete_budget_report.usage.duration_ms,
                pages: 0,
                browser_actions: 0,
                fan_out: 0,
                response_bytes: 0,
                browser_rendered_bytes: first.len() as u64,
            },
            completion: PhaseCompletion::BudgetExhausted {
                exhaustion: AllowanceExhaustion {
                    dimension: AllowanceDimension::Requests,
                    requested: 1,
                    remaining: 0,
                    limit_sources: vec![AllowanceLimitSource::Caller],
                },
            },
        }
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "acceptance_required_field_missing",
            "phase_allowance_exhausted",
        ]
    );
    let serialized = serde_json::to_string(&(complete_budget_report, diagnostics)).unwrap();
    assert!(!serialized.contains("\"patch\":"));
    assert!(!serialized.contains("reducedPayload"));
    assert!(!serialized.contains("<main>"));
    assert!(acquisition.expectations_satisfied());
}

#[tokio::test]
async fn active_detail_acquisition_cancellation_stays_outside_phase_outcome() {
    let plan = compiled_plan(true);
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Gate("cancel-detail".into()),
            ScriptedBrowserAcquisitionEvent::Content(html()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let cancellation = Cancellation(AtomicBool::new(false));
    let posting = posting();
    let source_config = Default::default();
    let http = ScriptedProfileHttpClient::new([]);
    let execute = execute_detail(
        &plan,
        &source_config,
        &posting,
        RequestedDetailFields::description_text(),
        &http,
        PhaseBrowser::Browser(DetailBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::with_cancellation(&cancellation),
    );
    let cancel = async {
        while !acquisition.gate_is_waiting("cancel-detail") {
            tokio::task::yield_now().await;
        }
        cancellation.0.store(true, Ordering::SeqCst);
    };
    let (result, _) = tokio::join!(execute, cancel);
    let Err(PhaseRunError::Cancelled(cancelled)) = result else {
        panic!("expected typed Detail cancellation")
    };
    assert!(matches!(
        cancelled.complete_budget_report.completion,
        PhaseCompletion::Cancelled { .. }
    ));
    assert_eq!(
        cancelled.diagnostics.last().unwrap().code,
        "runtime_execution_cancelled"
    );
}
