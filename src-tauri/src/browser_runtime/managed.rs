use chromiumoxide::Page;
use serde::Deserialize;
use std::{
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::profile_dsl::{
    execution_plan::capabilities::{ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait},
    runtime::{
        browser_acquisition::BoxedBrowserAcquisitionFuture, BrowserAcquisition,
        BrowserAcquisitionCancellation, BrowserAcquisitionCancellationReason,
        BrowserAcquisitionFailure, BrowserAcquisitionFailureKind, BrowserAcquisitionRequest,
        BrowserAcquisitionTerminal, BrowserInfrastructureFailure, BrowserRenderedContent,
    },
};

const NETWORK_TRACKER_SCRIPT: &str = r#"
(() => {
  if (globalThis.__jobRadarNetworkTrackerInstalled) return;
  globalThis.__jobRadarNetworkTrackerInstalled = true;
  globalThis.__jobRadarPendingRequests = 0;
  const originalFetch = globalThis.fetch;
  if (originalFetch) {
    globalThis.fetch = (...args) => {
      globalThis.__jobRadarPendingRequests += 1;
      return originalFetch(...args).finally(() => {
        globalThis.__jobRadarPendingRequests = Math.max(0, globalThis.__jobRadarPendingRequests - 1);
      });
    };
  }
  const originalSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function(...args) {
    globalThis.__jobRadarPendingRequests += 1;
    this.addEventListener('loadend', () => {
      globalThis.__jobRadarPendingRequests = Math.max(0, globalThis.__jobRadarPendingRequests - 1);
    }, { once: true });
    return originalSend.apply(this, args);
  };
})();
"#;

use super::owned::{
    OwnedChromiumDeadlines, OwnedChromiumError, OwnedChromiumErrorKind, OwnedChromiumLauncher,
    OwnedChromiumSession,
};

/// Production Browser acquisition edge backed only by Job Radar's pinned,
/// locally managed Chromium runtime.
pub struct ManagedBrowserAcquisition {
    runtime_dir: PathBuf,
}

impl ManagedBrowserAcquisition {
    pub fn new(runtime_dir: impl AsRef<Path>) -> Self {
        Self {
            runtime_dir: runtime_dir.as_ref().to_path_buf(),
        }
    }
}

impl BrowserAcquisition for ManagedBrowserAcquisition {
    fn acquire<'a>(
        &'a self,
        request: BrowserAcquisitionRequest<'a>,
    ) -> BoxedBrowserAcquisitionFuture<'a> {
        Box::pin(async move { acquire_managed(&self.runtime_dir, request).await })
    }
}

async fn acquire_managed(
    runtime_dir: &Path,
    request: BrowserAcquisitionRequest<'_>,
) -> Result<BrowserRenderedContent, BrowserAcquisitionTerminal> {
    ensure_work_control(&request)?;
    let launcher = match OwnedChromiumLauncher::from_installed_runtime(runtime_dir) {
        Ok(launcher) => launcher,
        Err(error) => {
            return apply_late_control(&request, Err(map_owned_launch_error(error)));
        }
    };
    let deadlines = OwnedChromiumDeadlines {
        work: request.browser_work_deadline(),
        graceful: request.browser_graceful_deadline(),
        force: request.browser_force_deadline(),
        handler: request.browser_handler_deadline(),
        finalize: request.hard_deadline(),
    };
    let mut session = match launcher.launch(deadlines, request.cancelled()).await {
        Ok(session) => session,
        Err(error) => {
            if error.kind == OwnedChromiumErrorKind::Deadline {
                request.mark_deadline();
            }
            return apply_late_control(&request, Err(map_owned_launch_error(error)));
        }
    };

    let primary = acquire_page(&mut session, &request).await;
    let cleanup = session.shutdown().await;
    if let Err(error) = cleanup {
        return Err(BrowserAcquisitionTerminal::InfrastructureFailure(
            BrowserInfrastructureFailure {
                message: error.message,
            },
        ));
    }

    apply_late_control(&request, primary)
}

fn apply_late_control<T>(
    request: &BrowserAcquisitionRequest<'_>,
    primary: Result<T, BrowserAcquisitionTerminal>,
) -> Result<T, BrowserAcquisitionTerminal> {
    if matches!(
        primary,
        Err(BrowserAcquisitionTerminal::InfrastructureFailure(_)
            | BrowserAcquisitionTerminal::AllowanceStopped)
    ) {
        return primary;
    }
    if request.is_cancelled() {
        return Err(cancelled_terminal());
    }
    primary
}

async fn acquire_page(
    session: &mut OwnedChromiumSession,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<BrowserRenderedContent, BrowserAcquisitionTerminal> {
    ensure_work_control(request)?;
    let page = controlled(request, session.browser_mut().new_page("about:blank"))
        .await?
        .map_err(|error| {
            stage_failure(
                BrowserAcquisitionFailureKind::RuntimeLaunch,
                format!("managed Chromium page creation failed: {error}"),
            )
        })?;
    if request
        .waits
        .iter()
        .any(|wait| matches!(wait, ExecutionPlanBrowserWait::NetworkIdle { .. }))
    {
        controlled(
            request,
            page.evaluate_on_new_document(NETWORK_TRACKER_SCRIPT),
        )
        .await?
        .map_err(|error| {
            stage_failure(
                BrowserAcquisitionFailureKind::RuntimeLaunch,
                format!("managed Chromium network tracker setup failed: {error}"),
            )
        })?;
    }

    ensure_work_control(request)?;
    request.admit_navigation()?;
    controlled(request, page.goto(&request.target))
        .await?
        .map_err(|error| {
            stage_failure(
                BrowserAcquisitionFailureKind::Navigation,
                format!("managed Chromium navigation failed: {error}"),
            )
        })?;

    for (wait_index, wait) in request.waits.iter().enumerate() {
        ensure_work_control(request)?;
        request.admit_wait()?;
        apply_wait(&page, wait, wait_index, request).await?;
    }
    for (interaction_index, interaction) in request.interactions.iter().enumerate() {
        apply_interaction(&page, interaction, interaction_index, request).await?;
    }

    ensure_work_control(request)?;
    let content = controlled(request, page.content())
        .await?
        .map_err(|error| {
            stage_failure(
                BrowserAcquisitionFailureKind::ContentRead,
                format!("managed Chromium rendered-content read failed: {error}"),
            )
        })?;
    request.admit_rendered_content(content)
}

async fn apply_wait(
    page: &Page,
    wait: &ExecutionPlanBrowserWait,
    wait_index: usize,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<(), BrowserAcquisitionTerminal> {
    match wait {
        ExecutionPlanBrowserWait::Selector {
            selector,
            timeout_ms,
        } => {
            let selector = selector.as_ref().ok_or_else(|| {
                stage_failure(
                    BrowserAcquisitionFailureKind::Wait { wait_index },
                    "managed Chromium selector wait is missing a selector",
                )
            })?;
            wait_for_selector(page, selector, *timeout_ms, wait_index, request).await
        }
        ExecutionPlanBrowserWait::NetworkIdle {
            selector,
            timeout_ms,
        } => {
            if let Some(selector) = selector {
                wait_for_selector(page, selector, *timeout_ms, wait_index, request).await?;
            }
            wait_for_network_idle(page, *timeout_ms, wait_index, request).await
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct NetworkState {
    ready: bool,
    pending_requests: u64,
    resource_count: u64,
    last_response_end: f64,
}

async fn wait_for_network_idle(
    page: &Page,
    timeout_ms: u64,
    wait_index: usize,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<(), BrowserAcquisitionTerminal> {
    const QUIET_MS: u64 = 500;
    const POLL_MS: u64 = 50;

    let deadline = bounded_local_deadline(request, timeout_ms);
    let mut stable: Option<(NetworkState, tokio::time::Instant)> = None;
    loop {
        let evaluation = match controlled_until(
            request,
            page.evaluate("(() => { const resources = performance.getEntriesByType('resource'); return { ready: document.readyState === 'complete', pendingRequests: Number(globalThis.__jobRadarPendingRequests || 0), resourceCount: resources.length, lastResponseEnd: resources.reduce((latest, entry) => Math.max(latest, entry.responseEnd || 0), 0) }; })()"),
            deadline,
        )
        .await?
        {
            Some(result) => result.map_err(|error| {
                stage_failure(
                    BrowserAcquisitionFailureKind::Wait { wait_index },
                    format!("managed Chromium network-idle observation failed: {error}"),
                )
            })?,
            None => return Err(network_idle_timeout(wait_index, timeout_ms)),
        };
        let state = evaluation.into_value::<NetworkState>().map_err(|error| {
            stage_failure(
                BrowserAcquisitionFailureKind::Wait { wait_index },
                format!("managed Chromium network-idle state was invalid: {error}"),
            )
        })?;
        let now = tokio::time::Instant::now();
        if state.ready && state.pending_requests == 0 {
            match stable {
                Some((previous, since)) if previous == state => {
                    if now.duration_since(since) >= Duration::from_millis(QUIET_MS) {
                        return Ok(());
                    }
                }
                _ => stable = Some((state, now)),
            }
        } else {
            stable = None;
        }
        if now >= deadline {
            return Err(network_idle_timeout(wait_index, timeout_ms));
        }
        controlled_sleep(
            request,
            deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(POLL_MS)),
        )
        .await?;
    }
}

fn bounded_local_deadline(
    request: &BrowserAcquisitionRequest<'_>,
    timeout_ms: u64,
) -> tokio::time::Instant {
    tokio::time::Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .unwrap_or_else(|| request.browser_work_deadline())
        .min(request.browser_work_deadline())
}

fn network_idle_timeout(wait_index: usize, timeout_ms: u64) -> BrowserAcquisitionTerminal {
    stage_failure(
        BrowserAcquisitionFailureKind::Wait { wait_index },
        format!("managed Chromium network did not become idle within {timeout_ms} ms"),
    )
}

async fn wait_for_selector(
    page: &Page,
    selector: &str,
    timeout_ms: u64,
    wait_index: usize,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<(), BrowserAcquisitionTerminal> {
    let deadline = bounded_local_deadline(request, timeout_ms);
    loop {
        ensure_work_control(request)?;
        match controlled_until(request, page.find_element(selector.to_string()), deadline).await? {
            Some(Ok(_)) => return Ok(()),
            Some(Err(error)) if tokio::time::Instant::now() >= deadline => {
                return Err(stage_failure(
                    BrowserAcquisitionFailureKind::Wait { wait_index },
                    format!(
                        "managed Chromium selector `{selector}` was not found within {timeout_ms} ms: {error}"
                    ),
                ));
            }
            Some(Err(_)) => {}
            None => {
                return Err(stage_failure(
                    BrowserAcquisitionFailureKind::Wait { wait_index },
                    format!(
                        "managed Chromium selector `{selector}` was not found within {timeout_ms} ms"
                    ),
                ));
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        controlled_sleep(request, remaining.min(Duration::from_millis(100))).await?;
    }
}

async fn apply_interaction(
    page: &Page,
    interaction: &ExecutionPlanBrowserInteraction,
    interaction_index: usize,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<(), BrowserAcquisitionTerminal> {
    let (selector, max_count, wait_after_ms, must_disappear) = match interaction {
        ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => (selector, *max_count, *wait_after_ms, false),
        ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => (selector, *max_count, *wait_after_ms, true),
    };

    for _ in 0..max_count {
        ensure_work_control(request)?;
        if !selector_is_visible(page, selector, interaction_index, request).await? {
            return Ok(());
        }
        let element = controlled(request, page.find_element(selector.clone()))
            .await?
            .map_err(|error| {
                stage_failure(
                    BrowserAcquisitionFailureKind::Interaction { interaction_index },
                    format!(
                        "managed Chromium could not resolve visible selector `{selector}`: {error}"
                    ),
                )
            })?;
        ensure_work_control(request)?;
        request.admit_interaction()?;
        controlled(request, element.click())
            .await?
            .map_err(|error| {
                stage_failure(
                    BrowserAcquisitionFailureKind::Interaction { interaction_index },
                    format!("managed Chromium click failed for selector `{selector}`: {error}"),
                )
            })?;
        if let Some(wait_after_ms) = wait_after_ms {
            controlled_sleep(request, Duration::from_millis(wait_after_ms)).await?;
        }
    }

    if must_disappear {
        ensure_work_control(request)?;
        if selector_is_visible(page, selector, interaction_index, request).await? {
            return Err(stage_failure(
                BrowserAcquisitionFailureKind::Interaction { interaction_index },
                format!(
                    "managed Chromium click_until_gone reached maxCount {max_count} while selector `{selector}` remained visible"
                ),
            ));
        }
    }
    Ok(())
}

async fn selector_is_visible(
    page: &Page,
    selector: &str,
    interaction_index: usize,
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<bool, BrowserAcquisitionTerminal> {
    let selector_literal = serde_json::to_string(selector).expect("CSS selector is serializable");
    let evaluation = controlled(
        request,
        page.evaluate(format!(
            "(() => {{ const element = document.querySelector({selector_literal}); if (!element || !element.isConnected) return false; if (typeof element.checkVisibility === 'function') return element.checkVisibility({{ checkOpacity: true, checkVisibilityCSS: true }}); for (let current = element; current; current = current.parentElement) {{ const style = getComputedStyle(current); if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false; }} const rect = element.getBoundingClientRect(); return rect.width > 0 && rect.height > 0; }})()"
        )),
    )
    .await?
    .map_err(|error| {
        stage_failure(
            BrowserAcquisitionFailureKind::Interaction { interaction_index },
            format!("managed Chromium selector visibility check failed: {error}"),
        )
    })?;
    evaluation.into_value::<bool>().map_err(|error| {
        stage_failure(
            BrowserAcquisitionFailureKind::Interaction { interaction_index },
            format!("managed Chromium selector visibility result was invalid: {error}"),
        )
    })
}

async fn controlled<T, E>(
    request: &BrowserAcquisitionRequest<'_>,
    operation: impl Future<Output = Result<T, E>>,
) -> Result<Result<T, E>, BrowserAcquisitionTerminal> {
    tokio::select! {
        biased;
        _ = request.cancelled() => Err(cancelled_terminal()),
        result = operation => Ok(result),
        _ = tokio::time::sleep_until(request.browser_work_deadline()) => {
            request.mark_deadline();
            Err(BrowserAcquisitionTerminal::AllowanceStopped)
        }
    }
}

async fn controlled_until<T, E>(
    request: &BrowserAcquisitionRequest<'_>,
    operation: impl Future<Output = Result<T, E>>,
    local_deadline: tokio::time::Instant,
) -> Result<Option<Result<T, E>>, BrowserAcquisitionTerminal> {
    tokio::select! {
        biased;
        _ = request.cancelled() => Err(cancelled_terminal()),
        result = operation => Ok(Some(result)),
        _ = tokio::time::sleep_until(request.browser_work_deadline()) => {
            request.mark_deadline();
            Err(BrowserAcquisitionTerminal::AllowanceStopped)
        }
        _ = tokio::time::sleep_until(local_deadline) => Ok(None),
    }
}

async fn controlled_sleep(
    request: &BrowserAcquisitionRequest<'_>,
    duration: Duration,
) -> Result<(), BrowserAcquisitionTerminal> {
    tokio::select! {
        biased;
        _ = request.cancelled() => Err(cancelled_terminal()),
        _ = tokio::time::sleep(duration) => Ok(()),
        _ = tokio::time::sleep_until(request.browser_work_deadline()) => {
            request.mark_deadline();
            Err(BrowserAcquisitionTerminal::AllowanceStopped)
        }
    }
}

fn ensure_work_control(
    request: &BrowserAcquisitionRequest<'_>,
) -> Result<(), BrowserAcquisitionTerminal> {
    if request.is_cancelled() {
        return Err(cancelled_terminal());
    }
    if tokio::time::Instant::now() >= request.browser_work_deadline() {
        request.mark_deadline();
        return Err(BrowserAcquisitionTerminal::AllowanceStopped);
    }
    Ok(())
}

fn map_owned_launch_error(error: OwnedChromiumError) -> BrowserAcquisitionTerminal {
    match error.kind {
        OwnedChromiumErrorKind::Cleanup => {
            BrowserAcquisitionTerminal::InfrastructureFailure(BrowserInfrastructureFailure {
                message: error.message,
            })
        }
        OwnedChromiumErrorKind::Cancelled => cancelled_terminal(),
        OwnedChromiumErrorKind::Deadline => BrowserAcquisitionTerminal::AllowanceStopped,
        OwnedChromiumErrorKind::RuntimeUnavailable
        | OwnedChromiumErrorKind::InvalidControl
        | OwnedChromiumErrorKind::Launch
        | OwnedChromiumErrorKind::EndpointDiscovery
        | OwnedChromiumErrorKind::Connect => {
            stage_failure(BrowserAcquisitionFailureKind::RuntimeLaunch, error.message)
        }
    }
}

fn stage_failure(
    kind: BrowserAcquisitionFailureKind,
    message: impl Into<String>,
) -> BrowserAcquisitionTerminal {
    BrowserAcquisitionTerminal::Failure(BrowserAcquisitionFailure::new(kind, message))
}

fn cancelled_terminal() -> BrowserAcquisitionTerminal {
    BrowserAcquisitionTerminal::Cancelled(BrowserAcquisitionCancellation {
        reason: BrowserAcquisitionCancellationReason::UserCancelled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        browser_runtime::current_runtime_spec,
        profile_dsl::{
            documents::PhaseLimits, runtime::browser_acquisition::BrowserAcquisitionTestInvocation,
        },
    };
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct CancellationFlag(AtomicBool);

    impl crate::profile_dsl::runtime::RuntimeCancellation for CancellationFlag {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[cfg(unix)]
    fn install_exiting_runtime(runtime_dir: &Path) {
        let spec = current_runtime_spec().expect("test platform has a managed runtime");
        let install_dir = format!("{}/{}", spec.platform, spec.version);
        let executable = runtime_dir
            .join(&install_dir)
            .join(&spec.relative_executable_path);
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(
            runtime_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": 1,
                "runtimeKind": "chrome-for-testing",
                "platform": spec.platform,
                "version": spec.version,
                "downloadUrl": spec.download_url,
                "archiveSha256": spec.expected_archive_sha256,
                "installDir": install_dir,
                "executablePath": spec.relative_executable_path,
                "installedAt": "test"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn oversized_wait_timeout_is_capped_to_the_caller_owned_work_deadline() {
        let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
        let request = invocation.request("data:text/html,test", Vec::new(), Vec::new());

        assert_eq!(
            bounded_local_deadline(&request, u64::MAX),
            request.browser_work_deadline()
        );
    }

    #[test]
    fn late_cancellation_replaces_ordinary_launch_failure_but_not_cleanup_or_allowance_loss() {
        let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
        let cancellation = CancellationFlag(AtomicBool::new(true));
        let request = invocation.request_with_cancellation(
            "data:text/html,test",
            Vec::new(),
            Vec::new(),
            &cancellation,
        );
        let ordinary: Result<(), _> = Err(stage_failure(
            BrowserAcquisitionFailureKind::RuntimeLaunch,
            "endpoint failed",
        ));

        assert!(matches!(
            apply_late_control(&request, ordinary),
            Err(BrowserAcquisitionTerminal::Cancelled(_))
        ));
        assert_eq!(
            apply_late_control::<()>(&request, Err(BrowserAcquisitionTerminal::AllowanceStopped)),
            Err(BrowserAcquisitionTerminal::AllowanceStopped)
        );
        let infrastructure =
            BrowserAcquisitionTerminal::InfrastructureFailure(BrowserInfrastructureFailure {
                message: "reap unconfirmed".to_string(),
            });
        assert_eq!(
            apply_late_control::<()>(&request, Err(infrastructure.clone())),
            Err(infrastructure)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn owned_endpoint_failure_is_mapped_and_finalized_through_b01() {
        let runtime = tempfile::tempdir().unwrap();
        install_exiting_runtime(runtime.path());
        let invocation = BrowserAcquisitionTestInvocation::new(PhaseLimits::BACKEND, false, None);
        let adapter = ManagedBrowserAcquisition::new(runtime.path());

        let result = adapter
            .acquire(invocation.request("data:text/html,never", Vec::new(), Vec::new()))
            .await;

        let Err(BrowserAcquisitionTerminal::Failure(failure)) = result else {
            panic!("failed owned launch must remain an ordinary typed failure: {result:?}");
        };
        assert_eq!(failure.kind, BrowserAcquisitionFailureKind::RuntimeLaunch);
        assert!(failure.message.contains("exited before publishing"));
        let temporary = runtime.path().join(".tmp");
        assert!(
            !temporary.exists() || std::fs::read_dir(temporary).unwrap().next().is_none(),
            "failed launch returned before private session finalization"
        );
    }
}
