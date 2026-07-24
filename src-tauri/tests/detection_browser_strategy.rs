use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use job_radar_lib::{
    compile_detection_plan, detection_descriptor_for_authored_kind, execute_detection_operation,
    AllowanceDimension, BrowserAcquisitionFailure, BrowserAcquisitionFailureKind,
    BrowserAcquisitionRequestSnapshot, BrowserLifecycleEvent, DetectionProfileCompletion,
    DetectionProfileExecutionFailureKind, DetectionProfileRejectionKind, DetectionRunStatus,
    DetectionStrategy, ExecutionPlanBrowserInteraction, PhaseBrowser, PhaseCompletion,
    RuntimeCancellation, ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization, ScriptedProfileHttpClient,
    SourceProfileDocument, DETECTION_BROWSER_DESCRIPTOR,
};
use serde_json::{json, Value};

#[derive(Default)]
struct Cancellation(AtomicBool);
impl Cancellation {
    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl RuntimeCancellation for Cancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[test]
fn d03_descriptor_ties_authored_and_compiled_browser_shape() {
    assert_eq!(DETECTION_BROWSER_DESCRIPTOR.owner, "D03");
    assert!(DETECTION_BROWSER_DESCRIPTOR
        .canonical_file
        .ends_with("source_profile/detection/strategy.rs"));
    assert_eq!(
        DETECTION_BROWSER_DESCRIPTOR
            .options
            .iter()
            .map(|option| {
                (
                    option.key,
                    option.required,
                    option.minimum,
                    option.maximum,
                    option.compiled_identity,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "key",
                true,
                None,
                None,
                "CompiledDetectionStrategy::Browser.key"
            ),
            ("fetch", true, None, None, "ExecutionPlanFetch::Browser"),
            (
                "contains",
                false,
                None,
                None,
                "CompiledDetectionStrategy::Browser.contains"
            ),
            (
                "regex",
                false,
                None,
                None,
                "CompiledDetectionStrategy::Browser.acceptance_regex"
            ),
            (
                "captures",
                false,
                None,
                None,
                "CompiledDetectionStrategy::Browser.captures"
            ),
            (
                "evidence",
                false,
                None,
                None,
                "CompiledDetectionStrategy::Browser.evidence"
            ),
        ]
    );

    let authored_value = json!({
        "type": "browser", "key": "browser",
        "fetch": {
            "mode": "browser", "url": "https://example.test", "timeoutMs": 2000,
            "waits": [{ "type": "network_idle", "timeoutMs": 1 }],
            "interactions": [{ "type": "click_if_visible", "selector": ".more", "maxCount": 1 }]
        },
        "contains": "known", "regex": "(?<tenant>known)",
        "captures": ["tenant"], "evidence": "known"
    });
    let authored: DetectionStrategy = serde_json::from_value(authored_value).unwrap();
    assert_eq!(
        detection_descriptor_for_authored_kind(authored.kind()),
        &DETECTION_BROWSER_DESCRIPTOR
    );
    let serialized = serde_json::to_value(&authored).unwrap();
    let structural_keys = serialized
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| key.as_str() != "type")
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let descriptor_keys = DETECTION_BROWSER_DESCRIPTOR
        .options
        .iter()
        .map(|option| option.key)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(structural_keys, descriptor_keys);
    let plan = compile_detection_plan(&profile(
        "descriptor",
        vec![json!({
            "type": "browser", "key": "browser",
            "fetch": { "mode": "browser", "url": "https://example.test", "timeoutMs": 2000 },
            "contains": "known"
        })],
    ))
    .unwrap();
    assert_eq!(
        plan.strategy_descriptors().copied().collect::<Vec<_>>(),
        vec![
            job_radar_lib::DETECTION_URL_DESCRIPTOR,
            DETECTION_BROWSER_DESCRIPTOR
        ]
    );
}

fn profile(key: &str, browser_strategies: Vec<Value>) -> SourceProfileDocument {
    let mut value: Value =
        serde_json::from_str(include_str!("../resources/profiles/greenhouse.json")).unwrap();
    value["key"] = json!(key);
    value["name"] = json!(format!("Fixture {key}"));
    value["sourceConfigSchema"] = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["startUrl"],
        "properties": { "startUrl": { "type": "string", "format": "uri" } }
    });
    value["accessPaths"].as_array_mut().unwrap().truncate(1);
    value["accessPaths"][0]["key"] = json!("browser");
    value["accessPaths"][0]["name"] = json!("Browser");
    let mut strategies = vec![json!({
        "type": "url",
        "key": "url",
        "input": { "type": "absolute_url" }
    })];
    strategies.extend(browser_strategies);
    value["detection"] = json!({
        "recommendedAccessPathKey": "browser",
        "policy": { "type": "all_required" },
        "strategies": strategies
    });
    serde_json::from_value(value).unwrap()
}

fn browser_strategy(key: &str, contains: &str) -> Value {
    json!({
        "type": "browser",
        "key": key,
        "fetch": {
            "mode": "browser",
            "url": "{{inputUrl}}",
            "timeoutMs": 20000
        },
        "contains": contains,
        "regex": "tenant=(?<tenant>[a-z]+)",
        "captures": ["tenant"],
        "evidence": "rendered browser evidence"
    })
}

#[test]
fn direct_serde_rejects_invalid_browser_detection_shapes() {
    let valid = profile("serde", vec![browser_strategy("render", "known")]);

    let mut wrong_mode = serde_json::to_value(&valid).unwrap();
    wrong_mode["detection"]["strategies"][1]["fetch"] = json!({
        "mode": "http", "url": "https://example.test", "timeoutMs": 1000
    });
    assert!(serde_json::from_value::<SourceProfileDocument>(wrong_mode).is_err());

    let mut no_acceptance = serde_json::to_value(valid).unwrap();
    no_acceptance["detection"]["strategies"][1]
        .as_object_mut()
        .unwrap()
        .retain(|key, _| key != "contains" && key != "regex" && key != "captures");
    assert!(serde_json::from_value::<SourceProfileDocument>(no_acceptance).is_err());
}

fn request(remaining: u64) -> BrowserAcquisitionRequestSnapshot {
    BrowserAcquisitionRequestSnapshot {
        target: "https://example.test/".into(),
        timeout_ms: 20_000,
        waits: vec![],
        interactions: vec![],
        browser_rendered_bytes_remaining: remaining,
    }
}

fn successful_expectation(body: String, remaining: u64) -> ScriptedBrowserAcquisitionExpectation {
    ScriptedBrowserAcquisitionExpectation {
        request: request(remaining),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content(body),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }
}

#[test]
fn compiler_accepts_only_bounded_browser_detection_and_compiles_dependencies_before_io() {
    let exact = profile("exact", vec![browser_strategy("render", "known")]);
    let plan = compile_detection_plan(&exact).unwrap();
    assert_eq!(
        plan.strategy_keys().collect::<Vec<_>>(),
        vec!["url", "render"]
    );

    let mut minimum_timeout = browser_strategy("minimum", "known");
    minimum_timeout["fetch"]["timeoutMs"] = json!(1);
    compile_detection_plan(&profile("minimum_timeout", vec![minimum_timeout]))
        .expect("positive Browser timeout below teardown reserve remains valid");

    for (field, value, code) in [(
        "timeoutMs",
        json!(20_001),
        "invalid_detection_browser_timeout",
    )] {
        let mut strategy = browser_strategy("render", "known");
        strategy["fetch"][field] = value;
        assert_eq!(
            compile_detection_plan(&profile("bounded", vec![strategy])).unwrap_err()[0].code,
            code
        );
    }

    let mut too_many_actions = browser_strategy("render", "known");
    too_many_actions["fetch"]["interactions"] = json!([{
        "type": "click_until_gone", "selector": ".more", "maxCount": 6
    }]);
    assert_eq!(
        compile_detection_plan(&profile("actions", vec![too_many_actions])).unwrap_err()[0].code,
        "invalid_detection_browser_action_count"
    );

    let mut long_wait = browser_strategy("render", "known");
    long_wait["fetch"]["waits"] = json!([{
        "type": "network_idle", "timeoutMs": 5001
    }]);
    assert_eq!(
        compile_detection_plan(&profile("wait", vec![long_wait])).unwrap_err()[0].code,
        "invalid_detection_browser_wait_timeout"
    );

    let mut long_wait_after = browser_strategy("render", "known");
    long_wait_after["fetch"]["interactions"] = json!([{
        "type": "click_if_visible", "selector": ".more", "maxCount": 1, "waitAfterMs": 5001
    }]);
    assert_eq!(
        compile_detection_plan(&profile("wait_after", vec![long_wait_after])).unwrap_err()[0].code,
        "invalid_detection_browser_wait_after"
    );

    let mut unknown_dependency = browser_strategy("render", "known");
    unknown_dependency["fetch"]["url"] = json!("https://example.test/{{capture:missing}}");
    assert_eq!(
        compile_detection_plan(&profile("dependency", vec![unknown_dependency])).unwrap_err()[0]
            .code,
        "compiled_execution_plan_invariant_violation"
    );
}

#[tokio::test]
async fn browser_acceptance_emits_native_capture_evidence_and_http_independent_report() {
    let plan = compile_detection_plan(&profile(
        "native",
        vec![browser_strategy("render", "known")],
    ))
    .unwrap();
    let browser = ScriptedBrowserAcquisition::new([successful_expectation(
        "known tenant=acme".into(),
        2_097_152,
    )]);
    let result = execute_detection_operation(
        "https://example.test",
        &[plan],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;

    assert_eq!(result.run_result.status, DetectionRunStatus::Matched);
    let proposal = &result.run_result.proposals[0];
    assert_eq!(
        proposal.captures.get("tenant").map(String::as_str),
        Some("acme")
    );
    assert_eq!(
        proposal.provenance.captures["tenant"][0].strategy_key(),
        "render"
    );
    assert!(proposal
        .evidence
        .iter()
        .any(|evidence| evidence.message == "rendered browser evidence"));
    assert_eq!(result.report.usage.response_bytes, 0);
    assert_eq!(
        result.report.usage.browser_rendered_bytes,
        "known tenant=acme".len() as u64
    );
    assert_eq!(result.report.usage.requests, 1);
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert!(browser.expectations_satisfied());
}

#[tokio::test]
async fn http_and_browser_use_independent_shared_report_dimensions() {
    let mut mixed = browser_strategy("render", "known");
    mixed["fetch"]["url"] = json!("https://example.test/render");
    let mut document = profile("mixed", vec![mixed]);
    document
        .detection
        .as_mut()
        .unwrap()
        .strategies
        .as_mut()
        .unwrap()
        .insert(
            1,
            serde_json::from_value(json!({
                "type": "http",
                "key": "probe",
                "fetch": { "mode": "http", "url": "https://example.test/probe", "timeoutMs": 1000 },
                "contains": "http"
            }))
            .unwrap(),
        );
    let browser = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            target: "https://example.test/render".into(),
            ..request(2_097_152)
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let http = ScriptedProfileHttpClient::new([job_radar_lib::ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/probe".into(),
        headers: vec![],
        body: vec![job_radar_lib::ScriptedHttpBodyEvent::Chunk(
            b"http".to_vec(),
        )],
        content_length: Some(4),
    }]);
    let result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&document).unwrap()],
        &http,
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert_eq!(result.report.usage.response_bytes, 4);
    assert_eq!(result.report.usage.browser_rendered_bytes, 17);
    assert_eq!(
        result.report.usage.requests, 1,
        "HTTP does not debit Browser navigations"
    );
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
}

#[tokio::test(start_paused = true)]
async fn http_time_does_not_consume_browser_duration() {
    let mut document = profile("duration", vec![browser_strategy("render", "known")]);
    document
        .detection
        .as_mut()
        .unwrap()
        .strategies
        .as_mut()
        .unwrap()
        .insert(
            1,
            serde_json::from_value(json!({
                "type": "http",
                "key": "probe",
                "fetch": { "mode": "http", "url": "https://example.test/probe", "timeoutMs": 1000 },
                "contains": "http"
            }))
            .unwrap(),
        );
    let http = Arc::new(ScriptedProfileHttpClient::new([
        job_radar_lib::ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/probe".into(),
            headers: vec![],
            body: vec![
                job_radar_lib::ScriptedHttpBodyEvent::Gate("http".into()),
                job_radar_lib::ScriptedHttpBodyEvent::Chunk(b"http".to_vec()),
            ],
            content_length: None,
        },
    ]));
    let browser = Arc::new(ScriptedBrowserAcquisition::new([
        ScriptedBrowserAcquisitionExpectation {
            request: request(2_097_152),
            events: vec![
                ScriptedBrowserAcquisitionEvent::Gate("browser".into()),
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        },
    ]));
    let plan = compile_detection_plan(&document).unwrap();
    let task_http = Arc::clone(&http);
    let task_browser = Arc::clone(&browser);
    let task = tokio::spawn(async move {
        execute_detection_operation(
            "https://example.test",
            &[plan],
            task_http.as_ref(),
            PhaseBrowser::Browser(task_browser.as_ref()),
            &Cancellation::default(),
        )
        .await
    });
    while !http.gate_is_waiting("http") {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(std::time::Duration::from_secs(70)).await;
    assert!(http.release_gate("http"));
    while !browser.gate_is_waiting("browser") {
        tokio::task::yield_now().await;
    }
    assert!(browser.release_gate("browser"));
    let result = task.await.unwrap();
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert_eq!(result.report.usage.duration_ms, 0);
}

#[tokio::test]
async fn profile_action_scope_accepts_exact_ten_executed_clicks() {
    let mut first = browser_strategy("first", "known");
    let mut second = browser_strategy("second", "known");
    for strategy in [&mut first, &mut second] {
        strategy["fetch"]["interactions"] = json!([{
            "type": "click_until_gone", "selector": ".more", "maxCount": 5
        }]);
    }
    let interaction = ExecutionPlanBrowserInteraction::ClickUntilGone {
        selector: ".more".into(),
        max_count: 5,
        wait_after_ms: None,
    };
    let expectation = || ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            interactions: vec![interaction.clone()],
            ..request(2_097_152)
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 5,
            },
            ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    };
    let browser = ScriptedBrowserAcquisition::new([expectation(), expectation()]);
    let result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile("actions_exact", vec![first, second])).unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert_eq!(result.report.usage.browser_actions, 10);
    assert_eq!(
        result.run_result.proposals[0].provenance.captures["tenant"].len(),
        2,
        "equal native captures retain both ordered origins",
    );
    assert!(browser.expectations_satisfied());
}

#[tokio::test]
async fn conflicting_browser_capture_is_a_native_reconciliation_failure() {
    let browser = ScriptedBrowserAcquisition::new([
        successful_expectation("known tenant=acme".into(), 2_097_152),
        successful_expectation("known tenant=other".into(), 2_097_152),
    ]);
    let result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "conflict",
            vec![
                browser_strategy("first", "known"),
                browser_strategy("second", "known"),
            ],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert!(result.run_result.proposals.is_empty());
    assert!(matches!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::ExecutionFailed {
            kind: DetectionProfileExecutionFailureKind::Reconciliation,
            ..
        }
    ));
    assert!(result.profile_outcomes[0]
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "detection_contribution_conflict"));
}

#[tokio::test]
async fn operation_action_scope_accepts_exact_32_and_denies_33rd_before_click() {
    let mut plans = Vec::new();
    let mut exact_expectations = Vec::new();
    for index in 0..7 {
        let mut strategy = browser_strategy("render", "known");
        strategy["fetch"]["interactions"] = json!([{
            "type": "click_until_gone", "selector": ".more", "maxCount": 5
        }]);
        plans.push(
            compile_detection_plan(&profile(&format!("action_{index}"), vec![strategy])).unwrap(),
        );
        let attempted_clicks = if index == 6 { 2 } else { 5 };
        exact_expectations.push(ScriptedBrowserAcquisitionExpectation {
            request: BrowserAcquisitionRequestSnapshot {
                interactions: vec![ExecutionPlanBrowserInteraction::ClickUntilGone {
                    selector: ".more".into(),
                    max_count: 5,
                    wait_after_ms: None,
                }],
                ..request(2_097_152)
            },
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Interaction {
                    interaction_index: 0,
                    attempted_clicks,
                },
                ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        });
    }
    let exact_browser = ScriptedBrowserAcquisition::new(exact_expectations.clone());
    let exact = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&exact_browser),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(exact.report.completion, PhaseCompletion::Accepted));
    assert_eq!(exact.report.usage.browser_actions, 32);

    let mut over_expectations = exact_expectations;
    let last = over_expectations.last_mut().unwrap();
    last.events = vec![
        ScriptedBrowserAcquisitionEvent::Navigate,
        ScriptedBrowserAcquisitionEvent::Interaction {
            interaction_index: 0,
            attempted_clicks: 3,
        },
        ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
    ];
    let over_browser = ScriptedBrowserAcquisition::new(over_expectations);
    let over = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&over_browser),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = over.report.completion else {
        panic!("expected action exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::BrowserActions);
    assert_eq!(over.report.usage.browser_actions, 32);
    assert_eq!(
        over_browser
            .lifecycle()
            .iter()
            .filter(|event| matches!(event, BrowserLifecycleEvent::InteractionAttempt { .. }))
            .count(),
        32
    );
}

#[tokio::test]
async fn first_browser_rejection_stops_all_required_without_accepted_prefix() {
    let plan = compile_detection_plan(&profile(
        "reject",
        vec![
            browser_strategy("first", "required"),
            browser_strategy("later", "later"),
        ],
    ))
    .unwrap();
    let browser =
        ScriptedBrowserAcquisition::new([successful_expectation("tenant=acme".into(), 2_097_152)]);
    let result = execute_detection_operation(
        "https://example.test",
        &[plan],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;

    assert_eq!(browser.requests().len(), 1);
    assert!(result.run_result.proposals.is_empty());
    assert!(matches!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::Rejected {
            kind: DetectionProfileRejectionKind::Contains,
            ..
        }
    ));
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

#[tokio::test]
async fn logical_wait_scopes_accept_32_and_deny_a_strategy_fifth_wait() {
    let waits = json!([
        { "type": "network_idle", "timeoutMs": 1 },
        { "type": "network_idle", "timeoutMs": 1 },
        { "type": "network_idle", "timeoutMs": 1 },
        { "type": "network_idle", "timeoutMs": 1 }
    ]);
    let compiled_waits = (0..4)
        .map(|_| job_radar_lib::ExecutionPlanBrowserWait::NetworkIdle { timeout_ms: 1 })
        .collect::<Vec<_>>();
    let mut plans = Vec::new();
    let mut expectations = Vec::new();
    for index in 0..8 {
        let mut strategy = browser_strategy("render", "known");
        strategy["fetch"]["waits"] = waits.clone();
        plans.push(
            compile_detection_plan(&profile(&format!("wait_{index}"), vec![strategy])).unwrap(),
        );
        expectations.push(ScriptedBrowserAcquisitionExpectation {
            request: BrowserAcquisitionRequestSnapshot {
                waits: compiled_waits.clone(),
                ..request(2_097_152)
            },
            events: vec![
                ScriptedBrowserAcquisitionEvent::Navigate,
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 1 },
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 2 },
                ScriptedBrowserAcquisitionEvent::Wait { wait_index: 3 },
                ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
            ],
            finalization: ScriptedBrowserFinalization::default(),
        });
    }
    let browser = ScriptedBrowserAcquisition::new(expectations);
    let exact = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(exact.report.completion, PhaseCompletion::Accepted));
    assert_eq!(exact.report.usage.requests, 8);

    let mut strategy = browser_strategy("render", "known");
    strategy["fetch"]["interactions"] = json!([{
        "type": "click_until_gone", "selector": ".more", "maxCount": 5, "waitAfterMs": 1
    }]);
    let over_browser = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            interactions: vec![ExecutionPlanBrowserInteraction::ClickUntilGone {
                selector: ".more".into(),
                max_count: 5,
                wait_after_ms: Some(1),
            }],
            ..request(2_097_152)
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 5,
            },
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let over = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile("wait_over", vec![strategy])).unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&over_browser),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = over.report.completion else {
        panic!("expected logical wait exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::LogicalWaits);
    assert_eq!(over.report.usage.browser_actions, 5);
    assert_eq!(
        over_browser
            .lifecycle()
            .iter()
            .filter(|event| matches!(event, BrowserLifecycleEvent::WaitAfter { .. }))
            .count(),
        4
    );
}

#[tokio::test]
async fn operation_navigation_scope_accepts_eight_and_denies_ninth_before_navigation() {
    let mut plans = Vec::new();
    for index in 0..5 {
        plans.push(
            compile_detection_plan(&profile(
                &format!("profile_{index}"),
                vec![
                    browser_strategy("first", "known"),
                    browser_strategy("second", "known"),
                ],
            ))
            .unwrap(),
        );
    }
    let mut expectations = (0..8)
        .map(|_| successful_expectation("known tenant=acme".into(), 2_097_152))
        .collect::<Vec<_>>();
    expectations.push(successful_expectation(
        "known tenant=acme".into(),
        2_097_152,
    ));
    let browser = ScriptedBrowserAcquisition::new(expectations);
    let result = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = &result.report.completion else {
        panic!("expected operation navigation exhaustion")
    };
    assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
    assert_eq!(result.report.usage.requests, 8);
    assert_eq!(browser.requests().len(), 9);
    assert_eq!(
        browser
            .lifecycle()
            .iter()
            .filter(|event| matches!(event, BrowserLifecycleEvent::Navigation))
            .count(),
        8
    );
    assert!(result.run_result.proposals.is_empty());
}

#[tokio::test]
async fn rendered_bytes_reach_exact_profile_and_operation_cumulative_boundaries() {
    let body = format!("{}tenant=acme", "x".repeat(2_097_152 - 11));
    let mut plans = Vec::new();
    let mut expectations = Vec::new();
    for index in 0..4 {
        plans.push(
            compile_detection_plan(&profile(
                &format!("bytes_{index}"),
                vec![
                    browser_strategy("first", "tenant=acme"),
                    browser_strategy("second", "tenant=acme"),
                ],
            ))
            .unwrap(),
        );
        expectations.push(successful_expectation(body.clone(), 2_097_152));
        expectations.push(successful_expectation(body.clone(), 2_097_152));
    }
    let browser = ScriptedBrowserAcquisition::new(expectations);
    let result = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert_eq!(result.report.usage.browser_rendered_bytes, 16_777_216);
    assert_eq!(result.report.usage.requests, 8);
    assert!(browser.expectations_satisfied());
}

#[tokio::test]
async fn cumulative_profile_and_operation_byte_excess_consumes_all_remaining_scopes() {
    let exact = format!("{}tenant=acme", "x".repeat(2_097_152 - 11));
    let over = "x".repeat(2_097_153);

    let profile_browser = ScriptedBrowserAcquisition::new([
        successful_expectation(exact.clone(), 2_097_152),
        successful_expectation(over.clone(), 2_097_152),
    ]);
    let profile_result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "profile_over",
            vec![
                browser_strategy("first", "tenant=acme"),
                browser_strategy("second", "tenant=acme"),
            ],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&profile_browser),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = &profile_result.report.completion else {
        panic!("expected cumulative profile byte exhaustion")
    };
    assert_eq!(
        exhaustion.dimension,
        AllowanceDimension::BrowserRenderedBytes
    );
    assert_eq!(
        profile_result.report.usage.browser_rendered_bytes,
        16_777_216
    );
    assert!(profile_result.run_result.proposals.is_empty());

    let mut plans = Vec::new();
    let mut expectations = Vec::new();
    for index in 0..4 {
        plans.push(
            compile_detection_plan(&profile(
                &format!("operation_over_{index}"),
                vec![
                    browser_strategy("first", "tenant=acme"),
                    browser_strategy("second", "tenant=acme"),
                ],
            ))
            .unwrap(),
        );
        expectations.push(successful_expectation(exact.clone(), 2_097_152));
        expectations.push(successful_expectation(
            if index == 3 {
                over.clone()
            } else {
                exact.clone()
            },
            2_097_152,
        ));
    }
    let operation_browser = ScriptedBrowserAcquisition::new(expectations);
    let operation_result = execute_detection_operation(
        "https://example.test",
        &plans,
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&operation_browser),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = &operation_result.report.completion
    else {
        panic!("expected cumulative operation byte exhaustion")
    };
    assert_eq!(
        exhaustion.dimension,
        AllowanceDimension::BrowserRenderedBytes
    );
    assert_eq!(
        operation_result.report.usage.browser_rendered_bytes,
        16_777_216
    );
    assert_eq!(operation_result.report.usage.requests, 8);
    assert!(operation_result.run_result.proposals.is_empty());
}

#[tokio::test]
async fn rendered_byte_exact_boundary_succeeds_and_one_over_consumes_every_scope() {
    let exact_body = format!("{}tenant=acme", "x".repeat(2_097_152 - 11));
    assert_eq!(exact_body.len(), 2_097_152);
    let exact = ScriptedBrowserAcquisition::new([successful_expectation(exact_body, 2_097_152)]);
    let exact_result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "exact_bytes",
            vec![browser_strategy("render", "tenant=acme")],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&exact),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(
        exact_result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert_eq!(exact_result.report.usage.browser_rendered_bytes, 2_097_152);

    let over =
        ScriptedBrowserAcquisition::new([successful_expectation("x".repeat(2_097_153), 2_097_152)]);
    let over_result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "over_bytes",
            vec![browser_strategy("render", "tenant=acme")],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&over),
        &Cancellation::default(),
    )
    .await;
    let PhaseCompletion::BudgetExhausted { exhaustion } = &over_result.report.completion else {
        panic!("expected Browser byte exhaustion")
    };
    assert_eq!(
        exhaustion.dimension,
        AllowanceDimension::BrowserRenderedBytes
    );
    assert_eq!(over_result.report.usage.browser_rendered_bytes, 16_777_216);
    assert!(over_result.run_result.proposals.is_empty());
    assert!(over_result.attempts.is_empty());
}

#[tokio::test]
async fn absent_optional_click_charges_neither_action_nor_wait() {
    let mut strategy = browser_strategy("render", "known");
    strategy["fetch"]["interactions"] = json!([{
        "type": "click_if_visible",
        "selector": ".optional",
        "maxCount": 5,
        "waitAfterMs": 1000
    }]);
    let interaction = ExecutionPlanBrowserInteraction::ClickIfVisible {
        selector: ".optional".into(),
        max_count: 5,
        wait_after_ms: Some(1000),
    };
    let browser = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            interactions: vec![interaction],
            ..request(2_097_152)
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 0,
            },
            ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile("optional", vec![strategy])).unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(
        result.report.completion,
        PhaseCompletion::Accepted
    ));
    assert_eq!(result.report.usage.browser_actions, 0);
    assert!(!browser
        .lifecycle()
        .iter()
        .any(|event| matches!(event, BrowserLifecycleEvent::WaitAfter { .. })));
}

#[tokio::test]
async fn cancellation_waits_for_cleanup_and_cleanup_failure_is_typed() {
    let plan = compile_detection_plan(&profile(
        "cancel",
        vec![browser_strategy("render", "known")],
    ))
    .unwrap();
    let cancellation = Arc::new(Cancellation::default());
    let browser = Arc::new(ScriptedBrowserAcquisition::new([
        ScriptedBrowserAcquisitionExpectation {
            request: request(2_097_152),
            events: vec![ScriptedBrowserAcquisitionEvent::Gate("content".into())],
            finalization: ScriptedBrowserFinalization::default(),
        },
    ]));
    let task_browser = Arc::clone(&browser);
    let task_cancellation = Arc::clone(&cancellation);
    let task = tokio::spawn(async move {
        execute_detection_operation(
            "https://example.test",
            &[plan],
            &ScriptedProfileHttpClient::new([]),
            PhaseBrowser::Browser(task_browser.as_ref()),
            task_cancellation.as_ref(),
        )
        .await
    });
    while !browser.gate_is_waiting("content") {
        tokio::task::yield_now().await;
    }
    cancellation.cancel();
    let cancelled = task.await.unwrap();
    assert!(matches!(
        cancelled.report.completion,
        PhaseCompletion::Cancelled { .. }
    ));
    assert_eq!(
        browser.lifecycle().last(),
        Some(&BrowserLifecycleEvent::SessionFinalized)
    );
    assert!(cancelled.run_result.proposals.is_empty());

    let infrastructure = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: request(2_097_152),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content("known tenant=acme".into()),
        ],
        finalization: ScriptedBrowserFinalization::InfrastructureFailure {
            message: "cleanup failed".into(),
        },
    }]);
    let failed = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "infrastructure",
            vec![browser_strategy("render", "known")],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&infrastructure),
        &Cancellation::default(),
    )
    .await;
    assert!(matches!(
        failed.profile_outcomes[0].completion,
        DetectionProfileCompletion::ExecutionFailed {
            kind: DetectionProfileExecutionFailureKind::BrowserInfrastructure,
            ..
        }
    ));
    assert!(matches!(
        failed.report.completion,
        PhaseCompletion::ExecutionFailed
    ));
    assert!(failed.run_result.proposals.is_empty());
}

#[tokio::test]
async fn typed_stage_failure_stops_later_browser_work() {
    let browser = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: request(2_097_152),
        events: vec![ScriptedBrowserAcquisitionEvent::Failure(
            BrowserAcquisitionFailure::new(
                BrowserAcquisitionFailureKind::Navigation,
                "navigation failed",
            ),
        )],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let result = execute_detection_operation(
        "https://example.test",
        &[compile_detection_plan(&profile(
            "stage",
            vec![
                browser_strategy("first", "known"),
                browser_strategy("later", "known"),
            ],
        ))
        .unwrap()],
        &ScriptedProfileHttpClient::new([]),
        PhaseBrowser::Browser(&browser),
        &Cancellation::default(),
    )
    .await;
    assert_eq!(browser.requests().len(), 1);
    assert!(matches!(
        result.profile_outcomes[0].completion,
        DetectionProfileCompletion::ExecutionFailed {
            kind: DetectionProfileExecutionFailureKind::BrowserAcquisition(_),
            ..
        }
    ));
}
