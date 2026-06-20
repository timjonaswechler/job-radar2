use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};

use super::types::{BrowserRuntimeArchiveFormat, BrowserRuntimeSpec};

pub trait RuntimeArchiveExtractor: Send + Sync {
    fn extract(
        &self,
        archive_path: &Path,
        destination_dir: &Path,
        spec: &BrowserRuntimeSpec,
    ) -> Result<(), String>;
}

pub struct ZipRuntimeArchiveExtractor;

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

#[cfg(unix)]
fn safe_zip_symlink_target(target: &Path) -> bool {
    !target.as_os_str().is_empty()
        && !target.is_absolute()
        && target
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}
