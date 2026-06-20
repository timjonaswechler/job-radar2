use futures_util::StreamExt;
use std::{future::Future, path::Path, pin::Pin};
use tokio::io::AsyncWriteExt;

use super::{
    emit_progress, BrowserRuntimeInstallPhase, BrowserRuntimeInstallProgressReporter,
    BrowserRuntimeSpec,
};

pub trait RuntimeDownloader: Send + Sync {
    fn download<'a>(
        &'a self,
        spec: &'a BrowserRuntimeSpec,
        destination: &'a Path,
        install_id: &'a str,
        progress: &'a dyn BrowserRuntimeInstallProgressReporter,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

pub struct ReqwestRuntimeDownloader {
    client: reqwest::Client,
}

impl Default for ReqwestRuntimeDownloader {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl RuntimeDownloader for ReqwestRuntimeDownloader {
    fn download<'a>(
        &'a self,
        spec: &'a BrowserRuntimeSpec,
        destination: &'a Path,
        install_id: &'a str,
        progress: &'a dyn BrowserRuntimeInstallProgressReporter,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            let response = self
                .client
                .get(&spec.download_url)
                .send()
                .await
                .map_err(|error| error.to_string())?
                .error_for_status()
                .map_err(|error| error.to_string())?;
            let total_bytes = response.content_length();
            let mut downloaded_bytes = 0_u64;
            let mut stream = response.bytes_stream();
            let mut archive_file = tokio::fs::File::create(destination)
                .await
                .map_err(|error| error.to_string())?;

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                archive_file
                    .write_all(&chunk)
                    .await
                    .map_err(|error| error.to_string())?;
                downloaded_bytes += chunk.len() as u64;
                emit_progress(
                    progress,
                    install_id,
                    BrowserRuntimeInstallPhase::Downloading,
                    Some(downloaded_bytes),
                    total_bytes,
                    None,
                );
            }

            archive_file
                .flush()
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    }
}
