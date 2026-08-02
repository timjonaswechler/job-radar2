use sqlx::SqlitePool;
use std::sync::Arc;
#[cfg(test)]
use std::{fs, io, path::Path};
use tokio::sync::Mutex;

use crate::app::{paths::AppPaths, resources::AppResources};

pub struct AppState {
    pub db: SqlitePool,
    pub paths: AppPaths,
    pub resources: AppResources,
    pub browser_runtime_install_lock: Mutex<()>,
    pub search_requests: search_requests::Catalog,
    pub background_tasks: crate::background_tasks::BackgroundTaskScheduler,
    pub agent_configuration: Arc<crate::agent::configuration::AgentConfiguration>,
    pub agent_chats: Arc<crate::agent::chat_application::AgentChatApplication>,
    pub installed_sources: sources::installed::Store,
    pub source_detection: sources::detection::Operation,
    pub source_live_check: sources::live_check::Operation,
}

impl AppState {
    pub async fn new(paths: AppPaths) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_background_task_notifier(
            paths,
            Arc::new(crate::background_tasks::NoopBackgroundTaskNotifier),
        )
        .await
    }

    pub async fn new_with_background_task_notifier(
        paths: AppPaths,
        notifier: Arc<dyn crate::background_tasks::BackgroundTaskNotifier>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_resources_and_background_task_notifier(
            paths,
            AppResources::default(),
            notifier,
        )
        .await
    }

    pub async fn new_with_resources_and_background_task_notifier(
        paths: AppPaths,
        resources: AppResources,
        notifier: Arc<dyn crate::background_tasks::BackgroundTaskNotifier>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db = crate::db::connect_and_migrate(&paths.database_path).await?;
        let agent_configuration = Arc::new(
            crate::agent::configuration::AgentConfiguration::from_agents_data_root(
                &paths.agents_data_dir,
            )?,
        );
        let agent_chat_provider = agent_configuration.configured_chat_provider();
        std::fs::create_dir_all(&paths.agents_data_dir)?;
        let canonical_agents_data_dir = std::fs::canonicalize(&paths.agents_data_dir)?;
        let agent_session_manager = crate::agent::sessions::SessionManager::from_agents_data_root(
            &canonical_agents_data_dir,
        )?;
        let agent_chats = Arc::new(crate::agent::chat_application::AgentChatApplication::new(
            agent_session_manager,
            agent_chat_provider,
        ));
        let source_http = Arc::new(crate::adapters::ReqwestProfileHttpClient::new());
        let source_browser = Arc::new(crate::browser_runtime::ManagedBrowserAcquisition::new(
            paths.browser_runtime_dir.clone(),
        ));
        let installed_sources = sources::installed::Store::new(paths.app_data_dir.clone());
        let source_detection = sources::detection::Operation::new(
            installed_sources.clone(),
            source_http.clone(),
            source_browser.clone(),
        );
        let source_live_check = sources::live_check::Operation::new(
            installed_sources.clone(),
            source_http,
            source_browser,
            Arc::new(sources::live_check::SystemClock),
        );

        let search_requests = search_requests::Catalog::new(db.clone());

        Ok(Self {
            db,
            paths,
            resources,
            browser_runtime_install_lock: Mutex::new(()),
            search_requests,
            background_tasks: crate::background_tasks::BackgroundTaskScheduler::new_with_notifier(
                crate::background_tasks::BackgroundTaskSchedulerConfig::default(),
                notifier,
            ),
            agent_configuration,
            agent_chats,
            installed_sources,
            source_detection,
            source_live_check,
        })
    }
}

#[cfg(test)]
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

#[cfg(test)]
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
