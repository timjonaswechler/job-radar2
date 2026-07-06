use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    Page,
};
use futures_util::StreamExt;
use std::{io, path::Path, time::Duration};
use uuid::Uuid;

const SESSION_CLEANUP_ATTEMPTS: usize = 3;
const SESSION_CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(50);

use super::{
    BrowserRuntimeInteraction, BrowserRuntimeRenderError, BrowserRuntimeRenderErrorKind,
    BrowserRuntimeRenderRequest, BrowserRuntimeWait,
};

pub async fn smoke_test(executable_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let session_dir = runtime_session_dir(runtime_dir);
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|error| error.to_string())?;

    let result = smoke_test_with_session(executable_path, &session_dir).await;
    let cleanup_result = cleanup_session_dir_best_effort(&session_dir).await;

    smoke_result_after_session_cleanup(result, cleanup_result)
}

pub async fn render_page_html_with_actions(
    executable_path: &Path,
    runtime_dir: &Path,
    request: BrowserRuntimeRenderRequest,
) -> Result<String, BrowserRuntimeRenderError> {
    let session_dir = runtime_session_dir(runtime_dir);
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|error| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
                error.to_string(),
            )
        })?;

    let result = render_page_html_with_session(executable_path, &session_dir, request).await;
    let cleanup_result = cleanup_session_dir_best_effort(&session_dir).await;

    render_result_after_session_cleanup(result, cleanup_result)
}

fn runtime_session_dir(runtime_dir: &Path) -> std::path::PathBuf {
    runtime_dir
        .join(".tmp")
        .join(format!("session-{}", Uuid::new_v4()))
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

async fn apply_wait(
    page: &Page,
    url: &str,
    wait: &BrowserRuntimeWait,
    wait_index: usize,
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
            wait_for_selector(page, url, selector, *timeout_ms, wait_index).await
        }
        BrowserRuntimeWait::NetworkIdle {
            selector,
            timeout_ms,
        } => {
            if let Some(selector) = selector {
                wait_for_selector(page, url, selector, *timeout_ms, wait_index).await?;
            }
            tokio::time::sleep(Duration::from_millis(*timeout_ms)).await;
            Ok(())
        }
    }
}

async fn wait_for_selector(
    page: &Page,
    url: &str,
    selector: &str,
    timeout_ms: u64,
    wait_index: usize,
) -> Result<(), BrowserRuntimeRenderError> {
    let timeout = Duration::from_millis(timeout_ms);
    let started_at = tokio::time::Instant::now();

    loop {
        let error = match page.find_element(selector.to_string()).await {
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

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn apply_interaction(
    page: &Page,
    interaction: &BrowserRuntimeInteraction,
    interaction_index: usize,
) -> Result<(), BrowserRuntimeRenderError> {
    match interaction {
        BrowserRuntimeInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        } => {
            for _ in 0..*max_count {
                let Ok(element) = page.find_element(selector.clone()).await else {
                    return Ok(());
                };
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
                sleep_after_interaction(*wait_after_ms).await;
            }
            Ok(())
        }
        BrowserRuntimeInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => {
            for _ in 0..*max_count {
                let Ok(element) = page.find_element(selector.clone()).await else {
                    return Ok(());
                };
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
                sleep_after_interaction(*wait_after_ms).await;
            }

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

async fn sleep_after_interaction(wait_after_ms: Option<u64>) {
    if let Some(wait_after_ms) = wait_after_ms {
        tokio::time::sleep(Duration::from_millis(wait_after_ms)).await;
    }
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
) -> Result<String, BrowserRuntimeRenderError> {
    let (mut browser, handler_task) =
        launch_browser(executable_path, session_dir)
            .await
            .map_err(|error| {
                BrowserRuntimeRenderError::new(
                    BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
                    error,
                )
            })?;

    let url = request.url.clone();
    let timeout = Duration::from_millis(request.timeout_ms);
    let page_result = match tokio::time::timeout(timeout, async {
        let page = browser.new_page("about:blank").await.map_err(|error| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
                format!("Managed browser runtime page failed: {error}"),
            )
        })?;
        page.goto(&request.url).await.map_err(|error| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::NavigationFailed,
                format!(
                    "Managed browser runtime navigation failed for {}: {error}",
                    request.url
                ),
            )
        })?;
        if request.waits.is_empty() && request.interactions.is_empty() {
            tokio::time::sleep(Duration::from_millis(1_500)).await;
        }
        for (wait_index, wait) in request.waits.iter().enumerate() {
            apply_wait(&page, &request.url, wait, wait_index).await?;
        }
        for (interaction_index, interaction) in request.interactions.iter().enumerate() {
            apply_interaction(&page, interaction, interaction_index).await?;
        }
        page.content().await.map_err(|error| {
            BrowserRuntimeRenderError::new(
                BrowserRuntimeRenderErrorKind::ContentReadFailed,
                format!(
                    "Managed browser runtime content read failed for {}: {error}",
                    request.url
                ),
            )
        })
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RenderTimeout,
            format!("Managed browser runtime timed out rendering {url}"),
        )),
    };

    let close_result = browser.close().await.map(|_| ()).map_err(|error| {
        BrowserRuntimeRenderError::new(
            BrowserRuntimeRenderErrorKind::RuntimeUnavailable,
            format!("Managed browser runtime failed to close: {error}"),
        )
    });
    let _ = handler_task.await;

    match (page_result, close_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(html), Ok(())) => Ok(html),
    }
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
