use job_radar_lib::{
    BrowserAcquisition, BrowserAcquisitionFailureKind, BrowserAcquisitionTerminal,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ManagedBrowserAcquisition,
    PhaseCompletion, PhaseLimits,
    __TestBrowserAcquisitionInvocation as BrowserAcquisitionTestInvocation,
};

static REAL_PROBE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn unavailable_pinned_runtime_is_a_typed_launch_failure_without_fallback_or_usage() {
    let runtime = tempfile::tempdir().unwrap();
    let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
    let adapter = ManagedBrowserAcquisition::new(runtime.path());

    let result = adapter
        .acquire(invocation.request(
            "data:text/html,%3Ch1%3Emanaged%3C%2Fh1%3E",
            Vec::new(),
            Vec::new(),
        ))
        .await;

    let Err(BrowserAcquisitionTerminal::Failure(failure)) = result else {
        panic!("unavailable runtime must be an ordinary typed failure: {result:?}");
    };
    assert_eq!(failure.kind, BrowserAcquisitionFailureKind::RuntimeLaunch);
    assert!(failure.message.contains("pinned managed Chromium"));

    let report = invocation.report(PhaseCompletion::ExecutionFailed);
    assert_eq!(report.usage.requests, 0);
    assert_eq!(report.usage.browser_actions, 0);
    assert_eq!(report.usage.browser_rendered_bytes, 0);
}

#[tokio::test]
async fn real_managed_adapter_probe_is_environment_gated_and_uses_the_final_interface() {
    let Ok(runtime_dir) = std::env::var("JOB_RADAR_BROWSER_RUNTIME_DIR") else {
        return;
    };
    let _probe_guard = REAL_PROBE_LOCK.lock().await;
    let limits = PhaseLimits {
        max_duration_ms: 20_000,
        max_browser_rendered_bytes: 64 * 1_024,
        ..PhaseLimits::BACKEND
    };
    let invocation = BrowserAcquisitionTestInvocation::new(limits, true, None);
    let adapter = ManagedBrowserAcquisition::new(&runtime_dir);

    let content = adapter
        .acquire(invocation.request(
            "data:text/html,%3Cmain%20id%3D%22probe%22%3Emanaged-adapter%3Cbutton%20id%3D%22remove%22%20onclick%3D%22this.remove()%22%3Eremove%3C%2Fbutton%3E%3Cdiv%20style%3D%22opacity%3A0%22%3E%3Cbutton%20id%3D%22hidden%22%3Ehidden%3C%2Fbutton%3E%3C%2Fdiv%3E%3C%2Fmain%3E",
            vec![
                ExecutionPlanBrowserWait::Selector {
                    selector: Some("#remove".to_string()),
                    timeout_ms: 500,
                },
                ExecutionPlanBrowserWait::NetworkIdle {
                    selector: None,
                    timeout_ms: 1_000,
                },
            ],
            vec![
                ExecutionPlanBrowserInteraction::ClickIfVisible {
                    selector: "#hidden".to_string(),
                    max_count: 1,
                    wait_after_ms: None,
                },
                ExecutionPlanBrowserInteraction::ClickUntilGone {
                    selector: "#remove".to_string(),
                    max_count: 1,
                    wait_after_ms: None,
                },
            ],
        ))
        .await
        .expect("managed adapter probe must acquire deterministic local content");

    assert!(content.as_str().contains("managed-adapter"));
    let report = invocation.report(PhaseCompletion::Accepted);
    assert_eq!(report.usage.requests, 1);
    assert_eq!(report.usage.browser_actions, 1);
    assert_eq!(report.usage.browser_rendered_bytes, content.utf8_len());
    assert!(!content.as_str().contains("id=\"remove\""));

    let one_byte_limits = PhaseLimits {
        max_duration_ms: 20_000,
        max_browser_rendered_bytes: 1,
        ..PhaseLimits::BACKEND
    };
    let limited = BrowserAcquisitionTestInvocation::new(one_byte_limits, true, None);
    assert_eq!(
        adapter
            .acquire(limited.request("data:text/html,too-large", Vec::new(), Vec::new()))
            .await,
        Err(BrowserAcquisitionTerminal::AllowanceStopped)
    );
    let limited_report = limited.report(PhaseCompletion::Accepted);
    assert_eq!(limited_report.usage.requests, 1);
    assert_eq!(limited_report.usage.browser_rendered_bytes, 1);

    let temporary = std::path::Path::new(&runtime_dir).join(".tmp");
    if temporary.is_dir() {
        assert!(
            std::fs::read_dir(temporary)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().starts_with("session-")),
            "managed adapter returned before session finalization"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn real_managed_adapter_probe_maps_session_finalization_loss_to_infrastructure_failure() {
    let Ok(runtime_dir) = std::env::var("JOB_RADAR_BROWSER_RUNTIME_DIR") else {
        return;
    };
    let _probe_guard = REAL_PROBE_LOCK.lock().await;
    let invocation = BrowserAcquisitionTestInvocation::new(
        PhaseLimits {
            max_duration_ms: 20_000,
            ..PhaseLimits::BACKEND
        },
        true,
        None,
    );
    let adapter = ManagedBrowserAcquisition::new(&runtime_dir);
    let future = adapter.acquire(invocation.request(
        "data:text/html,%3Cmain%3Einfrastructure-probe%3C%2Fmain%3E",
        Vec::new(),
        Vec::new(),
    ));
    tokio::pin!(future);
    let session_dir = tokio::select! {
        result = &mut future => panic!("managed probe returned before fault injection: {result:?}"),
        session_dir = wait_for_connected_session(std::path::Path::new(&runtime_dir)) => session_dir,
    };
    let permission_fault = SessionPermissionFault::new(session_dir.clone());

    let result = future.await;

    assert!(matches!(
        result,
        Err(BrowserAcquisitionTerminal::InfrastructureFailure(_))
    ));
    drop(permission_fault);
    assert!(!session_dir.exists(), "fault-probe residue was not removed");
}

#[cfg(unix)]
async fn wait_for_connected_session(runtime_dir: &std::path::Path) -> std::path::PathBuf {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Ok(entries) = std::fs::read_dir(runtime_dir.join(".tmp")) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with("session-")
                    && path.join("DevToolsActivePort").is_file()
                {
                    return path;
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "managed probe did not publish a connected session"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
struct SessionPermissionFault(std::path::PathBuf);

#[cfg(unix)]
impl SessionPermissionFault {
    fn new(path: std::path::PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
        Self(path)
    }
}

#[cfg(unix)]
impl Drop for SessionPermissionFault {
    fn drop(&mut self) {
        restore_directory_permissions(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn restore_directory_permissions(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_dir() {
                restore_directory_permissions(&entry.path());
            }
        }
    }
}
