use crate::support::{accepted_phase, budget_exhausted, cancelled, execution_failed, not_started};
use std::sync::atomic::{AtomicUsize, Ordering};

fn empty_source_config() -> &'static serde_json::Map<String, serde_json::Value> {
    static CONFIG: std::sync::OnceLock<serde_json::Map<String, serde_json::Value>> =
        std::sync::OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_value(json!({
            "feedUrl": "https://example.test/jobs.json",
            "baseUrl": "https://example.test",
            "sitemapUrl": "https://example.test/sitemap.xml"
        }))
        .unwrap()
    })
}
use job_radar_lib::{
    execute_discovery, AllowanceDimension, CompileSourceOutcome, CompiledPagination,
    ExecutionPlanFetch, PhaseCancellationReason, PhaseCompletion, PhaseLimits, PhaseLimitsFragment,
    ProfileHttpFailureKind, RegistrySourceProfile, RuntimeCancellation, RuntimeExecutionContext,
    ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient, SourceDocument,
    SourceProfileDocument, SourceProfileRegistrySnapshot, UnavailableProfileBrowserClient,
};
use serde_json::{json, Value};

struct QueueFetcher {
    client: ScriptedProfileHttpClient,
}

impl QueueFetcher {
    fn new(bodies: impl IntoIterator<Item = Value>) -> Self {
        Self::new_raw(bodies.into_iter().map(|value| value.to_string()))
    }

    fn new_raw(bodies: impl IntoIterator<Item = String>) -> Self {
        let events = bodies.into_iter().map(scripted_text_response);
        Self {
            client: ScriptedProfileHttpClient::new(events),
        }
    }

    fn client(&self) -> &ScriptedProfileHttpClient {
        &self.client
    }

    fn request_count(&self) -> usize {
        self.client.request_count()
    }
}

fn scripted_text_response(body: String) -> ScriptedHttpEvent {
    ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/fixture".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(body.into_bytes())],
        content_length: None,
    }
}

struct CancelsOnCheck {
    checks: AtomicUsize,
    cancel_on: usize,
}

impl RuntimeCancellation for CancelsOnCheck {
    fn is_cancelled(&self) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst) + 1 >= self.cancel_on
    }
}

#[test]
fn typed_limit_fragment_rejects_empty_null_and_unknown_shapes() {
    assert!(serde_json::from_value::<PhaseLimitsFragment>(json!({})).is_err());
    assert!(serde_json::from_value::<PhaseLimitsFragment>(json!({ "maxRequests": null })).is_err());
    assert!(serde_json::from_value::<PhaseLimitsFragment>(
        json!({ "maxBrowserRenderedBytes": null })
    )
    .is_err());
    assert!(serde_json::from_value::<PhaseLimitsFragment>(json!({ "unknownLimit": 1 })).is_err());
    assert_eq!(
        serde_json::from_value::<PhaseLimitsFragment>(json!({ "maxRequests": 3 }))
            .unwrap()
            .max_requests,
        Some(3)
    );
    assert_eq!(
        serde_json::from_value::<PhaseLimitsFragment>(json!({ "maxBrowserRenderedBytes": 1024 }))
            .unwrap()
            .max_browser_rendered_bytes,
        Some(1024)
    );
}

#[tokio::test]
async fn browser_compiled_plan_with_1999_ms_is_rejected_as_plan_mismatch_without_panic() {
    let mut plan = plan();
    plan.discovery.strategies[0].fetch = ExecutionPlanFetch::Browser {
        url: job_radar_lib::compile_template(
            "https://example.test/jobs",
            &job_radar_lib::TemplateDescriptor::new(),
        )
        .unwrap(),
        timeout_ms: 1_000,
        waits: Vec::new(),
        interactions: Vec::new(),
    };
    plan.discovery.limits.max_duration_ms = 1_999;
    let fetcher = QueueFetcher::new([]);

    let diagnostics = not_started(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
        job_radar_lib::PhasePreStartFailure::PlanMismatch,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "invalid_compiled_browser_phase_duration"
            && diagnostic.path == "/discovery/limits/maxDurationMs"
    }));
    assert!(fetcher.request_count() == 0);
}

#[tokio::test]
async fn browser_caller_tightening_to_1999_ms_is_execution_failed_without_panic() {
    let mut plan = plan();
    plan.discovery.strategies[0].fetch = ExecutionPlanFetch::Browser {
        url: job_radar_lib::compile_template(
            "https://example.test/jobs",
            &job_radar_lib::TemplateDescriptor::new(),
        )
        .unwrap(),
        timeout_ms: 1_000,
        waits: Vec::new(),
        interactions: Vec::new(),
    };
    let fetcher = QueueFetcher::new([]);
    let caller = PhaseLimits {
        max_duration_ms: 1_999,
        ..PhaseLimits::BACKEND
    };

    let result = execution_failed(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
        job_radar_lib::PhaseExecutionFailure::InvalidCallerLimits,
    );

    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage, Default::default());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_caller_phase_limits"));
    assert!(fetcher.request_count() == 0);
}

#[tokio::test]
async fn accepted_discovery_has_one_complete_exact_nine_dimension_report() {
    let plan = plan();
    let fetcher = QueueFetcher::new([
        json!({ "jobs": [{ "id": "1", "title": "Engineer", "url": "https://example.test/1", "locations": [] }] }),
        json!({ "jobs": [] }),
    ]);

    let equality_limits = PhaseLimits {
        max_strategy_attempts: 1,
        max_requests: 2,
        max_produced_items: 1,
        max_pages: 2,
        ..PhaseLimits::BACKEND
    };
    let result = accepted_phase(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(equality_limits),
        )
        .await,
    );

    assert_eq!(result.payload.candidates.len(), 1);
    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.strategy_attempts, 1);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(report.usage.produced_items, 1);
    assert_eq!(report.usage.pages, 2);
    assert_eq!(report.usage.browser_actions, 0);
    assert_eq!(report.usage.fan_out, 0);
    assert!(report.usage.response_bytes > 0);
    assert_eq!(report.usage.browser_rendered_bytes, 0);
    assert!(report.usage.duration_ms <= PhaseLimits::BACKEND.max_duration_ms);
}

#[tokio::test]
async fn attempt_one_over_is_denied_before_second_strategy() {
    let mut plan = plan();
    let mut fallback = plan.discovery.strategies[0].clone();
    fallback.key = "second".to_string();
    plan.discovery.strategies.push(fallback);
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 503,
        final_url: "https://example.test/failure".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Failure(
            ProfileHttpFailureKind::BodyStream,
        )],
        content_length: None,
    }]);
    let caller = PhaseLimits {
        max_strategy_attempts: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            &fetcher,
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::StrategyAttempts);
    assert_eq!(report.usage.strategy_attempts, 1);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test]
async fn request_one_over_is_denied_before_second_page_and_hides_prefix_payload() {
    let plan = plan();
    let fetcher = QueueFetcher::new([json!({ "jobs": [
        { "id": "1", "title": "One", "url": "https://example.test/1", "locations": [] }
    ] })]);
    let caller = PhaseLimits {
        max_requests: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::Requests);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(report.usage.pages, 1);
    assert_eq!(report.usage.produced_items, 0);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test]
async fn exact_response_byte_allowance_followed_by_eof_succeeds() {
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let body = "{\"jobs\":[]}";
    let fetcher = QueueFetcher::new_raw([body.to_string()]);
    let caller = PhaseLimits {
        max_response_bytes: body.len() as u64,
        ..PhaseLimits::BACKEND
    };

    let result = accepted_phase(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.response_bytes, body.len() as u64);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test]
async fn response_byte_one_over_commits_only_the_admitted_prefix_and_hides_payload() {
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let fetcher = QueueFetcher::new_raw(["{\"jobs\":[]}".to_string()]);
    let caller = PhaseLimits {
        max_response_bytes: 10,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    assert_eq!(fetcher.request_count(), 1);
    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::ResponseBytes);
    assert_eq!(report.usage.response_bytes, 10);
}

#[tokio::test]
async fn response_byte_allowance_is_cumulative_across_pages() {
    let plan = plan();
    let first = json!({ "jobs": [
        { "id": "1", "title": "One", "url": "https://example.test/1", "locations": [] }
    ] })
    .to_string();
    let second = json!({ "jobs": [] }).to_string();
    let limit = first.len() as u64 + 5;
    let fetcher = QueueFetcher::new_raw([first, second]);
    let caller = PhaseLimits {
        max_response_bytes: limit,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    assert_eq!(fetcher.request_count(), 2);
    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::ResponseBytes);
    assert_eq!(report.usage.response_bytes, limit);
    assert_eq!(report.usage.requests, 2);
    assert_eq!(report.usage.pages, 2);
}

#[tokio::test]
async fn atomic_request_page_one_over_does_not_charge_the_fitting_request() {
    let plan = plan();
    let fetcher = QueueFetcher::new([json!({ "jobs": [
        { "id": "1", "title": "One", "url": "https://example.test/1", "locations": [] }
    ] })]);
    let caller = PhaseLimits {
        max_pages: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::Pages);
    assert_eq!(
        report.usage.requests, 1,
        "denied request+page debit is atomic"
    );
    assert_eq!(report.usage.pages, 1);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test]
async fn cancellation_after_request_debit_prevents_the_effect_and_keeps_the_charge() {
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let cancellation = CancelsOnCheck {
        checks: AtomicUsize::new(0),
        cancel_on: 5,
    };
    let fetcher = QueueFetcher::new([json!({ "jobs": [] })]);

    let result = cancelled(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::with_cancellation(&cancellation),
        )
        .await,
    );

    let report = result.complete_budget_report;
    assert_eq!(
        report.completion,
        PhaseCompletion::Cancelled {
            reason: PhaseCancellationReason::UserCancelled
        }
    );
    assert_eq!(report.usage.requests, 1);
    assert!(
        fetcher.request_count() == 0,
        "effect must not start after Cancellation becomes observable"
    );
}

#[tokio::test]
async fn paginated_produced_item_exact_equality_is_charged_after_acceptance() {
    let mut plan = plan();
    let Some(CompiledPagination::Page(pagination)) = &mut plan.discovery.strategies[0].pagination
    else {
        panic!("expected page pagination")
    };
    pagination.limits.max_items = Some(1);
    let fetcher = QueueFetcher::new([json!({ "jobs": [
        { "id": "1", "title": "One", "url": "https://example.test/1", "locations": [] }
    ] })]);
    let caller = PhaseLimits {
        max_produced_items: 1,
        ..PhaseLimits::BACKEND
    };

    let result = accepted_phase(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(result.report.usage.produced_items, 1);
    assert_eq!(result.report.completion, PhaseCompletion::Accepted);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test]
async fn paginated_produced_item_one_over_is_payload_free_after_acceptance_validation() {
    let mut plan = plan();
    let Some(CompiledPagination::Page(pagination)) = &mut plan.discovery.strategies[0].pagination
    else {
        panic!("expected page pagination")
    };
    pagination.limits.max_items = Some(2);
    let fetcher = QueueFetcher::new([json!({ "jobs": [
        { "id": "1", "title": "One", "url": "https://example.test/1", "locations": [] },
        { "id": "2", "title": "Two", "url": "https://example.test/2", "locations": [] }
    ] })]);
    let caller = PhaseLimits {
        max_produced_items: 1,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    assert_eq!(fetcher.request_count(), 1);
    let report = result.report;
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("expected budget exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::ProducedItems);
    assert_eq!(exhaustion.requested, 1);
    assert_eq!(exhaustion.remaining, 0);
    assert_eq!(report.usage.produced_items, 1);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

#[tokio::test]
async fn fan_out_one_over_charges_only_the_fitting_nonduplicate_prefix() {
    let plan = sitemap_fan_out_plan();
    let fetcher = QueueFetcher::new_raw([
        "<?xml version=\"1.0\"?><sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"><sitemap><loc>https://example.test/child-1.xml</loc></sitemap><sitemap><loc>https://example.test/child-2.xml</loc></sitemap></sitemapindex>".to_string(),
    ]);
    let caller = PhaseLimits {
        max_fan_out: 1,
        max_response_bytes: 67_108_864,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    let report = result.report;
    assert_exhaustion(&report.completion, AllowanceDimension::FanOut);
    assert_eq!(report.usage.fan_out, 1);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn exact_duration_boundary_may_complete_before_deadline_exhaustion() {
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs".to_string(),
        headers: Vec::new(),
        body: vec![
            ScriptedHttpBodyEvent::Gate("exact-boundary".to_string()),
            ScriptedHttpBodyEvent::Chunk(json!({ "jobs": [] }).to_string().into_bytes()),
        ],
        content_length: None,
    }]);
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let caller = PhaseLimits {
        max_duration_ms: 50,
        ..PhaseLimits::BACKEND
    };

    let release = async {
        while !fetcher.gate_is_waiting("exact-boundary") {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(fetcher.release_gate("exact-boundary"));
    };
    let execute = execute_discovery(
        &plan,
        empty_source_config(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable().with_limits(caller),
    );
    let (_, result) = tokio::join!(release, execute);
    let result = accepted_phase(result);

    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::Accepted);
    assert_eq!(report.usage.duration_ms, 50);
}

#[tokio::test(start_paused = true)]
async fn admitted_response_prefix_is_committed_after_deadline_stop_is_recorded() {
    let prefix = b"{\"jobs\":[";
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs".to_string(),
        headers: Vec::new(),
        body: vec![
            ScriptedHttpBodyEvent::Chunk(prefix.to_vec()),
            ScriptedHttpBodyEvent::Gate("deadline-prefix".to_string()),
        ],
        content_length: None,
    }]);
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let caller = PhaseLimits {
        max_duration_ms: 50,
        ..PhaseLimits::BACKEND
    };

    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            &fetcher,
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );

    assert_exhaustion(&result.report.completion, AllowanceDimension::Duration);
    assert_eq!(result.report.usage.response_bytes, prefix.len() as u64);
    assert_eq!(fetcher.request_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn observed_cancellation_wins_when_the_deadline_becomes_ready() {
    struct DeadlineCancellation(tokio::time::Instant);
    impl RuntimeCancellation for DeadlineCancellation {
        fn is_cancelled(&self) -> bool {
            tokio::time::Instant::now() >= self.0
        }
    }
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Gate(
            "deadline-cancellation".to_string(),
        )],
        content_length: None,
    }]);
    let mut plan = plan();
    plan.discovery.strategies[0].pagination = None;
    let duration_ms = 50;
    let cancellation = DeadlineCancellation(
        tokio::time::Instant::now() + std::time::Duration::from_millis(duration_ms),
    );
    let caller = PhaseLimits {
        max_duration_ms: duration_ms,
        ..PhaseLimits::BACKEND
    };

    let result = cancelled(
        execute_discovery(
            &plan,
            empty_source_config(),
            &fetcher,
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::with_cancellation(&cancellation).with_limits(caller),
        )
        .await,
    );

    let report = result.complete_budget_report;
    assert_eq!(
        report.completion,
        PhaseCompletion::Cancelled {
            reason: PhaseCancellationReason::UserCancelled
        }
    );
    assert_eq!(report.usage.requests, 1);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "phase_allowance_exhausted"));
}

#[tokio::test]
async fn invocation_deadline_stops_active_effect_and_reports_duration_exhaustion() {
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Gate("past-deadline".to_string())],
        content_length: None,
    }]);
    let plan = plan();
    let caller = PhaseLimits {
        max_duration_ms: 1,
        ..PhaseLimits::BACKEND
    };
    let result = budget_exhausted(
        execute_discovery(
            &plan,
            empty_source_config(),
            &fetcher,
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )
        .await,
    );
    let report = result.report;
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("expected budget exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Duration);
    assert_eq!(exhaustion.requested, 1);
    assert_eq!(exhaustion.remaining, 0);
    assert!(report.usage.duration_ms >= 1);
}

#[test]
fn caller_raise_is_execution_failed_before_work_with_zero_usage_report() {
    let plan = plan();
    let fetcher = QueueFetcher::new([]);
    let caller = PhaseLimits {
        max_browser_rendered_bytes: plan.discovery.limits.max_browser_rendered_bytes + 1,
        ..PhaseLimits::BACKEND
    };

    let result = execution_failed(
        tauri::async_runtime::block_on(execute_discovery(
            &plan,
            empty_source_config(),
            fetcher.client(),
            &UnavailableProfileBrowserClient,
            RuntimeExecutionContext::uncancellable().with_limits(caller),
        )),
        job_radar_lib::PhaseExecutionFailure::InvalidCallerLimits,
    );

    let report = result.report;
    assert_eq!(report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage.strategy_attempts, 0);
    assert_eq!(report.usage.requests, 0);
    assert_eq!(report.usage.produced_items, 0);
    assert_eq!(report.usage.duration_ms, 0);
    assert_eq!(report.usage.pages, 0);
    assert_eq!(report.usage.browser_actions, 0);
    assert_eq!(report.usage.fan_out, 0);
    assert_eq!(report.usage.response_bytes, 0);
    assert_eq!(report.usage.browser_rendered_bytes, 0);
    assert!(fetcher.request_count() == 0);
    assert_eq!(result.diagnostics[0].code, "invalid_caller_phase_limits");
}

fn assert_exhaustion(completion: &PhaseCompletion, dimension: AllowanceDimension) {
    let PhaseCompletion::BudgetExhausted { exhaustion } = completion else {
        panic!("expected BudgetExhausted, got {completion:?}")
    };
    assert_eq!(exhaustion.dimension, dimension);
    assert_eq!(exhaustion.requested, 1);
    assert_eq!(exhaustion.remaining, 0);
}

fn sitemap_fan_out_plan() -> job_radar_lib::SourceExecutionPlan {
    let mut profile_value = read_json("resources/profiles/successfactors.json");
    profile_value["accessPaths"][0]["discovery"]["strategies"][0]["pagination"]
        ["childSitemapSelector"] = json!({ "type": "sitemap_urls" });
    profile_value["accessPaths"][0]["discovery"]["strategies"][0]["pagination"]["limits"] =
        json!({ "maxRequests": 3, "maxItems": 200, "maxDepth": 1 });
    let profile: SourceProfileDocument = serde_json::from_value(profile_value).unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "fanout_source",
        "name": "Fanout source",
        "status": "active",
        "sourceConfig": {
            "baseUrl": "https://example.test",
            "sitemapUrl": "https://example.test/sitemap.xml"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "successfactors",
            "pathKey": "rmk_sitemap_html"
        }
    }))
    .unwrap();
    compile_plan(source, profile)
}

fn plan() -> job_radar_lib::SourceExecutionPlan {
    let profile: SourceProfileDocument = serde_json::from_value(read_json(
        "tests/fixtures/source-profile-dsl/valid/simple-source-profile.json",
    ))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(read_json(
        "tests/fixtures/source-profile-dsl/valid/source-selecting-access-path.json",
    ))
    .unwrap();
    compile_plan(source, profile)
}

fn compile_plan(
    source: SourceDocument,
    profile: SourceProfileDocument,
) -> job_radar_lib::SourceExecutionPlan {
    let registry = SourceProfileRegistrySnapshot {
        profiles: vec![RegistrySourceProfile {
            origin: "test".into(),
            path: String::new(),
            document: profile,
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };
    match job_radar_lib::compile_source(&source, &registry) {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } if diagnostics.is_empty() => source.execution_plan,
        other => panic!("fixture must compile: {other:?}"),
    }
}

fn read_json(relative: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}
