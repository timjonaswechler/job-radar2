use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    io::Read,
    path::{Component, Path, PathBuf},
    pin::Pin,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

mod browser_control;

pub const INSTALL_PROGRESS_EVENT: &str = "browser-runtime:install-progress";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const RUNTIME_KIND: &str = "chrome-for-testing";

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

pub trait RuntimeDownloader: Send + Sync {
    fn download<'a>(
        &'a self,
        spec: &'a BrowserRuntimeSpec,
        destination: &'a Path,
        install_id: &'a str,
        progress: &'a dyn BrowserRuntimeInstallProgressReporter,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

pub trait RuntimeArchiveExtractor: Send + Sync {
    fn extract(
        &self,
        archive_path: &Path,
        destination_dir: &Path,
        spec: &BrowserRuntimeSpec,
    ) -> Result<(), String>;
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

pub struct ZipRuntimeArchiveExtractor;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserRuntimeManifest {
    schema_version: u32,
    runtime_kind: String,
    platform: String,
    version: String,
    download_url: String,
    archive_sha256: String,
    install_dir: String,
    executable_path: String,
    installed_at: String,
}

impl BrowserRuntimeSpec {
    #[cfg(test)]
    fn for_test(platform: &str, version: &str, expected_archive_sha256: &str) -> Self {
        Self {
            platform: platform.to_string(),
            version: version.to_string(),
            download_url: format!("https://example.test/{platform}/{version}.zip"),
            expected_archive_sha256: expected_archive_sha256.to_string(),
            archive_format: BrowserRuntimeArchiveFormat::Zip,
            archive_root_dir: format!("chrome-{platform}"),
            relative_executable_path: "chrome".to_string(),
        }
    }
}

pub fn current_runtime_spec() -> Option<BrowserRuntimeSpec> {
    match current_platform().as_str() {
        "mac-arm64" => Some(BrowserRuntimeSpec {
            platform: "mac-arm64".to_string(),
            version: "149.0.7827.55".to_string(),
            download_url:
                "https://storage.googleapis.com/chrome-for-testing-public/149.0.7827.55/mac-arm64/chrome-mac-arm64.zip"
                    .to_string(),
            expected_archive_sha256:
                "311211b54c429245e2cec0314ee1e314085e9c00350215b95e1a879350786630"
                    .to_string(),
            archive_format: BrowserRuntimeArchiveFormat::Zip,
            archive_root_dir: "chrome-mac-arm64".to_string(),
            relative_executable_path:
                "Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
                    .to_string(),
        }),
        _ => None,
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

impl RuntimeArchiveExtractor for ZipRuntimeArchiveExtractor {
    fn extract(
        &self,
        archive_path: &Path,
        destination_dir: &Path,
        spec: &BrowserRuntimeSpec,
    ) -> Result<(), String> {
        match spec.archive_format {
            BrowserRuntimeArchiveFormat::Zip => extract_zip_archive(archive_path, destination_dir),
        }
    }
}

pub async fn install_runtime<D, E, P>(
    runtime_dir: &Path,
    spec: &BrowserRuntimeSpec,
    downloader: &D,
    extractor: &E,
    progress: &P,
) -> Result<BrowserRuntimeStatus, String>
where
    D: RuntimeDownloader,
    E: RuntimeArchiveExtractor,
    P: BrowserRuntimeInstallProgressReporter,
{
    match status_for_runtime_dir(runtime_dir, Some(spec), false).status {
        BrowserRuntimeState::Installed => {
            return Ok(status_for_runtime_dir(runtime_dir, Some(spec), false));
        }
        BrowserRuntimeState::Invalid => {
            remove_dir_all_if_exists(runtime_dir)?;
        }
        BrowserRuntimeState::NotInstalled | BrowserRuntimeState::UpdateRequired => {}
        BrowserRuntimeState::Unsupported | BrowserRuntimeState::Installing => {}
    }

    let install_id = Uuid::new_v4().to_string();
    let temp_dir = runtime_dir
        .join(".tmp")
        .join(format!("install-{install_id}"));
    let archive_path = temp_dir.join("browser-runtime.zip");
    let extracted_dir = temp_dir.join("extracted");

    let result = install_runtime_inner(
        runtime_dir,
        spec,
        downloader,
        extractor,
        progress,
        &install_id,
        &temp_dir,
        &archive_path,
        &extracted_dir,
    )
    .await;

    if let Err(error) = &result {
        emit_progress(
            progress,
            &install_id,
            BrowserRuntimeInstallPhase::Failed,
            None,
            None,
            Some(error.clone()),
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

pub fn uninstall_runtime(
    runtime_dir: &Path,
    spec: Option<&BrowserRuntimeSpec>,
) -> Result<BrowserRuntimeStatus, String> {
    remove_dir_all_if_exists(runtime_dir)?;
    Ok(status_for_runtime_dir(runtime_dir, spec, false))
}

pub async fn check_runtime(
    runtime_dir: &Path,
    spec: Option<&BrowserRuntimeSpec>,
    installing: bool,
) -> BrowserRuntimeCheckResult {
    let status = status_for_runtime_dir(runtime_dir, spec, installing);
    if status.status != BrowserRuntimeState::Installed {
        return BrowserRuntimeCheckResult {
            ok: false,
            message: format!(
                "Managed browser runtime is not installed and ready: {:?}",
                status.status
            ),
            status,
        };
    }

    let executable_path = match status.executable_path.as_deref() {
        Some(path) => PathBuf::from(path),
        None => {
            return BrowserRuntimeCheckResult {
                ok: false,
                message: "Managed browser runtime status has no executable path".to_string(),
                status,
            }
        }
    };

    match browser_control::smoke_test(&executable_path, runtime_dir).await {
        Ok(()) => BrowserRuntimeCheckResult {
            ok: true,
            message: "Managed browser runtime smoke test passed".to_string(),
            status,
        },
        Err(error) => BrowserRuntimeCheckResult {
            ok: false,
            message: error,
            status,
        },
    }
}

async fn install_runtime_inner<D, E, P>(
    runtime_dir: &Path,
    spec: &BrowserRuntimeSpec,
    downloader: &D,
    extractor: &E,
    progress: &P,
    install_id: &str,
    temp_dir: &Path,
    archive_path: &Path,
    extracted_dir: &Path,
) -> Result<BrowserRuntimeStatus, String>
where
    D: RuntimeDownloader,
    E: RuntimeArchiveExtractor,
    P: BrowserRuntimeInstallProgressReporter,
{
    std::fs::create_dir_all(temp_dir).map_err(|error| error.to_string())?;

    emit_progress(
        progress,
        install_id,
        BrowserRuntimeInstallPhase::Downloading,
        None,
        None,
        Some("Downloading managed browser runtime".to_string()),
    );
    downloader
        .download(spec, archive_path, install_id, progress)
        .await?;

    emit_progress(
        progress,
        install_id,
        BrowserRuntimeInstallPhase::Verifying,
        None,
        None,
        Some("Verifying managed browser runtime archive".to_string()),
    );
    let actual_hash = sha256_file_hex(archive_path)?;
    if actual_hash != spec.expected_archive_sha256 {
        return Err(format!(
            "browser runtime archive hash mismatch: expected {}, got {}",
            spec.expected_archive_sha256, actual_hash
        ));
    }

    emit_progress(
        progress,
        install_id,
        BrowserRuntimeInstallPhase::Extracting,
        None,
        None,
        Some("Extracting managed browser runtime".to_string()),
    );
    std::fs::create_dir_all(extracted_dir).map_err(|error| error.to_string())?;
    extractor.extract(archive_path, extracted_dir, spec)?;

    emit_progress(
        progress,
        install_id,
        BrowserRuntimeInstallPhase::Finalizing,
        None,
        None,
        Some("Finalizing managed browser runtime".to_string()),
    );
    let extracted_root = extracted_dir.join(&spec.archive_root_dir);
    if !extracted_root.is_dir() {
        return Err(format!(
            "browser runtime archive root is missing: {}",
            spec.archive_root_dir
        ));
    }

    let final_install_dir = runtime_dir.join(relative_install_dir(spec));
    if let Some(parent) = final_install_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    remove_dir_all_if_exists(&final_install_dir)?;
    std::fs::rename(&extracted_root, &final_install_dir).map_err(|error| error.to_string())?;

    let executable_path = final_install_dir.join(&spec.relative_executable_path);
    if !executable_path.is_file() {
        return Err(format!(
            "browser runtime executable is missing after install: {}",
            executable_path.display()
        ));
    }

    let manifest = BrowserRuntimeManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        runtime_kind: RUNTIME_KIND.to_string(),
        platform: spec.platform.clone(),
        version: spec.version.clone(),
        download_url: spec.download_url.clone(),
        archive_sha256: spec.expected_archive_sha256.clone(),
        install_dir: relative_install_dir_string(spec),
        executable_path: spec.relative_executable_path.clone(),
        installed_at: installed_at_timestamp(),
    };
    write_manifest(runtime_dir, &manifest)?;
    cleanup_old_versions(runtime_dir, spec)?;
    let _ = std::fs::remove_file(archive_path);

    emit_progress(
        progress,
        install_id,
        BrowserRuntimeInstallPhase::Completed,
        None,
        None,
        Some("Managed browser runtime installed".to_string()),
    );

    Ok(status_for_runtime_dir(runtime_dir, Some(spec), false))
}

pub fn status_for_runtime_dir(
    runtime_dir: &Path,
    spec: Option<&BrowserRuntimeSpec>,
    installing: bool,
) -> BrowserRuntimeStatus {
    let current_platform = current_platform();
    let platform = spec
        .map(|spec| spec.platform.as_str())
        .unwrap_or(current_platform.as_str());

    status_for_runtime_dir_with_platform(runtime_dir, platform, spec, installing)
}

pub fn status_for_runtime_dir_with_platform(
    runtime_dir: &Path,
    platform: &str,
    spec: Option<&BrowserRuntimeSpec>,
    installing: bool,
) -> BrowserRuntimeStatus {
    let Some(spec) = spec else {
        return status(
            BrowserRuntimeState::Unsupported,
            platform,
            None,
            None,
            runtime_dir,
            None,
            Some("platform is not supported by the managed browser runtime".to_string()),
        );
    };

    if installing {
        return status(
            BrowserRuntimeState::Installing,
            &spec.platform,
            Some(spec.version.clone()),
            None,
            runtime_dir,
            None,
            None,
        );
    }

    let _ = cleanup_temporary_runtime_dirs(runtime_dir);

    let manifest_path = runtime_dir.join(MANIFEST_FILE_NAME);
    if !manifest_path.exists() {
        if runtime_dir_has_non_temporary_entries(runtime_dir) {
            return invalid_status(
                runtime_dir,
                &spec.platform,
                Some(spec.version.clone()),
                None,
                "browser runtime directory contains files but no manifest".to_string(),
            );
        }

        return status(
            BrowserRuntimeState::NotInstalled,
            &spec.platform,
            Some(spec.version.clone()),
            None,
            runtime_dir,
            None,
            None,
        );
    }

    let manifest = match read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return invalid_status(
                runtime_dir,
                &spec.platform,
                Some(spec.version.clone()),
                None,
                error,
            )
        }
    };

    let installed_version = Some(manifest.version.clone());

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return invalid_status(
            runtime_dir,
            &spec.platform,
            Some(spec.version.clone()),
            installed_version,
            format!(
                "unsupported browser runtime manifest schemaVersion {}",
                manifest.schema_version
            ),
        );
    }

    if manifest.runtime_kind != RUNTIME_KIND {
        return invalid_status(
            runtime_dir,
            &spec.platform,
            Some(spec.version.clone()),
            installed_version,
            format!("unexpected browser runtime kind {}", manifest.runtime_kind),
        );
    }

    let install_dir = match safe_relative_path(&manifest.install_dir) {
        Ok(path) => path,
        Err(error) => {
            return invalid_status(
                runtime_dir,
                &spec.platform,
                Some(spec.version.clone()),
                installed_version,
                error,
            )
        }
    };
    let executable_path = match safe_relative_path(&manifest.executable_path) {
        Ok(path) => path,
        Err(error) => {
            return invalid_status(
                runtime_dir,
                &spec.platform,
                Some(spec.version.clone()),
                installed_version,
                error,
            )
        }
    };
    let executable_absolute_path = runtime_dir.join(install_dir).join(executable_path);
    let executable_absolute_path_string = executable_absolute_path.to_string_lossy().to_string();

    if !executable_absolute_path.is_file() {
        return status(
            BrowserRuntimeState::Invalid,
            &spec.platform,
            Some(spec.version.clone()),
            installed_version,
            runtime_dir,
            None,
            Some(format!(
                "browser runtime executable is missing: {}",
                executable_absolute_path.display()
            )),
        );
    }

    if manifest.platform != spec.platform
        || manifest.version != spec.version
        || manifest.download_url != spec.download_url
        || manifest.archive_sha256 != spec.expected_archive_sha256
        || manifest.executable_path != spec.relative_executable_path
    {
        return status(
            BrowserRuntimeState::UpdateRequired,
            &spec.platform,
            Some(spec.version.clone()),
            installed_version,
            runtime_dir,
            Some(executable_absolute_path_string),
            Some("installed browser runtime does not match the pinned runtime spec".to_string()),
        );
    }

    status(
        BrowserRuntimeState::Installed,
        &spec.platform,
        Some(spec.version.clone()),
        installed_version,
        runtime_dir,
        Some(executable_absolute_path_string),
        None,
    )
}

fn write_manifest(runtime_dir: &Path, manifest: &BrowserRuntimeManifest) -> Result<(), String> {
    std::fs::create_dir_all(runtime_dir).map_err(|error| error.to_string())?;
    let manifest_path = runtime_dir.join(MANIFEST_FILE_NAME);
    let temp_manifest_path = runtime_dir.join(format!("{MANIFEST_FILE_NAME}.tmp"));
    let manifest_json =
        serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?;
    std::fs::write(&temp_manifest_path, manifest_json).map_err(|error| error.to_string())?;
    std::fs::rename(temp_manifest_path, manifest_path).map_err(|error| error.to_string())?;
    Ok(())
}

fn extract_zip_archive(archive_path: &Path, destination_dir: &Path) -> Result<(), String> {
    let archive_file = std::fs::File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(archive_file).map_err(|error| error.to_string())?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| format!("zip entry escapes destination: {}", entry.name()))?
            .to_owned();
        let output_path = destination_dir.join(enclosed_name);

        if entry.is_dir() {
            std::fs::create_dir_all(&output_path).map_err(|error| error.to_string())?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                use std::os::unix::{ffi::OsStringExt, fs::symlink};

                let mut target_bytes = Vec::new();
                entry
                    .read_to_end(&mut target_bytes)
                    .map_err(|error| error.to_string())?;
                let target = PathBuf::from(std::ffi::OsString::from_vec(target_bytes));
                if !safe_zip_symlink_target(&target) {
                    return Err(format!(
                        "zip symlink target escapes archive root: {}",
                        target.display()
                    ));
                }
                symlink(target, &output_path).map_err(|error| error.to_string())?;
                continue;
            }
        }

        let mut output_file =
            std::fs::File::create(&output_path).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut output_file).map_err(|error| error.to_string())?;

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(mode))
                .map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn sha256_file_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn relative_install_dir(spec: &BrowserRuntimeSpec) -> PathBuf {
    PathBuf::from(&spec.platform).join(&spec.version)
}

fn relative_install_dir_string(spec: &BrowserRuntimeSpec) -> String {
    format!("{}/{}", spec.platform, spec.version)
}

fn cleanup_old_versions(runtime_dir: &Path, spec: &BrowserRuntimeSpec) -> Result<(), String> {
    let platform_dir = runtime_dir.join(&spec.platform);
    if !platform_dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(platform_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() && entry.file_name() != std::ffi::OsStr::new(&spec.version) {
            std::fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn cleanup_temporary_runtime_dirs(runtime_dir: &Path) -> Result<(), String> {
    let temp_dir = runtime_dir.join(".tmp");
    if !temp_dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(&temp_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("install-") || file_name.starts_with("session-") {
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
            } else {
                std::fs::remove_file(path).map_err(|error| error.to_string())?;
            }
        }
    }

    if std::fs::read_dir(&temp_dir)
        .map_err(|error| error.to_string())?
        .next()
        .is_none()
    {
        let _ = std::fs::remove_dir(&temp_dir);
    }

    Ok(())
}

fn runtime_dir_has_non_temporary_entries(runtime_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(runtime_dir) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        entry.file_name() != std::ffi::OsStr::new(".tmp")
            && entry.file_name() != std::ffi::OsStr::new(MANIFEST_FILE_NAME)
    })
}

fn remove_dir_all_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn installed_at_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

fn emit_progress(
    progress: &dyn BrowserRuntimeInstallProgressReporter,
    install_id: &str,
    phase: BrowserRuntimeInstallPhase,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    message: Option<String>,
) {
    progress.emit(BrowserRuntimeInstallProgress {
        install_id: install_id.to_string(),
        phase,
        downloaded_bytes,
        total_bytes,
        message,
    });
}

fn read_manifest(path: &Path) -> Result<BrowserRuntimeManifest, String> {
    let manifest_json = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&manifest_json).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn safe_zip_symlink_target(target: &Path) -> bool {
    !target.as_os_str().is_empty()
        && !target.is_absolute()
        && target
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err(format!(
            "browser runtime manifest path must be relative: {value}"
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "browser runtime manifest path escapes runtime dir: {value}"
                ));
            }
        }
    }

    Ok(path.to_path_buf())
}

fn invalid_status(
    runtime_dir: &Path,
    platform: &str,
    required_version: Option<String>,
    installed_version: Option<String>,
    error: String,
) -> BrowserRuntimeStatus {
    status(
        BrowserRuntimeState::Invalid,
        platform,
        required_version,
        installed_version,
        runtime_dir,
        None,
        Some(error),
    )
}

fn status(
    runtime_state: BrowserRuntimeState,
    platform: &str,
    required_version: Option<String>,
    installed_version: Option<String>,
    runtime_dir: &Path,
    executable_path: Option<String>,
    error: Option<String>,
) -> BrowserRuntimeStatus {
    BrowserRuntimeStatus {
        status: runtime_state,
        platform: platform.to_string(),
        required_version,
        installed_version,
        install_dir: runtime_dir.to_string_lossy().to_string(),
        executable_path,
        error,
    }
}

pub fn current_platform() -> String {
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };

    match os {
        "macos" => format!("mac-{arch}"),
        other => format!("{other}-{arch}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn status_without_manifest_or_files_returns_not_installed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "abc123");

        let status = status_for_runtime_dir(&runtime_dir, Some(&spec), false);

        assert_eq!(status.status, BrowserRuntimeState::NotInstalled);
        assert_eq!(status.platform, "mac-arm64");
        assert_eq!(status.required_version.as_deref(), Some("1.0.0"));
        assert_eq!(status.install_dir, runtime_dir.to_string_lossy());
        assert_eq!(status.executable_path, None);
    }

    #[test]
    fn unsupported_platform_returns_unsupported_without_required_version() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");

        let status =
            status_for_runtime_dir_with_platform(&runtime_dir, "linux-riscv64", None, false);

        assert_eq!(status.status, BrowserRuntimeState::Unsupported);
        assert_eq!(status.platform, "linux-riscv64");
        assert_eq!(status.required_version, None);
        assert_eq!(status.executable_path, None);
    }

    #[test]
    fn valid_matching_manifest_and_executable_returns_installed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "abc123");
        write_manifest_json(
            &runtime_dir,
            serde_json::json!({
                "schemaVersion": 1,
                "runtimeKind": "chrome-for-testing",
                "platform": "mac-arm64",
                "version": "1.0.0",
                "downloadUrl": spec.download_url,
                "archiveSha256": "abc123",
                "installDir": "mac-arm64/1.0.0",
                "executablePath": "chrome",
                "installedAt": "2026-06-09T00:00:00Z"
            }),
        );
        let executable_path = runtime_dir.join("mac-arm64/1.0.0/chrome");
        std::fs::create_dir_all(executable_path.parent().unwrap()).unwrap();
        std::fs::write(&executable_path, "fake chrome").unwrap();

        let status = status_for_runtime_dir(&runtime_dir, Some(&spec), false);

        assert_eq!(status.status, BrowserRuntimeState::Installed);
        assert_eq!(status.installed_version.as_deref(), Some("1.0.0"));
        assert_eq!(
            status.executable_path,
            Some(executable_path.to_string_lossy().to_string())
        );
        assert_eq!(status.error, None);
    }

    #[test]
    fn manifest_version_mismatch_returns_update_required() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "2.0.0", "abc123");
        write_manifest_json(
            &runtime_dir,
            serde_json::json!({
                "schemaVersion": 1,
                "runtimeKind": "chrome-for-testing",
                "platform": "mac-arm64",
                "version": "1.0.0",
                "downloadUrl": spec.download_url,
                "archiveSha256": "abc123",
                "installDir": "mac-arm64/1.0.0",
                "executablePath": "chrome",
                "installedAt": "2026-06-09T00:00:00Z"
            }),
        );
        let executable_path = runtime_dir.join("mac-arm64/1.0.0/chrome");
        std::fs::create_dir_all(executable_path.parent().unwrap()).unwrap();
        std::fs::write(executable_path, "fake chrome").unwrap();

        let status = status_for_runtime_dir(&runtime_dir, Some(&spec), false);

        assert_eq!(status.status, BrowserRuntimeState::UpdateRequired);
        assert_eq!(status.installed_version.as_deref(), Some("1.0.0"));
        assert_eq!(status.required_version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn missing_manifest_executable_returns_invalid() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "abc123");
        write_manifest_json(
            &runtime_dir,
            serde_json::json!({
                "schemaVersion": 1,
                "runtimeKind": "chrome-for-testing",
                "platform": "mac-arm64",
                "version": "1.0.0",
                "downloadUrl": spec.download_url,
                "archiveSha256": "abc123",
                "installDir": "mac-arm64/1.0.0",
                "executablePath": "chrome",
                "installedAt": "2026-06-09T00:00:00Z"
            }),
        );

        let status = status_for_runtime_dir(&runtime_dir, Some(&spec), false);

        assert_eq!(status.status, BrowserRuntimeState::Invalid);
        assert!(status.error.unwrap().contains("executable is missing"));
    }

    #[cfg(unix)]
    #[test]
    fn zip_extractor_preserves_unix_symlinks_for_mac_app_bundles() {
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("runtime.zip");
        let extract_dir = temp_dir.path().join("extract");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "unused");
        std::fs::write(
            &archive_path,
            zip_with_symlink(
                "chrome-mac-arm64/Framework.framework/Resources",
                "Versions/A/Resources",
            ),
        )
        .unwrap();

        ZipRuntimeArchiveExtractor
            .extract(&archive_path, &extract_dir, &spec)
            .unwrap();

        assert_eq!(
            std::fs::read_link(extract_dir.join("chrome-mac-arm64/Framework.framework/Resources"))
                .unwrap(),
            PathBuf::from("Versions/A/Resources")
        );
    }

    #[test]
    fn install_downloads_verifies_extracts_and_writes_final_manifest() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let runtime_dir = temp_dir.path().join("browser-runtime");
            let archive = zip_with_file("chrome-mac-arm64/chrome", b"fake chrome");
            let expected_hash = sha256_hex(&archive);
            let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", &expected_hash);
            let downloader = FakeDownloader { archive };
            let progress = RecordingProgress::default();

            let status = install_runtime(
                &runtime_dir,
                &spec,
                &downloader,
                &ZipRuntimeArchiveExtractor,
                &progress,
            )
            .await
            .unwrap();

            let executable_path = runtime_dir.join("mac-arm64/1.0.0/chrome");
            assert_eq!(status.status, BrowserRuntimeState::Installed);
            assert_eq!(
                status.executable_path,
                Some(executable_path.to_string_lossy().to_string())
            );
            assert_eq!(
                std::fs::read_to_string(executable_path).unwrap(),
                "fake chrome"
            );

            let manifest: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(runtime_dir.join("manifest.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["schemaVersion"], 1);
            assert_eq!(manifest["runtimeKind"], "chrome-for-testing");
            assert_eq!(manifest["platform"], "mac-arm64");
            assert_eq!(manifest["version"], "1.0.0");
            assert_eq!(manifest["archiveSha256"], expected_hash);
            assert_eq!(manifest["installDir"], "mac-arm64/1.0.0");
            assert_eq!(manifest["executablePath"], "chrome");

            let phases = progress.phases();
            assert!(phases.contains(&BrowserRuntimeInstallPhase::Downloading));
            assert!(phases.contains(&BrowserRuntimeInstallPhase::Verifying));
            assert!(phases.contains(&BrowserRuntimeInstallPhase::Extracting));
            assert!(phases.contains(&BrowserRuntimeInstallPhase::Finalizing));
            assert!(phases.contains(&BrowserRuntimeInstallPhase::Completed));
        });
    }

    #[test]
    fn hash_mismatch_fails_without_writing_manifest_and_emits_failed_progress() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let runtime_dir = temp_dir.path().join("browser-runtime");
            let archive = zip_with_file("chrome-mac-arm64/chrome", b"fake chrome");
            let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "not-the-archive-hash");
            let downloader = FakeDownloader { archive };
            let progress = RecordingProgress::default();

            let error = install_runtime(
                &runtime_dir,
                &spec,
                &downloader,
                &ZipRuntimeArchiveExtractor,
                &progress,
            )
            .await
            .unwrap_err();

            assert!(error.contains("hash mismatch"));
            assert!(!runtime_dir.join("manifest.json").exists());
            assert!(progress
                .phases()
                .contains(&BrowserRuntimeInstallPhase::Failed));
        });
    }

    #[test]
    fn uninstall_is_idempotent_and_removes_managed_runtime_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let runtime_dir = temp_dir.path().join("browser-runtime");
        let spec = BrowserRuntimeSpec::for_test("mac-arm64", "1.0.0", "abc123");
        std::fs::create_dir_all(runtime_dir.join("mac-arm64/1.0.0")).unwrap();
        std::fs::write(runtime_dir.join("manifest.json"), "{}").unwrap();

        let first_status = uninstall_runtime(&runtime_dir, Some(&spec)).unwrap();
        let second_status = uninstall_runtime(&runtime_dir, Some(&spec)).unwrap();

        assert!(!runtime_dir.exists());
        assert_eq!(first_status.status, BrowserRuntimeState::NotInstalled);
        assert_eq!(second_status.status, BrowserRuntimeState::NotInstalled);
    }

    fn write_manifest_json(runtime_dir: &Path, manifest: serde_json::Value) {
        std::fs::create_dir_all(runtime_dir).unwrap();
        std::fs::write(
            runtime_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    struct FakeDownloader {
        archive: Vec<u8>,
    }

    impl RuntimeDownloader for FakeDownloader {
        fn download<'a>(
            &'a self,
            _spec: &'a BrowserRuntimeSpec,
            destination: &'a Path,
            install_id: &'a str,
            progress: &'a dyn BrowserRuntimeInstallProgressReporter,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
            Box::pin(async move {
                tokio::fs::write(destination, &self.archive)
                    .await
                    .map_err(|error| error.to_string())?;
                emit_progress(
                    progress,
                    install_id,
                    BrowserRuntimeInstallPhase::Downloading,
                    Some(self.archive.len() as u64),
                    Some(self.archive.len() as u64),
                    None,
                );
                Ok(())
            })
        }
    }

    #[derive(Default)]
    struct RecordingProgress {
        events: std::sync::Mutex<Vec<BrowserRuntimeInstallProgress>>,
    }

    impl RecordingProgress {
        fn phases(&self) -> Vec<BrowserRuntimeInstallPhase> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .map(|event| event.phase.clone())
                .collect()
        }
    }

    impl BrowserRuntimeInstallProgressReporter for RecordingProgress {
        fn emit(&self, progress: BrowserRuntimeInstallProgress) {
            self.events.lock().unwrap().push(progress);
        }
    }

    fn zip_with_file(path: &str, contents: &[u8]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        zip.start_file(path, options).unwrap();
        zip.write_all(contents).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[cfg(unix)]
    fn zip_with_symlink(path: &str, target: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        zip.add_symlink(path, target, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap().into_inner()
    }
}
