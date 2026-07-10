use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

mod archive;
mod control;
mod download;
mod install;
mod manifest;
mod spec;
mod status;
#[cfg(test)]
mod tests;
mod types;

pub use archive::{RuntimeArchiveExtractor, ZipRuntimeArchiveExtractor};
pub(crate) use control::render_page_html_with_actions_and_context;
pub use download::{ReqwestRuntimeDownloader, RuntimeDownloader};
pub use install::{install_runtime, uninstall_runtime};
pub use spec::{current_platform, current_runtime_spec};
#[cfg(test)]
pub use status::status_for_runtime_dir_with_platform;
pub use status::{check_runtime, status_for_runtime_dir};
#[allow(unused_imports)]
pub use types::BrowserRuntimeArchiveFormat;
pub use types::{
    BrowserRuntimeCheckResult, BrowserRuntimeInstallPhase, BrowserRuntimeInstallProgress,
    BrowserRuntimeInstallProgressReporter, BrowserRuntimeInteraction, BrowserRuntimeRenderError,
    BrowserRuntimeRenderErrorKind, BrowserRuntimeRenderRequest, BrowserRuntimeSpec,
    BrowserRuntimeState, BrowserRuntimeStatus, BrowserRuntimeWait, INSTALL_PROGRESS_EVENT,
};

use install::emit_progress;

static ACTIVE_BROWSER_SESSIONS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

pub(super) struct ActiveBrowserSession {
    path: PathBuf,
}

pub(super) fn begin_active_browser_session(path: &Path) -> ActiveBrowserSession {
    active_browser_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf());
    ActiveBrowserSession {
        path: path.to_path_buf(),
    }
}

pub(super) fn is_active_browser_session(path: &Path) -> bool {
    active_browser_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(path)
}

fn active_browser_sessions() -> &'static Mutex<HashSet<PathBuf>> {
    ACTIVE_BROWSER_SESSIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

impl Drop for ActiveBrowserSession {
    fn drop(&mut self) {
        active_browser_sessions()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.path);
    }
}
use manifest::{
    installed_at_timestamp, read_manifest, write_manifest, BrowserRuntimeManifest,
    MANIFEST_FILE_NAME, MANIFEST_SCHEMA_VERSION, RUNTIME_KIND,
};
