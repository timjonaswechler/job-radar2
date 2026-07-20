use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    Page,
};
use futures_util::StreamExt;
use std::{future::Future, io, path::Path, pin::Pin, time::Duration};
use uuid::Uuid;

const SESSION_CLEANUP_ATTEMPTS: usize = 3;
const SESSION_CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(50);

use super::{
    BrowserRuntimeInteraction, BrowserRuntimeRenderError, BrowserRuntimeRenderErrorKind,
    BrowserRuntimeRenderRequest, BrowserRuntimeWait,
};
use crate::profile_dsl::runtime::{
    allowance::{
        AllowanceCharge, BROWSER_FORCE_TERMINATE_REAP_MS, BROWSER_GRACEFUL_CLOSE_MS,
        BROWSER_HANDLER_COMPLETION_MS, BROWSER_SESSION_FINALIZATION_MS,
    },
    RuntimeExecutionContext,
};

pub async fn smoke_test(executable_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let session_dir = runtime_session_dir(runtime_dir);
    let _active_session = super::begin_active_browser_session(&session_dir);
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|error| error.to_string())?;

    let result = smoke_test_with_session(executable_path, &session_dir).await;
    let cleanup_result = cleanup_session_dir_best_effort(&session_dir).await;

    smoke_result_after_session_cleanup(result, cleanup_result)
}

pub(crate) async fn render_page_html_with_actions_and_context(
    executable_path: &Path,
    runtime_dir: &Path,
    request: BrowserRuntimeRenderRequest,
    context: RuntimeExecutionContext<'_>,
) -> Result<String, BrowserRuntimeRenderError> {
    ensure_not_cancelled(context)?;
    let session_dir = runtime_session_dir(runtime_dir);
    let _active_session = super::begin_active_browser_session(&session_dir);
    let session_create = tokio::select! {
        biased;
        _ = context.cancelled() => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::Cancelled,
            "Managed browser runtime execution cancelled during session setup",
        )),
        result = tokio::fs::create_dir_all(&session_dir) => result.map_err(|error| BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
            error.to_string(),
        )),
        _ = context.browser_work_deadline_reached() => {
            if context.is_cancelled() {
                Err(BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::Cancelled, "Managed browser runtime execution cancelled during session setup"))
            } else {
                context.mark_deadline();
                Err(BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::RenderTimeout,
                    "Managed browser runtime stopped session setup before the teardown reserve",
                ))
            }
        }
    };

    let result = match session_create {
        Ok(()) => {
            render_page_html_with_session(executable_path, &session_dir, request, context).await
        }
        Err(error) => Err(error),
    };
    let cleanup_result = match context.deadline() {
        Some(phase_deadline) => {
            let result = finalize_session_with_deadline(
                cleanup_session_dir_best_effort(&session_dir),
                phase_deadline,
            )
            .await;
            if result
                .as_ref()
                .is_err_and(|error| error.kind() == io::ErrorKind::TimedOut)
            {
                context.mark_deadline_if_expired();
            }
            result
        }
        None => cleanup_session_dir_best_effort(&session_dir).await,
    };

    render_result_after_session_cleanup(result, cleanup_result)
}

fn runtime_session_dir(runtime_dir: &Path) -> std::path::PathBuf {
    runtime_dir
        .join(".tmp")
        .join(format!("session-{}", Uuid::new_v4()))
}

fn capped_stage_deadline(
    absolute_boundary: tokio::time::Instant,
    maximum_duration: Duration,
) -> tokio::time::Instant {
    absolute_boundary.min(tokio::time::Instant::now() + maximum_duration)
}

async fn finalize_session_with_deadline<F>(
    cleanup: F,
    phase_deadline: tokio::time::Instant,
) -> Result<(), io::Error>
where
    F: Future<Output = Result<(), io::Error>>,
{
    let finalization_deadline = capped_stage_deadline(
        phase_deadline,
        Duration::from_millis(BROWSER_SESSION_FINALIZATION_MS),
    );
    tokio::select! {
        biased;
        result = cleanup => result,
        _ = tokio::time::sleep_until(finalization_deadline) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "browser session cleanup exceeded its 250 ms slice",
        )),
    }
}

async fn cleanup_session_dir_best_effort(session_dir: &Path) -> Result<(), io::Error> {
    let mut last_error = None;

    for attempt in 0..SESSION_CLEANUP_ATTEMPTS {
        match tokio::fs::remove_dir_all(session_dir).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < SESSION_CLEANUP_ATTEMPTS {
                    tokio::time::sleep(SESSION_CLEANUP_RETRY_DELAY).await;
                }
            }
        }
    }

    Err(last_error.expect("session cleanup should have produced an error before retries ended"))
}

pub(super) fn smoke_result_after_session_cleanup(
    result: Result<(), String>,
    _cleanup_result: Result<(), io::Error>,
) -> Result<(), String> {
    result
}

pub(super) fn render_result_after_session_cleanup(
    result: Result<String, BrowserRuntimeRenderError>,
    _cleanup_result: Result<(), io::Error>,
) -> Result<String, BrowserRuntimeRenderError> {
    result
}

fn ensure_not_cancelled(
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    if context.is_cancelled() {
        Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::Cancelled,
            "Managed browser runtime execution cancelled",
        ))
    } else {
        Ok(())
    }
}

async fn cancellable_sleep(
    duration: Duration,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    ensure_not_cancelled(context)?;
    tokio::select! {
        _ = tokio::time::sleep(duration) => Ok(()),
        _ = context.cancelled() => ensure_not_cancelled(context),
    }
}

async fn apply_wait(
    page: &Page,
    url: &str,
    wait: &BrowserRuntimeWait,
    wait_index: usize,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    match wait {
        BrowserRuntimeWait::Selector {
            selector,
            timeout_ms,
        } => {
            let selector = selector.as_deref().ok_or_else(|| {
                BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::WaitTimeout {
                        wait_index: Some(wait_index),
                    },
                    "Managed browser runtime selector wait is missing a selector",
                )
            })?;
            wait_for_selector(page, url, selector, *timeout_ms, wait_index, context).await
        }
        BrowserRuntimeWait::NetworkIdle {
            selector,
            timeout_ms,
        } => {
            if let Some(selector) = selector {
                wait_for_selector(page, url, selector, *timeout_ms, wait_index, context).await?;
            }
            cancellable_sleep(Duration::from_millis(*timeout_ms), context).await
        }
    }
}

async fn wait_for_selector(
    page: &Page,
    url: &str,
    selector: &str,
    timeout_ms: u64,
    wait_index: usize,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    let timeout = Duration::from_millis(timeout_ms);
    let started_at = tokio::time::Instant::now();

    loop {
        ensure_not_cancelled(context)?;
        let element_result = page.find_element(selector.to_string()).await;
        ensure_not_cancelled(context)?;
        let error = match element_result {
            Ok(_) => return Ok(()),
            Err(error) => error.to_string(),
        };

        if started_at.elapsed() >= timeout {
            return Err(BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::WaitTimeout {
                    wait_index: Some(wait_index),
                },
                format!(
                    "Managed browser runtime waitFor selector `{selector}` was not found for {url} within {timeout_ms} ms: {error}"
                ),
            ));
        }

        cancellable_sleep(Duration::from_millis(100), context).await?;
    }
}

fn admit_browser_click(
    context: RuntimeExecutionContext<'_>,
    interaction_index: usize,
) -> Result<(), BrowserRuntimeRenderError> {
    ensure_not_cancelled(context)?;
    context
        .debit(AllowanceCharge {
            browser_actions: 1,
            ..AllowanceCharge::default()
        })
        .map_err(|_| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::InteractionFailed {
                    interaction_index: Some(interaction_index),
                },
                "Managed browser runtime click denied by cumulative phase allowance",
            )
        })?;
    ensure_not_cancelled(context)
}

async fn apply_interaction(
    page: &Page,
    interaction: &BrowserRuntimeInteraction,
    interaction_index: usize,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    match interaction {
        BrowserRuntimeInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => {
            for _ in 0..*max_count {
                ensure_not_cancelled(context)?;
                let Ok(element) = page.find_element(selector.clone()).await else {
                    return Ok(());
                };
                admit_browser_click(context, interaction_index)?;
                element.click().await.map_err(|error| {
                    BrowserRuntimeRenderError::new(
                        BrowserRuntimeRenderErrorKind::InteractionFailed {
                            interaction_index: Some(interaction_index),
                        },
                        format!(
                            "Managed browser runtime click_if_visible failed for selector `{selector}`: {error}"
                        ),
                    )
                })?;
                ensure_not_cancelled(context)?;
                sleep_after_interaction(*wait_after_ms, context).await?;
            }
            Ok(())
        }
        BrowserRuntimeInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => {
            for _ in 0..*max_count {
                ensure_not_cancelled(context)?;
                let Ok(element) = page.find_element(selector.clone()).await else {
                    return Ok(());
                };
                admit_browser_click(context, interaction_index)?;
                element.click().await.map_err(|error| {
                    BrowserRuntimeRenderError::new(
                        BrowserRuntimeRenderErrorKind::InteractionFailed {
                            interaction_index: Some(interaction_index),
                        },
                        format!(
                            "Managed browser runtime click_until_gone failed for selector `{selector}`: {error}"
                        ),
                    )
                })?;
                ensure_not_cancelled(context)?;
                sleep_after_interaction(*wait_after_ms, context).await?;
            }

            ensure_not_cancelled(context)?;
            if page.find_element(selector.clone()).await.is_ok() {
                return Err(BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::InteractionFailed {
                        interaction_index: Some(interaction_index),
                    },
                    format!(
                        "Managed browser runtime click_until_gone reached maxCount {max_count} while selector `{selector}` was still visible"
                    ),
                ));
            }
            Ok(())
        }
    }
}

async fn sleep_after_interaction(
    wait_after_ms: Option<u64>,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    if let Some(wait_after_ms) = wait_after_ms {
        cancellable_sleep(Duration::from_millis(wait_after_ms), context).await?;
    }
    Ok(())
}

async fn smoke_test_with_session(executable_path: &Path, session_dir: &Path) -> Result<(), String> {
    let (mut browser, handler_task) = launch_browser(executable_path, session_dir).await?;

    let page_result = browser
        .new_page("about:blank")
        .await
        .map(|_| ())
        .map_err(|error| format!("Managed browser runtime smoke page failed: {error}"));

    let close_result = browser
        .close()
        .await
        .map(|_| ())
        .map_err(|error| format!("Managed browser runtime failed to close: {error}"));
    let _ = handler_task.await;

    page_result.and(close_result)
}

async fn render_page_html_with_session(
    executable_path: &Path,
    session_dir: &Path,
    request: BrowserRuntimeRenderRequest,
    context: RuntimeExecutionContext<'_>,
) -> Result<String, BrowserRuntimeRenderError> {
    ensure_not_cancelled(context)?;
    let launch_result = tokio::select! {
        biased;
        _ = context.cancelled() => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::Cancelled,
            "Managed browser runtime execution cancelled during launch",
        )),
        result = launch_browser(executable_path, session_dir) => result.map_err(|error| {
            BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::RuntimeUnavailable, error)
        }),
        _ = context.browser_work_deadline_reached() => {
            if context.is_cancelled() {
                Err(BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::Cancelled, "Managed browser runtime execution cancelled during launch"))
            } else {
                context.mark_deadline();
                Err(BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::RenderTimeout,
                    "Managed browser runtime stopped launch before the teardown reserve",
                ))
            }
        }
    };
    let (mut browser, mut handler_task) = launch_result?;

    let url = request.url.clone();
    let timeout = Duration::from_millis(request.timeout_ms);
    let page_result = {
        let page_operation = tokio::time::timeout(timeout, async {
            ensure_not_cancelled(context)?;
            let page = browser.new_page("about:blank").await.map_err(|error| {
                BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
                    format!("Managed browser runtime page failed: {error}"),
                )
            })?;
            ensure_not_cancelled(context)?;
            page.goto(&request.url).await.map_err(|error| {
                BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::NavigationFailed,
                    format!(
                        "Managed browser runtime navigation failed for {}: {error}",
                        request.url
                    ),
                )
            })?;
            ensure_not_cancelled(context)?;
            if request.waits.is_empty() && request.interactions.is_empty() {
                cancellable_sleep(Duration::from_millis(1_500), context).await?;
            }
            for (wait_index, wait) in request.waits.iter().enumerate() {
                apply_wait(&page, &request.url, wait, wait_index, context).await?;
            }
            for (interaction_index, interaction) in request.interactions.iter().enumerate() {
                apply_interaction(&page, interaction, interaction_index, context).await?;
            }
            ensure_not_cancelled(context)?;
            let html = page.content().await.map_err(|error| {
                BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::ContentReadFailed,
                    format!(
                        "Managed browser runtime content read failed for {}: {error}",
                        request.url
                    ),
                )
            })?;
            ensure_not_cancelled(context)?;
            Ok(html)
        });
        tokio::pin!(page_operation);
        tokio::select! {
            biased;
            _ = context.cancelled() => Err(BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::Cancelled, "Managed browser runtime execution cancelled")),
            result = &mut page_operation => match result {
                Ok(result) => result,
                Err(_) => Err(BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::RenderTimeout,
                    format!("Managed browser runtime timed out rendering {url}"),
                )),
            },
            _ = context.browser_work_deadline_reached() => {
                if context.is_cancelled() {
                    Err(BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::Cancelled, "Managed browser runtime execution cancelled"))
                } else {
                    context.mark_deadline();
                    Err(BrowserRuntimeRenderError::new(BrowserRuntimeRenderErrorKind::RenderTimeout, format!("Managed browser runtime stopped page work before the teardown reserve while rendering {url}")))
                }
            }
        }
    };

    // Page work stops before the reserved teardown partition. Graceful close,
    // forced termination/reap, and handler completion/abort all finish before
    // filesystem finalization receives the last phase-deadline slice.
    let close_result = teardown_browser(&mut browser, &mut handler_task, context).await;

    match (page_result, close_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(html), Ok(())) => Ok(html),
    }
}

async fn teardown_browser(
    browser: &mut Browser,
    handler_task: &mut tokio::task::JoinHandle<()>,
    context: RuntimeExecutionContext<'_>,
) -> Result<(), BrowserRuntimeRenderError> {
    let Some(graceful_boundary) = context.browser_graceful_deadline() else {
        browser.close().await.map_err(|error| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
                format!("Managed browser runtime failed to close: {error}"),
            )
        })?;
        let _ = handler_task.await;
        return Ok(());
    };
    let force_boundary = context
        .browser_force_deadline()
        .expect("phase Browser teardown deadlines are complete");
    let handler_boundary = context
        .browser_handler_deadline()
        .expect("phase Browser teardown deadlines are complete");

    let graceful_deadline = capped_stage_deadline(
        graceful_boundary,
        Duration::from_millis(BROWSER_GRACEFUL_CLOSE_MS),
    );
    let graceful_result = tokio::select! {
        biased;
        result = browser.close() => result.map(|_| ()).map_err(|error| error.to_string()),
        _ = tokio::time::sleep_until(graceful_deadline) => Err("graceful close exceeded its 500 ms slice".to_string()),
    };

    let process_result = match browser.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => match browser.get_mut_child() {
            Some(child) => force_terminate_and_reap(child.as_mut_inner(), force_boundary).await,
            None => Err("managed Chromium process handle was unavailable".to_string()),
        },
        Err(error) => Err(error.to_string()),
    };

    let handler_completed = complete_or_abort_handler(handler_task, handler_boundary).await;

    match (graceful_result, process_result) {
        (_, Err(message)) => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
            format!("Managed browser runtime teardown failed: {message}"),
        )),
        (Err(message), Ok(())) => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
            format!("Managed browser runtime teardown required forced termination: {message}"),
        )),
        (Ok(()), Ok(())) if handler_completed => Ok(()),
        (Ok(()), Ok(())) => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
            "Managed browser runtime handler exceeded its 250 ms completion slice and was aborted",
        )),
    }
}

trait ForceTerminationChild {
    fn start_kill(&mut self) -> io::Result<()>;

    fn wait_for_reap(&mut self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>>;
}

impl ForceTerminationChild for tokio::process::Child {
    fn start_kill(&mut self) -> io::Result<()> {
        tokio::process::Child::start_kill(self)
    }

    fn wait_for_reap(&mut self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> {
        Box::pin(async move { self.wait().await.map(|_| ()) })
    }
}

async fn force_terminate_and_reap(
    child: &mut impl ForceTerminationChild,
    force_boundary: tokio::time::Instant,
) -> Result<(), String> {
    // start_kill is synchronous: once this returns successfully, dropping the
    // bounded reap future cannot be the only mechanism that initiates kill.
    child
        .start_kill()
        .map_err(|error| format!("failed to initiate forced termination: {error}"))?;

    let force_deadline = capped_stage_deadline(
        force_boundary,
        Duration::from_millis(BROWSER_FORCE_TERMINATE_REAP_MS),
    );
    tokio::select! {
        biased;
        result = child.wait_for_reap() => result.map_err(|error| format!("failed to reap forced Chromium process: {error}")),
        _ = tokio::time::sleep_until(force_deadline) => Err("forced termination was initiated but OS process reap exceeded its 1000 ms slice".to_string()),
    }
}

async fn complete_or_abort_handler(
    handler_task: &mut tokio::task::JoinHandle<()>,
    handler_boundary: tokio::time::Instant,
) -> bool {
    let handler_deadline = capped_stage_deadline(
        handler_boundary,
        Duration::from_millis(BROWSER_HANDLER_COMPLETION_MS),
    );
    let completed = tokio::select! {
        biased;
        _ = &mut *handler_task => true,
        _ = tokio::time::sleep_until(handler_deadline) => false,
    };
    if !completed {
        handler_task.abort();
        // Awaiting the aborted JoinHandle guarantees that no spawned handler
        // application task continues after the phase returns.
        let _ = (&mut *handler_task).await;
    }
    completed
}

async fn launch_browser(
    executable_path: &Path,
    session_dir: &Path,
) -> Result<(Browser, tokio::task::JoinHandle<()>), String> {
    let config = BrowserConfig::builder()
        .chrome_executable(executable_path)
        .user_data_dir(session_dir)
        .arg("--no-first-run")
        .arg("--disable-background-networking")
        .arg("--disable-default-apps")
        .arg("--disable-extensions")
        .arg("--disable-sync")
        .arg("--disable-component-update")
        .build()
        .map_err(|error| error.to_string())?;

    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|error| format!("Managed browser runtime failed to launch: {error}"))?;

    let handler_task = tokio::spawn(async move {
        while let Some(message) = handler.next().await {
            if message.is_err() {
                break;
            }
        }
    });

    Ok((browser, handler_task))
}

#[cfg(test)]
mod allowance_tests {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    use crate::profile_dsl::{
        documents::PhaseLimits,
        runtime::{allowance::InvocationAllowance, RuntimeCancellation, RuntimeExecutionContext},
    };

    use super::*;

    struct CancelOnCheck {
        checks: AtomicUsize,
        cancel_on: usize,
    }

    impl RuntimeCancellation for CancelOnCheck {
        fn is_cancelled(&self) -> bool {
            self.checks.fetch_add(1, Ordering::SeqCst) + 1 >= self.cancel_on
        }
    }

    struct PendingReapChild {
        kill_started: bool,
    }

    impl ForceTerminationChild for PendingReapChild {
        fn start_kill(&mut self) -> io::Result<()> {
            self.kill_started = true;
            Ok(())
        }

        fn wait_for_reap(&mut self) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send + '_>> {
            Box::pin(std::future::pending())
        }
    }

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn early_teardown_stage_deadlines_use_each_maximum_and_never_cross_the_boundary() {
        let started_at = tokio::time::Instant::now();
        let distant_boundary = started_at + Duration::from_secs(10);
        let near_boundary = started_at + Duration::from_millis(100);

        for maximum_ms in [
            BROWSER_GRACEFUL_CLOSE_MS,
            BROWSER_FORCE_TERMINATE_REAP_MS,
            BROWSER_HANDLER_COMPLETION_MS,
            BROWSER_SESSION_FINALIZATION_MS,
        ] {
            assert_eq!(
                capped_stage_deadline(distant_boundary, Duration::from_millis(maximum_ms)),
                started_at + Duration::from_millis(maximum_ms)
            );
            assert_eq!(
                capped_stage_deadline(near_boundary, Duration::from_millis(maximum_ms)),
                near_boundary
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn forced_teardown_initiates_kill_and_caps_early_reap_at_1000_ms() {
        let mut child = PendingReapChild {
            kill_started: false,
        };
        let started_at = tokio::time::Instant::now();
        let distant_boundary = started_at + Duration::from_secs(10);

        let result = force_terminate_and_reap(&mut child, distant_boundary).await;

        assert!(child.kill_started);
        assert_eq!(
            tokio::time::Instant::now() - started_at,
            Duration::from_millis(1_000)
        );
        assert_eq!(
            result.unwrap_err(),
            "forced termination was initiated but OS process reap exceeded its 1000 ms slice"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn early_handler_is_capped_at_250_ms_then_aborted_awaited_and_dropped() {
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = Arc::clone(&dropped);
        let mut handler_task = tokio::spawn(async move {
            let _drop_flag = DropFlag(task_dropped);
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;
        let started_at = tokio::time::Instant::now();
        let distant_boundary = started_at + Duration::from_secs(10);

        let completed = complete_or_abort_handler(&mut handler_task, distant_boundary).await;

        assert!(!completed);
        assert_eq!(
            tokio::time::Instant::now() - started_at,
            Duration::from_millis(250)
        );
        assert!(handler_task.is_finished());
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn early_filesystem_finalization_is_capped_at_250_ms() {
        let started_at = tokio::time::Instant::now();
        let distant_phase_deadline = started_at + Duration::from_secs(10);

        let result = finalize_session_with_deadline(
            std::future::pending::<Result<(), io::Error>>(),
            distant_phase_deadline,
        )
        .await;

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            tokio::time::Instant::now() - started_at,
            Duration::from_millis(250)
        );
    }

    #[test]
    fn actual_click_admission_has_exact_equality_and_one_over_behavior() {
        let limits = PhaseLimits {
            max_browser_actions: 1,
            ..PhaseLimits::BACKEND
        };
        let allowance = InvocationAllowance::new(limits, true, None);
        let caller = RuntimeExecutionContext::uncancellable();
        let context = caller.for_invocation(&allowance);
        let mut effects = 0;

        admit_browser_click(context, 0).expect("equality admits click");
        effects += 1;
        assert!(
            admit_browser_click(context, 0).is_err(),
            "one-over is denied"
        );

        assert_eq!(effects, 1);
        assert_eq!(
            allowance
                .report(crate::profile_dsl::runtime::PhaseCompletion::Accepted)
                .usage
                .browser_actions,
            1
        );
    }

    #[test]
    fn cancellation_observed_after_click_debit_prevents_the_effect_without_refund() {
        let limits = PhaseLimits {
            max_browser_actions: 1,
            ..PhaseLimits::BACKEND
        };
        let allowance = InvocationAllowance::new(limits, true, None);
        let cancellation = CancelOnCheck {
            checks: AtomicUsize::new(0),
            cancel_on: 2,
        };
        let caller = RuntimeExecutionContext::with_cancellation(&cancellation);
        let context = caller.for_invocation(&allowance);
        let mut effects = 0;

        if admit_browser_click(context, 0).is_ok() {
            effects += 1;
        }

        assert_eq!(effects, 0);
        assert_eq!(
            allowance
                .report(crate::profile_dsl::runtime::PhaseCompletion::Cancelled {
                    reason: crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                })
                .usage
                .browser_actions,
            1
        );
    }
}
