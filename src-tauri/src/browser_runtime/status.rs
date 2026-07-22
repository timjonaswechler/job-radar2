use std::path::{Path, PathBuf};

use super::*;

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

    match control::smoke_test(&executable_path, runtime_dir).await {
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
        || manifest.install_dir != install::relative_install_dir_string(spec)
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
            if file_name.starts_with("session-") && super::is_protected_browser_session(&path) {
                continue;
            }
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

fn safe_relative_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.starts_with('/') || value.starts_with('\\') {
        return Err(format!(
            "browser runtime manifest path must be relative: {value}"
        ));
    }

    let mut path = PathBuf::new();
    for component in value.split('/') {
        if component.is_empty() || component == "." {
            return Err(format!(
                "browser runtime manifest path must be relative: {value}"
            ));
        }
        if component == ".." || component.contains('\\') || component.contains(':') {
            return Err(format!(
                "browser runtime manifest path escapes runtime dir: {value}"
            ));
        }
        path.push(component);
    }

    Ok(path)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_path_converts_manifest_slashes_to_native_components() {
        assert_eq!(
            safe_relative_path("mac-arm64/1.0.0/chrome").unwrap(),
            PathBuf::from("mac-arm64").join("1.0.0").join("chrome")
        );
    }

    #[test]
    fn safe_relative_path_rejects_absolute_and_escaping_paths() {
        for value in [
            "",
            "/tmp/chrome",
            "C:/chrome",
            "C:chrome",
            "mac-arm64/../chrome",
            "mac-arm64\\chrome",
        ] {
            assert!(
                safe_relative_path(value).is_err(),
                "{value} should be rejected"
            );
        }
    }
}
