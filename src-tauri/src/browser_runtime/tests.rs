use sha2::{Digest, Sha256};
use std::{
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
};

use super::*;

#[cfg(test)]
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

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

    let status = status_for_runtime_dir_with_platform(&runtime_dir, "linux-riscv64", None, false);

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
    let executable_path = test_executable_path(&runtime_dir);
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
    let executable_path = test_executable_path(&runtime_dir);
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

        let executable_path = test_executable_path(&runtime_dir);
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

#[test]
fn successful_smoke_result_survives_session_cleanup_failure() {
    let cleanup_error = std::io::Error::new(
        std::io::ErrorKind::Other,
        "Directory not empty (os error 66)",
    );

    let result = super::control::smoke_result_after_session_cleanup(Ok(()), Err(cleanup_error));

    assert!(result.is_ok());
}

#[test]
fn successful_render_result_survives_session_cleanup_failure() {
    let cleanup_error = std::io::Error::new(
        std::io::ErrorKind::Other,
        "Directory not empty (os error 66)",
    );

    let result = super::control::render_result_after_session_cleanup(
        Ok("<html>rendered</html>".to_string()),
        Err(cleanup_error),
    );

    assert_eq!(result.unwrap(), "<html>rendered</html>");
}

#[test]
fn render_failure_is_preserved_after_session_cleanup() {
    let render_error = BrowserRuntimeRenderError::new(
        BrowserRuntimeRenderErrorKind::NavigationFailed,
        "navigation failed",
    );

    let result =
        super::control::render_result_after_session_cleanup(Err(render_error.clone()), Ok(()));

    assert_eq!(result.unwrap_err(), render_error);
}

#[test]
fn status_cleans_stale_session_dirs_without_marking_runtime_invalid() {
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
    let executable_path = test_executable_path(&runtime_dir);
    std::fs::create_dir_all(executable_path.parent().unwrap()).unwrap();
    std::fs::write(&executable_path, "fake chrome").unwrap();
    let stale_session_dir = runtime_dir.join(".tmp/session-stale");
    std::fs::create_dir_all(&stale_session_dir).unwrap();
    std::fs::write(stale_session_dir.join("lockfile"), "stale").unwrap();

    let status = status_for_runtime_dir(&runtime_dir, Some(&spec), false);

    assert_eq!(status.status, BrowserRuntimeState::Installed);
    assert!(!stale_session_dir.exists());
}

fn test_executable_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("mac-arm64").join("1.0.0").join("chrome")
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
