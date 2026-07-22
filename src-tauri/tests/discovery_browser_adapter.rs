use std::sync::atomic::{AtomicBool, Ordering};

use job_radar_lib::{
    compile_source, execute_discovery_with_browser_adapter, AllowanceDimension,
    AllowanceExhaustion, AllowanceLimitSource, BrowserAcquisitionFailure,
    BrowserAcquisitionFailureKind, BrowserAcquisitionRequestSnapshot, CompileSourceOutcome,
    DiscoveryBrowserAdapter, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
    PhaseBrowser, PhaseCompletion, PhaseExecutionReport, PhaseLimits, PhaseOutcome,
    PhasePreStartFailure, PhaseRunError, PhaseUsage, PolicyOutcome, RegistrySourceProfile,
    RuntimeCancellation, RuntimeExecutionContext, ScriptedBrowserAcquisition,
    ScriptedBrowserAcquisitionEvent, ScriptedBrowserAcquisitionExpectation,
    ScriptedBrowserFinalization, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
    SourceProfileRegistrySnapshot,
};
use serde_json::json;

fn compiled_plan(browser: bool) -> SourceExecutionPlan {
    let fetch = if browser {
        json!({
            "mode": "browser", "url": "{{sourceConfig:startUrl}}", "timeoutMs": 30000,
            "waits": [{ "type": "selector", "selector": "main", "timeoutMs": 1000 }],
            "interactions": [{ "type": "click_if_visible", "selector": ".more", "maxCount": 1 }]
        })
    } else {
        json!({ "mode": "http", "method": "GET", "url": "{{sourceConfig:startUrl}}", "timeoutMs": 30000 })
    };
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "adapter", "name": "Adapter", "kind": "generic",
        "support": { "level": "experimental", "summary": "fixture" },
        "sourceConfigSchema": {
            "type": "object", "required": ["startUrl"],
            "properties": { "startUrl": { "type": "string" } }, "additionalProperties": false
        },
        "accessPaths": [{
            "key": "main", "name": "Main",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "acceptWhen": { "minResults": 1 },
                "strategies": [{
                    "key": "rendered", "fetch": fetch,
                    "parse": { "type": "html" },
                    "select": { "type": "css", "selector": "article" },
                    "extract": {
                        "reference": { "url": { "type": "css_attribute", "selector": "a", "attribute": "href", "cardinality": "one" } },
                        "providerValues": {
                            "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
                            "company": { "type": "css_text", "selector": ".company", "cardinality": "one" }
                        }
                    }
                }]
            }
        }]
    })).unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3, "key": "source", "name": "Source", "status": "active",
        "sourceConfig": { "startUrl": "https://example.test/jobs" },
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

fn snapshot(remaining: u64) -> BrowserAcquisitionRequestSnapshot {
    BrowserAcquisitionRequestSnapshot {
        target: "https://example.test/jobs".into(),
        timeout_ms: 30000,
        waits: vec![ExecutionPlanBrowserWait::Selector {
            selector: "main".into(),
            timeout_ms: 1000,
        }],
        interactions: vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: ".more".into(),
            max_count: 1,
            wait_after_ms: None,
        }],
        browser_rendered_bytes_remaining: remaining,
    }
}

struct Cancellation(AtomicBool);
impl RuntimeCancellation for Cancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

fn html() -> String {
    "<main><article><span class=\"title\">Engineer</span><span class=\"company\">Example</span><a href=\"https://example.test/jobs/1\"></a></article></main>".into()
}

#[tokio::test]
async fn rendered_content_is_only_parser_input_before_acceptance() {
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
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await;
    let Ok(PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { .. },
        complete_budget_report,
        ..
    }) = result
    else {
        panic!("render success must not accept: {result:?}");
    };
    assert_eq!(
        complete_budget_report.usage.browser_rendered_bytes,
        body.len() as u64
    );
    assert_eq!(complete_budget_report.usage.response_bytes, 0);
    assert!(acquisition.expectations_satisfied());
}

#[tokio::test]
async fn accepted_discovery_preserves_exact_shared_browser_report() {
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
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
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
    assert_eq!(reduced_payload.candidates.len(), 1);
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
    assert_eq!(
        serde_json::to_value(&complete_budget_report).unwrap()["usage"]["browserRenderedBytes"],
        body.len() as u64
    );
}

#[tokio::test]
async fn ordinary_failure_is_attempt_failure_and_diagnostic_is_payload_safe() {
    let plan = compiled_plan(true);
    let secret = "secret query https://example.test/jobs?token=abc";
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![ScriptedBrowserAcquisitionEvent::Failure(
            BrowserAcquisitionFailure::new(BrowserAcquisitionFailureKind::RuntimeLaunch, secret),
        )],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    .unwrap();
    let PhaseOutcome::Completed { diagnostics, .. } = result else {
        panic!("ordinary failure is policy-unsatisfied")
    };
    assert_eq!(diagnostics[0].code, "browser_runtime_unavailable");
    assert!(!serde_json::to_string(&diagnostics)
        .unwrap()
        .contains("token"));
}

#[tokio::test]
async fn cumulative_one_over_consumes_remaining_browser_bytes_and_exposes_no_payload() {
    let mut plan = compiled_plan(true);
    let mut fallback = plan.discovery.strategies[0].clone();
    fallback.key = "fallback".into();
    plan.discovery.strategies.push(fallback);
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
                ScriptedBrowserAcquisitionEvent::Content(first.clone()),
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
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable().with_limits(limits),
    )
    .await
    .unwrap();
    let PhaseOutcome::BudgetExhausted {
        complete_budget_report,
        diagnostics,
    } = result
    else {
        panic!("expected budget")
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
    assert!(!serialized.contains("candidates"));
    assert!(!serialized.contains("12345"));
}

#[tokio::test]
async fn cumulative_request_exhaustion_is_payload_free_with_ordered_diagnostics() {
    let mut plan = compiled_plan(true);
    let mut fallback = plan.discovery.strategies[0].clone();
    fallback.key = "fallback".into();
    plan.discovery.strategies.push(fallback);
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
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
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
        vec!["css_select_missing", "phase_allowance_exhausted"]
    );
    let serialized = serde_json::to_string(&(complete_budget_report, diagnostics)).unwrap();
    assert!(!serialized.contains("candidates"));
    assert!(!serialized.contains("reducedPayload"));
    assert!(!serialized.contains("<main>"));
    assert!(acquisition.expectations_satisfied());
}

#[tokio::test]
async fn discovery_infrastructure_failure_is_phase_fatal_and_payload_safe() {
    let plan = compiled_plan(true);
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 0,
            },
            ScriptedBrowserAcquisitionEvent::Content(html()),
        ],
        finalization: ScriptedBrowserFinalization::InfrastructureFailure {
            message: "private teardown token=secret".into(),
        },
    }]);
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::uncancellable(),
    )
    .await
    .unwrap();
    let PhaseOutcome::ExecutionFailed {
        complete_budget_report,
        diagnostics,
        ..
    } = result
    else {
        panic!("expected phase-fatal infrastructure failure")
    };
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::ExecutionFailed
    );
    assert_eq!(diagnostics[0].code, "browser_infrastructure_failure");
    let serialized = serde_json::to_string(&(complete_budget_report, diagnostics)).unwrap();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("candidates"));
}

#[tokio::test]
async fn active_acquisition_cancellation_projects_outside_ordinary_outcome() {
    let plan = compiled_plan(true);
    let acquisition = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(PhaseLimits::BACKEND.max_browser_rendered_bytes),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Gate("cancel".into()),
            ScriptedBrowserAcquisitionEvent::Content(html()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let cancellation = Cancellation(AtomicBool::new(false));
    let source_config = json!({ "startUrl": "https://example.test/jobs" })
        .as_object()
        .unwrap()
        .clone();
    let http = ScriptedProfileHttpClient::new([]);
    let execute = execute_discovery_with_browser_adapter(
        &plan,
        &source_config,
        &http,
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&acquisition)),
        RuntimeExecutionContext::with_cancellation(&cancellation),
    );
    let cancel = async {
        while !acquisition.gate_is_waiting("cancel") {
            tokio::task::yield_now().await;
        }
        cancellation.0.store(true, Ordering::SeqCst);
    };
    let (result, _) = tokio::join!(execute, cancel);
    let Err(PhaseRunError::Cancelled(cancelled)) = result else {
        panic!("expected cancellation")
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

#[tokio::test]
async fn browser_free_construction_has_no_dependency_or_acquisition_call() {
    let plan = compiled_plan(false);
    let body = html().into_bytes();
    let http = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs".into(),
        headers: vec![("content-type".into(), b"text/html; charset=utf-8".to_vec())],
        content_length: Some(body.len() as u64),
        body: vec![ScriptedHttpBodyEvent::Chunk(body)],
    }]);
    let result = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
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
    let mismatch = execute_discovery_with_browser_adapter(
        &plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
        &http,
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(&empty)),
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
    let missing = execute_discovery_with_browser_adapter(
        &browser_plan,
        &json!({ "startUrl": "https://example.test/jobs" })
            .as_object()
            .unwrap()
            .clone(),
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
