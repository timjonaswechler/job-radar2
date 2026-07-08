use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DB_NAME: &str = "job_radar.db";

pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub browser_runtime_dir: PathBuf,
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
