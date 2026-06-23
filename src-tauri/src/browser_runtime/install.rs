use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use super::*;

struct BrowserRuntimeInstallWorkspace {
    install_id: String,
    temp_dir: PathBuf,
    archive_path: PathBuf,
    extracted_dir: PathBuf,
}

impl BrowserRuntimeInstallWorkspace {
    fn new(runtime_dir: &Path) -> Self {
        let install_id = Uuid::new_v4().to_string();
        let temp_dir = runtime_dir
            .join(".tmp")
            .join(format!("install-{install_id}"));
        let archive_path = temp_dir.join("browser-runtime.zip");
        let extracted_dir = temp_dir.join("extracted");

        Self {
            install_id,
            temp_dir,
            archive_path,
            extracted_dir,
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

    let workspace = BrowserRuntimeInstallWorkspace::new(runtime_dir);

    let result = install_runtime_inner(
        runtime_dir,
        spec,
        downloader,
        extractor,
        progress,
        &workspace,
    )
    .await;

    if let Err(error) = &result {
        emit_progress(
            progress,
            &workspace.install_id,
            BrowserRuntimeInstallPhase::Failed,
            None,
            None,
            Some(error.clone()),
        );
    }

    let _ = std::fs::remove_dir_all(&workspace.temp_dir);
    result
}

pub fn uninstall_runtime(
    runtime_dir: &Path,
    spec: Option<&BrowserRuntimeSpec>,
) -> Result<BrowserRuntimeStatus, String> {
    remove_dir_all_if_exists(runtime_dir)?;
    Ok(status_for_runtime_dir(runtime_dir, spec, false))
}

async fn install_runtime_inner<D, E, P>(
    runtime_dir: &Path,
    spec: &BrowserRuntimeSpec,
    downloader: &D,
    extractor: &E,
    progress: &P,
    workspace: &BrowserRuntimeInstallWorkspace,
) -> Result<BrowserRuntimeStatus, String>
where
    D: RuntimeDownloader,
    E: RuntimeArchiveExtractor,
    P: BrowserRuntimeInstallProgressReporter,
{
    let install_id = workspace.install_id.as_str();
    let temp_dir = workspace.temp_dir.as_path();
    let archive_path = workspace.archive_path.as_path();
    let extracted_dir = workspace.extracted_dir.as_path();

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

fn remove_dir_all_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn emit_progress(
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
