use std::sync::atomic::{AtomicBool, Ordering};

use job_radar_lib::{
    BrowserAcquisition, BrowserAcquisitionFailure, BrowserAcquisitionFailureKind,
    BrowserAcquisitionRequestSnapshot, BrowserAcquisitionTerminal, BrowserInfrastructureFailure,
    BrowserLifecycleEvent, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
    PhaseCompletion, PhaseLimits, RuntimeCancellation, ScriptedBrowserAcquisition,
    ScriptedBrowserAcquisitionEvent, ScriptedBrowserAcquisitionExpectation,
    ScriptedBrowserFinalization,
    __TestBrowserAcquisitionInvocation as BrowserAcquisitionTestInvocation,
};

fn snapshot(target: &str, remaining: u64) -> BrowserAcquisitionRequestSnapshot {
    snapshot_with_timeout(target, 120_000, remaining)
}

fn snapshot_with_timeout(
    target: &str,
    timeout_ms: u64,
    remaining: u64,
) -> BrowserAcquisitionRequestSnapshot {
    BrowserAcquisitionRequestSnapshot {
        target: target.to_string(),
        timeout_ms,
        waits: Vec::new(),
        interactions: Vec::new(),
        browser_rendered_bytes_remaining: remaining,
    }
}

fn expectation(
    target: &str,
    remaining: u64,
    content: &str,
) -> ScriptedBrowserAcquisitionExpectation {
    ScriptedBrowserAcquisitionExpectation {
        request: snapshot(target, remaining),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content(content.to_string()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }
}

#[tokio::test]
async fn exact_limit_content_is_exposed_only_after_distinct_browser_byte_admission() {
    let limits = PhaseLimits {
        max_browser_rendered_bytes: 3,
        ..PhaseLimits::BACKEND
    };
    let invocation = BrowserAcquisitionTestInvocation::new(limits, true, None);
    let adapter = ScriptedBrowserAcquisition::new([expectation("https://example.test", 3, "abc")]);

    let content = adapter
        .acquire(invocation.request("https://example.test", Vec::new(), Vec::new()))
        .await
        .expect("exact-limit rendered content is admitted");

    assert_eq!(content.as_str(), "abc");
    let report = invocation.report(PhaseCompletion::Accepted);
    assert_eq!(report.usage.browser_rendered_bytes, 3);
    assert_eq!(report.usage.response_bytes, 0);
    assert_eq!(
        serde_json::to_value(&report).unwrap()["usage"]["browserRenderedBytes"],
        3
    );
    assert_eq!(
        adapter.lifecycle(),
        vec![
            BrowserLifecycleEvent::Reserved,
            BrowserLifecycleEvent::Navigation,
            BrowserLifecycleEvent::ContentRead,
            BrowserLifecycleEvent::PrimarySealed,
            BrowserLifecycleEvent::GracefulClose,
            BrowserLifecycleEvent::HandlerCompleted,
            BrowserLifecycleEvent::ActiveSessionReleased,
            BrowserLifecycleEvent::SessionFinalized,
        ]
    );
}

#[tokio::test]
async fn later_cumulative_excess_consumes_only_remaining_capacity_and_exposes_no_body() {
    let limits = PhaseLimits {
        max_browser_rendered_bytes: 3,
        ..PhaseLimits::BACKEND
    };
    let invocation = BrowserAcquisitionTestInvocation::new(limits, true, None);
    let adapter = ScriptedBrowserAcquisition::new([
        expectation("https://example.test/one", 3, "ab"),
        expectation("https://example.test/two", 1, "cd"),
    ]);

    let first = adapter
        .acquire(invocation.request("https://example.test/one", Vec::new(), Vec::new()))
        .await
        .expect("first rendered body fits");
    assert_eq!(first.as_str(), "ab");

    let second = adapter
        .acquire(invocation.request("https://example.test/two", Vec::new(), Vec::new()))
        .await;
    assert_eq!(second, Err(BrowserAcquisitionTerminal::AllowanceStopped));

    let report = invocation.report(PhaseCompletion::Accepted);
    assert_eq!(report.usage.browser_rendered_bytes, 3);
    assert_eq!(report.usage.response_bytes, 0);
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("Browser byte excess must be the shared allowance terminal")
    };
    assert_eq!(
        serde_json::to_value(exhaustion).unwrap(),
        serde_json::json!({
            "dimension": "browser_rendered_bytes",
            "requested": 1,
            "remaining": 0,
            "limitSources": ["compiled"]
        })
    );
}

#[tokio::test]
async fn every_ordinary_stage_failure_is_typed_charged_and_finalized_before_return() {
    let failures = [
        BrowserAcquisitionFailureKind::RuntimeLaunch,
        BrowserAcquisitionFailureKind::Navigation,
        BrowserAcquisitionFailureKind::Wait { wait_index: 0 },
        BrowserAcquisitionFailureKind::Interaction {
            interaction_index: 0,
        },
        BrowserAcquisitionFailureKind::ContentRead,
    ];

    for (index, kind) in failures.into_iter().enumerate() {
        let target = format!("https://example.test/failure-{index}");
        let (waits, interactions, mut events, expected_requests, expected_actions) = match &kind {
            BrowserAcquisitionFailureKind::RuntimeLaunch => {
                (Vec::new(), Vec::new(), Vec::new(), 0, 0)
            }
            BrowserAcquisitionFailureKind::Navigation => (Vec::new(), Vec::new(), Vec::new(), 1, 0),
            BrowserAcquisitionFailureKind::Wait { .. } => (
                vec![ExecutionPlanBrowserWait::Selector {
                    selector: "main".to_string(),
                    timeout_ms: 500,
                }],
                Vec::new(),
                vec![ScriptedBrowserAcquisitionEvent::Navigate],
                1,
                0,
            ),
            BrowserAcquisitionFailureKind::Interaction { .. } => (
                Vec::new(),
                vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
                    selector: ".more".to_string(),
                    max_count: 1,
                    wait_after_ms: None,
                }],
                vec![ScriptedBrowserAcquisitionEvent::Navigate],
                1,
                1,
            ),
            BrowserAcquisitionFailureKind::ContentRead => (
                Vec::new(),
                Vec::new(),
                vec![ScriptedBrowserAcquisitionEvent::Navigate],
                1,
                0,
            ),
            BrowserAcquisitionFailureKind::Deadline => unreachable!(),
        };
        let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
        let expected_failure = BrowserAcquisitionFailure::new(kind, "safe stage failure");
        events.push(ScriptedBrowserAcquisitionEvent::Failure(
            expected_failure.clone(),
        ));
        events.push(ScriptedBrowserAcquisitionEvent::Content(
            "must-not-run-after-primary".to_string(),
        ));
        let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
            request: BrowserAcquisitionRequestSnapshot {
                target: target.clone(),
                timeout_ms: 120_000,
                waits: waits.clone(),
                interactions: interactions.clone(),
                browser_rendered_bytes_remaining: PhaseLimits::BACKEND.max_browser_rendered_bytes,
            },
            events,
            finalization: ScriptedBrowserFinalization::default(),
        }]);

        let result = adapter
            .acquire(invocation.request(&target, waits, interactions))
            .await;

        assert_eq!(
            result,
            Err(BrowserAcquisitionTerminal::Failure(expected_failure))
        );
        let report = invocation.report(PhaseCompletion::ExecutionFailed);
        assert_eq!(report.usage.requests, expected_requests);
        assert_eq!(report.usage.browser_actions, expected_actions);
        assert_eq!(report.usage.browser_rendered_bytes, 0);
        assert!(adapter.lifecycle().ends_with(&[
            BrowserLifecycleEvent::GracefulClose,
            BrowserLifecycleEvent::HandlerCompleted,
            BrowserLifecycleEvent::ActiveSessionReleased,
            BrowserLifecycleEvent::SessionFinalized,
        ]));
    }
}

#[tokio::test(start_paused = true)]
async fn hard_deadline_comes_only_from_shared_control_and_finalizes_before_budget_return() {
    let limits = PhaseLimits {
        max_duration_ms: 2_001,
        ..PhaseLimits::BACKEND
    };
    let invocation = BrowserAcquisitionTestInvocation::new(limits, true, None);
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot_with_timeout(
            "https://example.test/deadline",
            2_001,
            PhaseLimits::BACKEND.max_browser_rendered_bytes,
        ),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Gate("work-deadline".to_string()),
            ScriptedBrowserAcquisitionEvent::Content("must-not-run".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let future = adapter.acquire(invocation.request(
        "https://example.test/deadline",
        Vec::new(),
        Vec::new(),
    ));
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => panic!("deadline gate unexpectedly completed: {result:?}"),
        _ = async {
            while !adapter.gate_is_waiting("work-deadline") {
                tokio::task::yield_now().await;
            }
        } => {}
    }

    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    assert_eq!(
        future.await,
        Err(BrowserAcquisitionTerminal::AllowanceStopped)
    );
    let report = invocation.report(PhaseCompletion::Accepted);
    let PhaseCompletion::BudgetExhausted { exhaustion } = report.completion else {
        panic!("shared duration allowance must own the deadline terminal")
    };
    assert_eq!(
        exhaustion.dimension,
        job_radar_lib::AllowanceDimension::Duration
    );
    assert!(adapter.lifecycle().ends_with(&[
        BrowserLifecycleEvent::HandlerCompleted,
        BrowserLifecycleEvent::ActiveSessionReleased,
        BrowserLifecycleEvent::SessionFinalized,
    ]));
}

#[test]
fn caller_tightening_rejects_browser_primitive_raises_before_acquisition() {
    let caller = PhaseLimits {
        max_duration_ms: 5_000,
        max_browser_actions: 1,
        ..PhaseLimits::BACKEND
    };
    let invocation =
        BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, Some(caller));

    let Err(timeout_error) =
        invocation.try_request_with_timeout("https://example.test", 5_001, Vec::new(), Vec::new())
    else {
        panic!("Fetch timeout may not raise caller duration ceiling")
    };
    assert_eq!(timeout_error.kind, BrowserAcquisitionFailureKind::Deadline);

    let Err(action_error) = invocation.try_request_with_timeout(
        "https://example.test",
        5_000,
        Vec::new(),
        vec![ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector: ".more".to_string(),
            max_count: 2,
            wait_after_ms: None,
        }],
    ) else {
        panic!("interaction may not raise caller action ceiling")
    };
    assert_eq!(
        action_error.kind,
        BrowserAcquisitionFailureKind::Interaction {
            interaction_index: 0
        }
    );
    let report = invocation.report(PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage.requests, 0);
    assert_eq!(report.usage.browser_actions, 0);
}

#[tokio::test(start_paused = true)]
async fn browser_fetch_timeout_tightens_work_without_exhausting_the_phase_duration() {
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: BrowserAcquisitionRequestSnapshot {
            target: "https://example.test/local-timeout".to_string(),
            timeout_ms: 1,
            waits: Vec::new(),
            interactions: Vec::new(),
            browser_rendered_bytes_remaining: PhaseLimits::BACKEND.max_browser_rendered_bytes,
        },
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Gate("local-timeout".to_string()),
            ScriptedBrowserAcquisitionEvent::Content("must-not-run".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    let future = adapter.acquire(invocation.request_with_timeout(
        "https://example.test/local-timeout",
        1,
        Vec::new(),
        Vec::new(),
    ));
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => panic!("local timeout gate unexpectedly completed: {result:?}"),
        _ = async {
            while !adapter.gate_is_waiting("local-timeout") {
                tokio::task::yield_now().await;
            }
        } => {}
    }

    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    let terminal = future
        .await
        .expect_err("local Browser timeout must stop work");
    assert!(matches!(
        terminal,
        BrowserAcquisitionTerminal::Failure(BrowserAcquisitionFailure {
            kind: BrowserAcquisitionFailureKind::Deadline,
            ..
        })
    ));
    let report = invocation.report(PhaseCompletion::ExecutionFailed);
    assert_eq!(report.completion, PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage.requests, 1);
    assert!(adapter.lifecycle().ends_with(&[
        BrowserLifecycleEvent::GracefulClose,
        BrowserLifecycleEvent::HandlerCompleted,
        BrowserLifecycleEvent::ActiveSessionReleased,
        BrowserLifecycleEvent::SessionFinalized,
    ]));
}

#[tokio::test]
async fn cleanup_invariant_loss_overrides_an_observed_success() {
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(
            "https://example.test",
            PhaseLimits::BACKEND.max_browser_rendered_bytes,
        ),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content("ok".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::InfrastructureFailure {
            message: "process reap was not established".to_string(),
        },
    }]);

    let result = adapter
        .acquire(invocation.request("https://example.test", Vec::new(), Vec::new()))
        .await;

    assert_eq!(
        result,
        Err(BrowserAcquisitionTerminal::InfrastructureFailure(
            BrowserInfrastructureFailure {
                message: "process reap was not established".to_string(),
            }
        ))
    );
    assert!(adapter.lifecycle().ends_with(&[
        BrowserLifecycleEvent::ReapFailed,
        BrowserLifecycleEvent::HandlerAborted,
        BrowserLifecycleEvent::ActiveSessionReleased,
        BrowserLifecycleEvent::SessionFinalized,
    ]));
}

struct CancellationFlag(AtomicBool);

impl RuntimeCancellation for CancellationFlag {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn active_cancellation_at_a_stage_gate_returns_only_after_cleanup() {
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let cancellation = CancellationFlag(AtomicBool::new(false));
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot(
            "https://example.test",
            PhaseLimits::BACKEND.max_browser_rendered_bytes,
        ),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Gate("navigation-complete".to_string()),
            ScriptedBrowserAcquisitionEvent::Content("must-not-be-exposed".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    }]);

    let future = adapter.acquire(invocation.request_with_cancellation(
        "https://example.test",
        Vec::new(),
        Vec::new(),
        &cancellation,
    ));
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => panic!("gate unexpectedly completed: {result:?}"),
        _ = async {
            while !adapter.gate_is_waiting("navigation-complete") {
                tokio::task::yield_now().await;
            }
        } => {}
    }
    cancellation.0.store(true, Ordering::SeqCst);

    let result = future.await;

    assert!(matches!(
        result,
        Err(BrowserAcquisitionTerminal::Cancelled(_))
    ));
    assert_eq!(
        invocation
            .report(PhaseCompletion::Accepted)
            .usage
            .browser_rendered_bytes,
        0
    );
    assert!(adapter.lifecycle().ends_with(&[
        BrowserLifecycleEvent::ForceTerminate,
        BrowserLifecycleEvent::Reaped,
        BrowserLifecycleEvent::HandlerCompleted,
        BrowserLifecycleEvent::ActiveSessionReleased,
        BrowserLifecycleEvent::SessionFinalized,
    ]));
    assert!(!adapter
        .lifecycle()
        .contains(&BrowserLifecycleEvent::ContentRead));
}

#[tokio::test]
async fn cancellation_during_forced_reap_waits_for_cleanup_without_overriding_budget() {
    let limits = PhaseLimits {
        max_browser_rendered_bytes: 1,
        ..PhaseLimits::BACKEND
    };
    let invocation = BrowserAcquisitionTestInvocation::new(limits, true, None);
    let cancellation = CancellationFlag(AtomicBool::new(false));
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: snapshot("https://example.test/oversized", 1),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content("too large".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::Forced {
            gate: Some("forced-reap".to_string()),
        },
    }]);
    let future = adapter.acquire(invocation.request_with_cancellation(
        "https://example.test/oversized",
        Vec::new(),
        Vec::new(),
        &cancellation,
    ));
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => panic!("forced reap gate unexpectedly completed: {result:?}"),
        _ = async {
            while !adapter.gate_is_waiting("forced-reap") {
                tokio::task::yield_now().await;
            }
        } => {}
    }

    cancellation.0.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(adapter.release_gate("forced-reap"));

    assert_eq!(
        future.await,
        Err(BrowserAcquisitionTerminal::AllowanceStopped)
    );
    assert!(matches!(
        invocation.report(PhaseCompletion::Accepted).completion,
        PhaseCompletion::BudgetExhausted { .. }
    ));
    assert!(adapter.lifecycle().ends_with(&[
        BrowserLifecycleEvent::Reaped,
        BrowserLifecycleEvent::HandlerCompleted,
        BrowserLifecycleEvent::ActiveSessionReleased,
        BrowserLifecycleEvent::SessionFinalized,
    ]));
}
