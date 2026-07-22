use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

mod archive;
mod control;
mod download;
mod install;
mod managed;
mod manifest;
#[allow(dead_code)]
pub(crate) mod owned;
mod spec;
mod status;
#[cfg(test)]
mod tests;
mod types;

pub use archive::{RuntimeArchiveExtractor, ZipRuntimeArchiveExtractor};
pub(crate) use control::render_page_html_with_actions_and_context;
pub use download::{ReqwestRuntimeDownloader, RuntimeDownloader};
pub use install::{install_runtime, uninstall_runtime};
pub use managed::ManagedBrowserAcquisition;
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

#[derive(Default)]
struct BrowserSessionProtection {
    active_guards: usize,
    quarantined: bool,
}

static PROTECTED_BROWSER_SESSIONS: OnceLock<Mutex<HashMap<PathBuf, BrowserSessionProtection>>> =
    OnceLock::new();

pub(super) struct ActiveBrowserSession {
    path: PathBuf,
}

pub(super) fn begin_active_browser_session(path: &Path) -> ActiveBrowserSession {
    let mut sessions = protected_browser_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    sessions
        .entry(path.to_path_buf())
        .or_default()
        .active_guards += 1;
    ActiveBrowserSession {
        path: path.to_path_buf(),
    }
}

impl ActiveBrowserSession {
    /// Atomically changes active protection into persistent quarantine. A later
    /// guard release cannot erase quarantine established by process ownership.
    pub(super) fn quarantine(&mut self) {
        quarantine_browser_session(&self.path);
    }
}

pub(super) fn quarantine_browser_session(path: &Path) {
    protected_browser_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(path.to_path_buf())
        .or_default()
        .quarantined = true;
}

pub(super) fn is_protected_browser_session(path: &Path) -> bool {
    protected_browser_sessions()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(path)
}

fn protected_browser_sessions() -> &'static Mutex<HashMap<PathBuf, BrowserSessionProtection>> {
    PROTECTED_BROWSER_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

impl Drop for ActiveBrowserSession {
    fn drop(&mut self) {
        let mut sessions = protected_browser_sessions()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let should_remove = if let Some(protection) = sessions.get_mut(&self.path) {
            protection.active_guards = protection.active_guards.saturating_sub(1);
            protection.active_guards == 0 && !protection.quarantined
        } else {
            false
        };
        if should_remove {
            sessions.remove(&self.path);
        }
    }
}
use manifest::{
    installed_at_timestamp, read_manifest, write_manifest, BrowserRuntimeManifest,
    MANIFEST_FILE_NAME, MANIFEST_SCHEMA_VERSION, RUNTIME_KIND,
};
