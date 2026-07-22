use job_radar_lib::{
    BrowserAcquisition, BrowserAcquisitionRequestSnapshot, BrowserAcquisitionTerminal,
    BrowserLifecycleEvent, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
    PhaseCompletion, PhaseLimits, ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization,
    __TestBrowserAcquisitionInvocation as BrowserAcquisitionTestInvocation,
};

fn request_snapshot(
    target: &str,
    waits: Vec<ExecutionPlanBrowserWait>,
    interactions: Vec<ExecutionPlanBrowserInteraction>,
) -> BrowserAcquisitionRequestSnapshot {
    BrowserAcquisitionRequestSnapshot {
        target: target.to_string(),
        waits,
        interactions,
        browser_rendered_bytes_remaining: PhaseLimits::BACKEND.max_browser_rendered_bytes,
    }
}

#[tokio::test]
async fn ordered_script_asserts_the_complete_phase_neutral_request_and_real_effect_debits() {
    let waits = vec![ExecutionPlanBrowserWait::Selector {
        selector: Some("main".to_string()),
        timeout_ms: 500,
    }];
    let interactions = vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
        selector: ".more".to_string(),
        max_count: 2,
        wait_after_ms: Some(10),
    }];
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let adapter = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: request_snapshot(
            "https://example.test/jobs",
            waits.clone(),
            interactions.clone(),
        ),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Wait { wait_index: 0 },
            ScriptedBrowserAcquisitionEvent::Interaction {
                interaction_index: 0,
                attempted_clicks: 2,
            },
            ScriptedBrowserAcquisitionEvent::Content("<main>jobs</main>".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::Forced { gate: None },
    }]);

    let content = adapter
        .acquire(invocation.request("https://example.test/jobs", waits, interactions))
        .await
        .expect("scripted acquisition succeeds");

    assert_eq!(content.as_str(), "<main>jobs</main>");
    assert!(adapter.expectations_satisfied());
    let report = invocation.report(PhaseCompletion::Accepted);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(report.usage.browser_actions, 2);
    assert_eq!(report.usage.browser_rendered_bytes, 17);
    assert_eq!(report.usage.response_bytes, 0);
    assert_eq!(
        adapter.lifecycle(),
        vec![
            BrowserLifecycleEvent::Reserved,
            BrowserLifecycleEvent::Navigation,
            BrowserLifecycleEvent::Wait { wait_index: 0 },
            BrowserLifecycleEvent::InteractionAttempt {
                interaction_index: 0,
            },
            BrowserLifecycleEvent::InteractionAttempt {
                interaction_index: 0,
            },
            BrowserLifecycleEvent::ContentRead,
            BrowserLifecycleEvent::PrimarySealed,
            BrowserLifecycleEvent::GracefulClose,
            BrowserLifecycleEvent::ForceTerminate,
            BrowserLifecycleEvent::Reaped,
            BrowserLifecycleEvent::HandlerCompleted,
            BrowserLifecycleEvent::ActiveSessionReleased,
            BrowserLifecycleEvent::SessionFinalized,
        ]
    );
}

#[tokio::test]
async fn mismatched_unexpected_and_missing_calls_are_detected_deterministically() {
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let expected = ScriptedBrowserAcquisitionExpectation {
        request: request_snapshot("https://example.test/expected", Vec::new(), Vec::new()),
        events: vec![
            ScriptedBrowserAcquisitionEvent::Navigate,
            ScriptedBrowserAcquisitionEvent::Content("ok".to_string()),
        ],
        finalization: ScriptedBrowserFinalization::default(),
    };
    let adapter = ScriptedBrowserAcquisition::new([expected]);

    let mismatch = adapter
        .acquire(invocation.request("https://example.test/actual", Vec::new(), Vec::new()))
        .await;
    assert!(matches!(
        mismatch,
        Err(BrowserAcquisitionTerminal::Failure(_))
    ));
    assert_eq!(adapter.mismatches().len(), 1);
    assert!(!adapter.expectations_satisfied());

    let unexpected = adapter
        .acquire(invocation.request("https://example.test/unexpected", Vec::new(), Vec::new()))
        .await;
    assert!(matches!(
        unexpected,
        Err(BrowserAcquisitionTerminal::Failure(_))
    ));
    assert_eq!(adapter.mismatches().len(), 2);

    let missing = ScriptedBrowserAcquisition::new([ScriptedBrowserAcquisitionExpectation {
        request: request_snapshot("https://example.test/missing", Vec::new(), Vec::new()),
        events: vec![ScriptedBrowserAcquisitionEvent::Failure(
            job_radar_lib::BrowserAcquisitionFailure::new(
                job_radar_lib::BrowserAcquisitionFailureKind::Navigation,
                "navigation failed",
            ),
        )],
        finalization: ScriptedBrowserFinalization::default(),
    }]);
    assert!(!missing.expectations_satisfied());
    assert!(missing.mismatches().is_empty());
}
