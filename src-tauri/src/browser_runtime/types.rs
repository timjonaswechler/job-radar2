use serde::{Deserialize, Serialize};

pub const INSTALL_PROGRESS_EVENT: &str = "browser-runtime:install-progress";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserRuntimeState {
    Unsupported,
    NotInstalled,
    Installing,
    Installed,
    UpdateRequired,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeStatus {
    pub status: BrowserRuntimeState,
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    pub install_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserRuntimeSpec {
    pub platform: String,
    pub version: String,
    pub download_url: String,
    pub expected_archive_sha256: String,
    pub archive_format: BrowserRuntimeArchiveFormat,
    pub archive_root_dir: String,
    pub relative_executable_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserRuntimeArchiveFormat {
    Zip,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserRuntimeInstallPhase {
    Downloading,
    Verifying,
    Extracting,
    Finalizing,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeInstallProgress {
    pub install_id: String,
    pub phase: BrowserRuntimeInstallPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloaded_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeCheckResult {
    pub ok: bool,
    pub status: BrowserRuntimeStatus,
    pub message: String,
}

pub trait BrowserRuntimeInstallProgressReporter: Send + Sync {
    fn emit(&self, progress: BrowserRuntimeInstallProgress);
}
