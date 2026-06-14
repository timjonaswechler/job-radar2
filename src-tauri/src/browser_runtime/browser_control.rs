use chromiumoxide::{
    browser::{Browser, BrowserConfig},
    Page,
};
use futures_util::StreamExt;
use std::{path::Path, time::Duration};
use uuid::Uuid;

use super::BrowserRuntimePageWait;

pub async fn smoke_test(executable_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let session_dir = runtime_session_dir(runtime_dir);
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|error| error.to_string())?;

    let result = smoke_test_with_session(executable_path, &session_dir).await;
    let cleanup_result = tokio::fs::remove_dir_all(&session_dir).await;

    match (result, cleanup_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) if error.kind() != std::io::ErrorKind::NotFound => Err(format!(
            "Managed browser runtime smoke test passed, but session cleanup failed: {error}"
        )),
        (Ok(()), _) => Ok(()),
    }
}

pub async fn render_page_html(
    executable_path: &Path,
    runtime_dir: &Path,
    url: &str,
) -> Result<String, String> {
    render_page_html_with_wait(executable_path, runtime_dir, url, None).await
}

pub async fn render_page_html_with_wait(
    executable_path: &Path,
    runtime_dir: &Path,
    url: &str,
    wait_for: Option<&BrowserRuntimePageWait>,
) -> Result<String, String> {
    let session_dir = runtime_session_dir(runtime_dir);
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|error| error.to_string())?;

    let result = render_page_html_with_session(executable_path, &session_dir, url, wait_for).await;
    let cleanup_result = tokio::fs::remove_dir_all(&session_dir).await;

    match (result, cleanup_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) if error.kind() != std::io::ErrorKind::NotFound => Err(format!(
            "Managed browser runtime rendered page, but session cleanup failed: {error}"
        )),
        (Ok(html), _) => Ok(html),
    }
}

fn runtime_session_dir(runtime_dir: &Path) -> std::path::PathBuf {
    runtime_dir
        .join(".tmp")
        .join(format!("session-{}", Uuid::new_v4()))
}

fn render_timeout(wait_for: Option<&BrowserRuntimePageWait>) -> Duration {
    match wait_for {
        Some(wait_for) => {
            Duration::from_millis(wait_for.timeout_ms.saturating_add(5_000).max(30_000))
        }
        None => Duration::from_secs(30),
    }
}

async fn wait_for_selector(
    page: &Page,
    url: &str,
    wait_for: &BrowserRuntimePageWait,
) -> Result<(), String> {
    let timeout = Duration::from_millis(wait_for.timeout_ms);
    let started_at = tokio::time::Instant::now();

    loop {
        let error = match page.find_element(wait_for.selector.clone()).await {
            Ok(_) => return Ok(()),
            Err(error) => error.to_string(),
        };

        if started_at.elapsed() >= timeout {
            return Err(format!(
                "Managed browser runtime waitFor selector `{}` was not found for {url} within {} ms: {error}",
                wait_for.selector, wait_for.timeout_ms
            ));
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
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
    url: &str,
    wait_for: Option<&BrowserRuntimePageWait>,
) -> Result<String, String> {
    let (mut browser, handler_task) = launch_browser(executable_path, session_dir).await?;

    let timeout = render_timeout(wait_for);
    let page_result = match tokio::time::timeout(timeout, async {
        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|error| format!("Managed browser runtime page failed: {error}"))?;
        page.goto(url).await.map_err(|error| {
            format!("Managed browser runtime navigation failed for {url}: {error}")
        })?;
        if let Some(wait_for) = wait_for {
            wait_for_selector(&page, url, wait_for).await?;
        } else {
            tokio::time::sleep(Duration::from_millis(1_500)).await;
        }
        page.content().await.map_err(|error| {
            format!("Managed browser runtime content read failed for {url}: {error}")
        })
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(format!("Managed browser runtime timed out rendering {url}")),
    };

    let close_result = browser
        .close()
        .await
        .map(|_| ())
        .map_err(|error| format!("Managed browser runtime failed to close: {error}"));
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
