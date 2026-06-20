use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const MANIFEST_FILE_NAME: &str = "manifest.json";

pub(super) const MANIFEST_SCHEMA_VERSION: u32 = 1;

pub(super) const RUNTIME_KIND: &str = "chrome-for-testing";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BrowserRuntimeManifest {
    pub(super) schema_version: u32,
    pub(super) runtime_kind: String,
    pub(super) platform: String,
    pub(super) version: String,
    pub(super) download_url: String,
    pub(super) archive_sha256: String,
    pub(super) install_dir: String,
    pub(super) executable_path: String,
    pub(super) installed_at: String,
}

pub(super) fn write_manifest(
    runtime_dir: &Path,
    manifest: &BrowserRuntimeManifest,
) -> Result<(), String> {
    std::fs::create_dir_all(runtime_dir).map_err(|error| error.to_string())?;
    let manifest_path = runtime_dir.join(MANIFEST_FILE_NAME);
    let temp_manifest_path = runtime_dir.join(format!("{MANIFEST_FILE_NAME}.tmp"));
    let manifest_json =
        serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?;
    std::fs::write(&temp_manifest_path, manifest_json).map_err(|error| error.to_string())?;
    std::fs::rename(temp_manifest_path, manifest_path).map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn read_manifest(path: &Path) -> Result<BrowserRuntimeManifest, String> {
    let manifest_json = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&manifest_json).map_err(|error| error.to_string())
}

pub(super) fn installed_at_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}
