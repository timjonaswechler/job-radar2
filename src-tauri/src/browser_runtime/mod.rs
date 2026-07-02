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
pub use control::{render_page_html_with_actions, render_page_html_with_wait};
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
    BrowserRuntimeInstallProgressReporter, BrowserRuntimeInteraction, BrowserRuntimePageWait,
    BrowserRuntimeRenderError, BrowserRuntimeRenderErrorKind, BrowserRuntimeRenderRequest,
    BrowserRuntimeSpec, BrowserRuntimeState, BrowserRuntimeStatus, BrowserRuntimeWait,
    INSTALL_PROGRESS_EVENT,
};

use install::emit_progress;
use manifest::{
    installed_at_timestamp, read_manifest, write_manifest, BrowserRuntimeManifest,
    MANIFEST_FILE_NAME, MANIFEST_SCHEMA_VERSION, RUNTIME_KIND,
};
