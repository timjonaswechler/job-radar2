use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathError {
    Invalid,
    Unavailable,
}

pub(crate) fn path_is_inside_repository(path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

pub(crate) fn canonical_existing_prefix_is_inside_repository(path: &Path) -> bool {
    path.ancestors()
        .find(|ancestor| ancestor.exists())
        .and_then(|ancestor| fs::canonicalize(ancestor).ok())
        .is_some_and(|canonical| path_is_inside_repository(&canonical))
}

pub(crate) fn path_below_ancestor_contains_symlink(
    trusted_ancestor: &Path,
    path: &Path,
) -> Result<bool, PathError> {
    let relative = path
        .strip_prefix(trusted_ancestor)
        .map_err(|_| PathError::Invalid)?;
    let mut current = trusted_ancestor.to_owned();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if unsafe_path_metadata(&metadata) => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PathError::Unavailable),
        }
    }
    Ok(false)
}

pub(crate) fn unsafe_path_metadata(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}
