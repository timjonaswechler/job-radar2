use chromiumoxide::browser::{Browser, BrowserConfig};
use futures_util::StreamExt;
use std::path::Path;
use uuid::Uuid;

pub async fn smoke_test(executable_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let session_dir = runtime_dir
        .join(".tmp")
        .join(format!("session-{}", Uuid::new_v4()));
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

async fn smoke_test_with_session(executable_path: &Path, session_dir: &Path) -> Result<(), String> {
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

    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|error| format!("Managed browser runtime failed to launch: {error}"))?;

    let handler_task = tokio::spawn(async move {
        while let Some(message) = handler.next().await {
            if message.is_err() {
                break;
            }
        }
    });

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
