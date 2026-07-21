use crate::support::{accepted_phase, budget_exhausted, cancelled, execution_failed, not_started};
use std::{collections::BTreeMap, future::Future};

fn empty_source_config() -> &'static serde_json::Map<String, serde_json::Value> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, serde_json::Value>> =
        std::sync::OnceLock::new();
    EMPTY.get_or_init(serde_json::Map::new)
}
use job_radar_lib::{
    compile_source, execute_detail, execute_discovery, AllowanceDimension, CompileSourceOutcome,
    DiscoveryStep, ExecutionPlanFetch, PhaseCompletion, PhaseLimits, PhaseOutcome, PolicyOutcome,
    PolicyUnsatisfiedCause, PostingOccurrence, ProfileHttpFailureKind, RegistrySourceProfile,
    RequestedDetailFields, RuntimeCancellation, RuntimeExecutionContext, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument, SourceProfileRegistrySnapshot, StrategyPolicy,
    UnavailableProfileBrowserClient,
};
use serde_json::{json, Value};

#[test]
fn final_strategy_set_requires_the_closed_first_accepted_policy() {
    let strategy_set = json!({ "strategies": [] });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("a final Strategy Set without Policy must be rejected");

    let strategy_set = json!({ "policy": "first_accepted", "strategies": [] });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("a raw string Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "unknown" },
        "strategies": []
    });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("an unknown Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "first_accepted", "extra": true },
        "strategies": []
    });
    serde_json::from_value::<DiscoveryStep>(strategy_set)
        .expect_err("additional Policy properties must be rejected");

    let strategy_set: DiscoveryStep = serde_json::from_value(json!({
        "policy": { "type": "first_accepted" },
        "strategies": []
    }))
    .unwrap();
    assert_eq!(strategy_set.policy, StrategyPolicy::FirstAccepted);
    assert_eq!(
        serde_json::to_value(strategy_set.policy).unwrap(),
        json!({ "type": "first_accepted" })
    );
}

#[test]
fn final_compiler_preserves_policy_for_inherited_specialized_added_and_source_owned_sets() {
    let profile = profile_document();

    let inherited = compile(profile_source(None, "main"), profile.clone());
    assert_plan_policies(&inherited);

    let specialized = compile(
        profile_source(
            Some(json!([{
                "key": "main",
                "discovery": {
                    "policy": { "type": "first_accepted" },
                    "strategies": [discovery_strategy(
                        "source_added",
                        "https://example.test/discovery/source-added"
                    )]
                },
                "detail": { "policy": { "type": "first_accepted" } }
            }])),
            "main",
        ),
        profile.clone(),
    );
    assert_plan_policies(&specialized);
    assert_eq!(
        specialized.discovery.strategies.len(),
        4,
        "a complete Source-added Strategy must retain the inherited Policy"
    );

    let added_path = json!({
        "key": "added",
        "name": "Added path",
        "discovery": discovery_step(),
        "detail": detail_step()
    });
    let added = compile(profile_source(Some(json!([added_path])), "added"), profile);
    assert_plan_policies(&added);

    let source_owned: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "owned",
        "name": "Owned",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "owned_path",
            "name": "Owned path",
            "discovery": discovery_step(),
            "detail": detail_step()
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Deterministic test source."
        }
    }))
    .unwrap();
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(&source_owned, &SourceProfileRegistrySnapshot::default())
    else {
        panic!("valid Source-owned final path must compile");
    };
    assert!(diagnostics.is_empty());
    assert!(matches!(
        source.access,
        job_radar_lib::CompiledSourceAccess::SourceOwned { .. }
    ));
    assert_plan_policies(&source.execution_plan);
}

#[test]
fn first_accepted_execution_is_ordered_and_recovers_for_both_phases() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({
                "jobs": [{
                    "title": "Rejected partial output",
                    "company": "Example",
                    "url": "https://example.test/jobs/rejected"
                }]
            })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({
                "jobs": [{
                    "title": "Platform Engineer",
                    "company": "Example",
                    "url": "https://example.test/jobs/1"
                }]
            })
            .to_string()),
        ),
    ]);

    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Platform Engineer"
    );
    let report = &result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(report.usage.produced_items, 2);
    assert_eq!(
        discovery.requests(),
        vec![
            "https://example.test/discovery/empty",
            "https://example.test/discovery/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["acceptance_min_results_not_met",]
    );

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "Rejected partial detail." }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "A complete accepted description." }).to_string()),
        ),
    ]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )));

    assert_eq!(
        result.payload.patch.description_text.as_deref(),
        Some("A complete accepted description.")
    );
    let report = &result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(report.usage.produced_items, 1);
    assert_eq!(
        detail.requests(),
        vec![
            "https://example.test/detail/failed",
            "https://example.test/detail/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["description_too_short",]
    );
}

#[test]
fn first_accepted_execution_stops_after_an_accepted_first_attempt() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([(
        "https://example.test/discovery/empty",
        Ok(json!({
            "jobs": [
                {
                    "title": "First accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/first"
                },
                {
                    "title": "Also accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/second"
                }
            ]
        })
        .to_string()),
    )]);
    let result = accepted_phase(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(discovery.requests().len(), 1);
    assert!(result.diagnostics.is_empty());

    let accepted_description = "accepted ".repeat(20);
    let detail = DetailScriptedClient::new([(
        "https://example.test/detail/failed",
        Ok(json!({ "description": accepted_description }).to_string()),
    )]);
    let result = accepted_phase(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    )));
    assert!(result.payload.patch.description_text.is_some());
    assert_eq!(detail.requests().len(), 1);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn first_accepted_exhaustion_adds_one_terminal_after_attempt_diagnostics() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = DiscoveryScriptedClient::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
    ]);

    let result = block_on(execute_discovery(
        &plan,
        empty_source_config(),
        discovery.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.expect("phase completes without control-flow error")
    else {
        panic!("expected payload-free PolicyUnsatisfied completion")
    };

    assert_eq!(cause, PolicyUnsatisfiedCause::RejectedOnly);
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::PolicyUnsatisfied
    );
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert!(!serde_json::to_string(
        &PolicyOutcome::<job_radar_lib::DiscoveryPhasePayload>::PolicyUnsatisfied {
            cause: cause.clone()
        }
    )
    .unwrap()
    .contains("candidates"));
    let codes = diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "fallback_exhausted",
        ]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/discovery/strategies");

    let detail = DetailScriptedClient::new([
        (
            "https://example.test/detail/failed",
            Err("failed one".to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("failed two".to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Err("failed three".to_string()),
        ),
    ]);
    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));
    let PhaseOutcome::Completed {
        policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
        complete_budget_report,
        diagnostics,
    } = result.expect("phase completes without control-flow error")
    else {
        panic!("expected payload-free PolicyUnsatisfied completion")
    };
    assert_eq!(cause, PolicyUnsatisfiedCause::IncludesExecutionFailure);
    assert_eq!(
        complete_budget_report.completion,
        PhaseCompletion::PolicyUnsatisfied
    );
    assert_eq!(complete_budget_report.usage.strategy_attempts, 3);
    assert_eq!(complete_budget_report.usage.produced_items, 0);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "fetch_failed",
            "fetch_failed",
            "fetch_failed",
            "fallback_exhausted"
        ]
    );
    assert_eq!(diagnostics.last().unwrap().path, "/detail/strategies");
}

#[test]
fn detail_request_one_over_is_budget_exhausted_with_no_patch() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let detail = DetailScriptedClient::new([(
        "https://example.test/detail/failed",
        Err("first failed".to_string()),
    )]);
    let caller = PhaseLimits {
        max_requests: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        detail.client(),
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable().with_limits(caller),
    )));

    let report = result.report;
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("expected Detail budget exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
    assert_eq!(report.usage.strategy_attempts, 2);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(report.usage.produced_items, 0);
    assert_eq!(detail.requests().len(), 1);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

#[test]
fn detail_browser_1999_ms_compiled_and_caller_limits_are_rejected_without_panic() {
    let mut plan = compile(profile_source(None, "main"), profile_document());
    let detail_plan = plan.detail.as_mut().expect("fixture has Detail");
    detail_plan.strategies[0].fetch = ExecutionPlanFetch::Browser {
        url: job_radar_lib::compile_template(
            "https://example.test/detail/browser",
            &job_radar_lib::TemplateDescriptor::new(),
        )
        .unwrap(),
        timeout_ms: 1_000,
        waits: Vec::new(),
        interactions: Vec::new(),
    };
    detail_plan.limits.max_duration_ms = 1_999;
    let detail = DetailScriptedClient::new([]);

    let invalid_plan_diagnostics = not_started(
        block_on(execute_detail(
            &plan,
            empty_source_config(),
            &posting(),
            RequestedDetailFields::description_text(),
            detail.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable(),
        )),
        job_radar_lib::PhasePreStartFailure::PlanMismatch,
    );

    assert!(invalid_plan_diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "invalid_compiled_browser_phase_duration"
            && diagnostic.path == "/detail/limits/maxDurationMs"
    }));

    plan.detail.as_mut().unwrap().limits.max_duration_ms = PhaseLimits::BACKEND.max_duration_ms;
    let caller = PhaseLimits {
        max_duration_ms: 1_999,
        ..PhaseLimits::BACKEND
    };
    let caller_result = execution_failed(
        block_on(execute_detail(
            &plan,
            empty_source_config(),
            &posting(),
            RequestedDetailFields::description_text(),
            detail.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )),
        job_radar_lib::PhaseExecutionFailure::InvalidCallerLimits,
    );

    let report = caller_result.report;
    assert_eq!(report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage, Default::default());
    assert!(caller_result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_caller_phase_limits"));
    assert!(detail.requests().is_empty());
}

#[test]
fn cancellation_discards_an_accepted_attempt_and_suppresses_later_work_and_exhaustion() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/discovery/empty".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "jobs": [] }).to_string().into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };

    let result = cancelled(block_on(execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));

    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "fallback_exhausted"));

    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/detail/failed".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "description": "Discarded detail description." })
                .to_string()
                .into_bytes(),
        )],
        content_length: None,
    }]);
    let signal = RequestObservedCancellation { client: &fetcher };
    let result = cancelled(block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting(),
        RequestedDetailFields::description_text(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::with_cancellation(&signal),
    )));
    assert_eq!(fetcher.request_count(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

fn assert_plan_policies(plan: &SourceExecutionPlan) {
    assert_eq!(plan.discovery.policy, StrategyPolicy::FirstAccepted);
    assert_eq!(
        plan.detail.as_ref().unwrap().policy,
        StrategyPolicy::FirstAccepted
    );
}

fn compile(source: SourceDocument, profile: SourceProfileDocument) -> SourceExecutionPlan {
    let outcome = compile_source(
        &source,
        &SourceProfileRegistrySnapshot {
            profiles: vec![RegistrySourceProfile {
                origin: "test".into(),
                path: String::new(),
                document: profile,
            }],
            sources: Vec::new(),
            diagnostics: Vec::new(),
        },
    );
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = outcome
    else {
        panic!("valid final source must compile: {outcome:?}");
    };
    assert!(diagnostics.is_empty());
    source.execution_plan
}

fn profile_document() -> SourceProfileDocument {
    serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "policy_profile",
        "name": "Policy profile",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Deterministic policy test profile."
        },
        "accessPaths": [{
            "key": "main",
            "name": "Main",
            "discovery": discovery_step(),
            "detail": detail_step()
        }]
    }))
    .unwrap()
}

fn profile_source(access_paths: Option<Value>, path_key: &str) -> SourceDocument {
    let mut value = json!({
        "schemaVersion": 3,
        "key": "policy_source",
        "name": "Policy source",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "policy_profile",
            "pathKey": path_key
        }
    });
    if let Some(access_paths) = access_paths {
        value["accessPaths"] = access_paths;
    }
    serde_json::from_value(value).unwrap()
}

fn discovery_step() -> Value {
    let mut rejected = discovery_strategy("empty", "https://example.test/discovery/empty");
    rejected["acceptWhen"] = json!({ "minResults": 2 });
    json!({
        "policy": { "type": "first_accepted" },
        "acceptWhen": { "minResults": 1 },
        "strategies": [
            rejected,
            discovery_strategy("accepted", "https://example.test/discovery/accepted"),
            discovery_strategy("unused", "https://example.test/discovery/unused")
        ]
    })
}

fn discovery_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
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
    })
}

fn detail_step() -> Value {
    let mut rejected = detail_strategy("failed", "https://example.test/detail/failed");
    rejected["acceptWhen"] = json!({ "minDescriptionLength": 100 });
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [
            rejected,
            detail_strategy("accepted", "https://example.test/detail/accepted"),
            detail_strategy("unused", "https://example.test/detail/unused")
        ]
    })
}

fn detail_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
            "fields": {
                "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
            }
        }
    })
}

fn posting() -> PostingOccurrence {
    let (reference, identity) =
        job_radar_lib::validate_posting_reference("example", "https://example.test/jobs/1", None)
            .unwrap();
    PostingOccurrence {
        identity,
        reference,
        provider_values: Default::default(),
        hints: Default::default(),
        posting_meta: BTreeMap::new(),
    }
}

struct DiscoveryScriptedClient {
    client: ScriptedProfileHttpClient,
}

impl DiscoveryScriptedClient {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            client: scripted_client(entries),
        }
    }

    fn client(&self) -> &ScriptedProfileHttpClient {
        &self.client
    }

    fn requests(&self) -> Vec<String> {
        self.client
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect()
    }
}

struct DetailScriptedClient {
    client: ScriptedProfileHttpClient,
}

impl DetailScriptedClient {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            client: scripted_client(entries),
        }
    }

    fn client(&self) -> &ScriptedProfileHttpClient {
        &self.client
    }

    fn requests(&self) -> Vec<String> {
        self.client
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect()
    }
}

fn scripted_client<const N: usize>(
    entries: [(&str, Result<String, String>); N],
) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new(entries.into_iter().map(|(url, result)| {
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: url.to_string(),
            headers: Vec::new(),
            body: vec![match result {
                Ok(body) => ScriptedHttpBodyEvent::Chunk(body.into_bytes()),
                Err(_) => ScriptedHttpBodyEvent::Failure(ProfileHttpFailureKind::BodyStream),
            }],
            content_length: None,
        }
    }))
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

struct RequestObservedCancellation<'a> {
    client: &'a ScriptedProfileHttpClient,
}

impl RuntimeCancellation for RequestObservedCancellation<'_> {
    fn is_cancelled(&self) -> bool {
        self.client.request_count() > 0
    }
}
