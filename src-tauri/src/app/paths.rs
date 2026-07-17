#[cfg(target_os = "macos")]
use std::ffi::CStr;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
const APP_IDENTIFIER: &str = "de.timjonaswechler.jobradar";
const DB_NAME: &str = "job_radar.db";

#[cfg(target_os = "macos")]
pub(crate) struct CurrentUserAppDataLocation {
    pub(crate) trusted_ancestor: PathBuf,
    pub(crate) root: PathBuf,
}

#[cfg(target_os = "macos")]
pub(crate) fn current_user_app_data_location() -> std::io::Result<CurrentUserAppDataLocation> {
    let buffer_size = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_size = if buffer_size > 0 {
        usize::try_from(buffer_size)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?
    } else {
        16_384
    };
    let mut buffer = vec![0_u8; buffer_size];
    let mut password_entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let status = unsafe {
        libc::getpwuid_r(
            libc::geteuid(),
            password_entry.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    let password_entry = unsafe { password_entry.assume_init() };
    if password_entry.pw_dir.is_null() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    let home = unsafe { CStr::from_ptr(password_entry.pw_dir) };
    let home = std::str::from_utf8(home.to_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }

    Ok(CurrentUserAppDataLocation {
        root: home
            .join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER),
        trusted_ancestor: home,
    })
}

pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub browser_runtime_dir: PathBuf,
    #[allow(dead_code)] // Used by the follow-up provider-registry integration.
    pub agents_data_dir: PathBuf,
    pub source_profiles_dir: PathBuf,
    pub sources_dir: PathBuf,
    pub source_live_checks_dir: PathBuf,
}

impl AppPaths {
    pub fn from_app(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_app_data_dir(app.path().app_data_dir()?)
    }

    pub fn from_app_data_dir(app_data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&app_data_dir)?;

        let database_path = app_data_dir.join(DB_NAME);
        let browser_runtime_dir = app_data_dir.join("browser-runtime");
        let agents_data_dir = app_data_dir.join("agents");
        let source_profiles_dir = app_data_dir.join("source-profiles");
        let sources_dir = app_data_dir.join("sources");
        let source_live_checks_dir = app_data_dir.join("source-live-checks");
        std::fs::create_dir_all(&source_profiles_dir)?;
        std::fs::create_dir_all(&sources_dir)?;
        std::fs::create_dir_all(&source_live_checks_dir)?;

        Ok(Self {
            app_data_dir,
            database_path,
            browser_runtime_dir,
            agents_data_dir,
            source_profiles_dir,
            sources_dir,
            source_live_checks_dir,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_paths_from_app_data_dir_computes_database_and_browser_runtime_paths() {
        let app_data_dir = PathBuf::from("/tmp/job-radar-test-data");

        let paths = AppPaths::from_app_data_dir(app_data_dir.clone()).unwrap();

        assert_eq!(paths.app_data_dir, app_data_dir);
        assert_eq!(
            paths.database_path,
            PathBuf::from("/tmp/job-radar-test-data/job_radar.db")
        );
        assert_eq!(
            paths.browser_runtime_dir,
            PathBuf::from("/tmp/job-radar-test-data/browser-runtime")
        );
        assert_eq!(
            paths.agents_data_dir,
            PathBuf::from("/tmp/job-radar-test-data/agents")
        );
        assert_eq!(
            paths.source_profiles_dir,
            PathBuf::from("/tmp/job-radar-test-data/source-profiles")
        );
        assert_eq!(
            paths.sources_dir,
            PathBuf::from("/tmp/job-radar-test-data/sources")
        );
        assert_eq!(
            paths.source_live_checks_dir,
            PathBuf::from("/tmp/job-radar-test-data/source-live-checks")
        );
    }
}
