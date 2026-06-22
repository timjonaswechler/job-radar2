use sqlx::SqlitePool;
#[cfg(any(debug_assertions, test))]
use std::{fs, io, path::Path};
use tokio::sync::Mutex;

use crate::app::paths::AppPaths;

pub const RESET_DEV_DB_ENV: &str = "JOB_RADAR_RESET_DEV_DB";

pub struct AppState {
    pub db: SqlitePool,
    pub paths: AppPaths,
    pub browser_runtime_install_lock: Mutex<()>,
    pub running_search_runs: crate::search::request::RunningSearchRuns,
}

impl AppState {
    pub async fn new(paths: AppPaths) -> Result<Self, Box<dyn std::error::Error>> {
        maybe_reset_dev_database(&paths)?;
        let db = crate::db::connect_and_migrate(&paths.database_path).await?;

        Ok(Self {
            db,
            paths,
            browser_runtime_install_lock: Mutex::new(()),
            running_search_runs: crate::search::request::RunningSearchRuns::default(),
        })
    }
}

fn maybe_reset_dev_database(paths: &AppPaths) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var(RESET_DEV_DB_ENV).ok().as_deref() != Some("1") {
        return Ok(());
    }

    #[cfg(debug_assertions)]
    {
        reset_database_files(&paths.database_path)?;
        println!(
            "Reset development SQLite database: {}",
            paths.database_path.display()
        );
    }

    #[cfg(not(debug_assertions))]
    {
        eprintln!(
            "Ignoring {RESET_DEV_DB_ENV}=1 because database reset is only enabled in debug builds"
        );
    }

    Ok(())
}

#[cfg(any(debug_assertions, test))]
fn reset_database_files(database_path: &Path) -> io::Result<()> {
    for path in sqlite_database_file_family(database_path) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

#[cfg(any(debug_assertions, test))]
fn sqlite_database_file_family(database_path: &Path) -> Vec<std::path::PathBuf> {
    ["", "-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| {
            if suffix.is_empty() {
                database_path.to_path_buf()
            } else {
                database_path.with_file_name(format!(
                    "{}{}",
                    database_path
                        .file_name()
                        .and_then(|file_name| file_name.to_str())
                        .unwrap_or_default(),
                    suffix
                ))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_database_files_removes_sqlite_file_family() {
        let temp_dir = tempfile::tempdir().unwrap();
        let database_path = temp_dir.path().join("job_radar.db");
        for path in sqlite_database_file_family(&database_path) {
            fs::write(&path, "test").unwrap();
            assert!(path.exists());
        }

        reset_database_files(&database_path).unwrap();

        for path in sqlite_database_file_family(&database_path) {
            assert!(!path.exists());
        }
    }
}
