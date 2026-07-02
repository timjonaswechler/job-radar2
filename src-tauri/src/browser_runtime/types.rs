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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserRuntimePageWait {
    pub selector: String,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserRuntimeRenderRequest {
    pub url: String,
    pub timeout_ms: u64,
    pub waits: Vec<BrowserRuntimeWait>,
    pub interactions: Vec<BrowserRuntimeInteraction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserRuntimeWait {
    Selector {
        selector: Option<String>,
        timeout_ms: u64,
    },
    NetworkIdle {
        selector: Option<String>,
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserRuntimeInteraction {
    ClickIfVisible {
        selector: String,
        max_count: u64,
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        selector: String,
        max_count: u64,
        wait_after_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserRuntimeRenderError {
    pub kind: BrowserRuntimeRenderErrorKind,
    pub message: String,
}

impl BrowserRuntimeRenderError {
    pub fn new(kind: BrowserRuntimeRenderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserRuntimeRenderErrorKind {
    RuntimeUnavailable,
    NavigationFailed,
    WaitTimeout { wait_index: Option<usize> },
    InteractionFailed { interaction_index: Option<usize> },
    RenderTimeout,
    ContentReadFailed,
}

pub trait BrowserRuntimeInstallProgressReporter: Send + Sync {
    fn emit(&self, progress: BrowserRuntimeInstallProgress);
}
