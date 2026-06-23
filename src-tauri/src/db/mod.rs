pub mod migrations;
pub mod seed;
use crate::db::seed::seed_database;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{error::Error, path::Path};

pub async fn connect_and_migrate(db_path: &Path) -> Result<SqlitePool, Box<dyn Error>> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(options).await?;

    // Embedded at compile time, so packaged builds do not need loose SQL files.
    sqlx::migrate!("./migrations").run(&pool).await?;
    seed_database(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    use std::fs;

    #[test]
    fn fresh_dev_schema_has_no_source_or_profile_domain_tables() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");

            let pool = connect_and_migrate(&database_path).await.unwrap();
            let table_names = sqlx::query_scalar::<_, String>(
                "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            assert!(table_names.contains(&"app_metadata".to_string()));
            assert!(table_names.contains(&"app_settings".to_string()));
            assert!(table_names.contains(&"search_requests".to_string()));
            assert!(table_names.contains(&"job_postings".to_string()));
            assert!(table_names.contains(&"job_posting_sources".to_string()));
            assert!(!table_names.contains(&"system_profiles".to_string()));
            assert!(!table_names.contains(&"browser_profiles".to_string()));
            assert!(!table_names.contains(&"sources".to_string()));
        });
    }

    #[test]
    fn fresh_dev_schema_has_job_posting_work_item_tables() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");

            let pool = connect_and_migrate(&database_path).await.unwrap();

            let search_request_columns = sqlx::query_scalar::<_, String>(
                "SELECT name FROM pragma_table_info('search_requests') ORDER BY cid",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert!(search_request_columns.contains(&"last_run_at".to_string()));
            assert!(search_request_columns.contains(&"last_run_status".to_string()));
            assert!(search_request_columns.contains(&"last_run_error".to_string()));

            let posting_state_defaults = sqlx::query(
                "INSERT INTO job_postings (title, company)
                 VALUES ('Laser Engineer', 'SCHOTT AG')
                 RETURNING read_state, interest_state, preparation_state, application_state",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                posting_state_defaults
                    .try_get::<String, _>("read_state")
                    .unwrap(),
                "unread"
            );
            assert_eq!(
                posting_state_defaults
                    .try_get::<String, _>("interest_state")
                    .unwrap(),
                "undecided"
            );
            assert_eq!(
                posting_state_defaults
                    .try_get::<String, _>("preparation_state")
                    .unwrap(),
                "not_started"
            );
            assert_eq!(
                posting_state_defaults
                    .try_get::<String, _>("application_state")
                    .unwrap(),
                "not_applied"
            );

            let posting_id = sqlx::query_scalar::<_, i64>("SELECT id FROM job_postings")
                .fetch_one(&pool)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO job_posting_sources (posting_id, source_key, source_name_snapshot, url)
                 VALUES (?1, 'schott_ag', 'SCHOTT AG', 'https://example.test/jobs/laser')",
            )
            .bind(posting_id)
            .execute(&pool)
            .await
            .unwrap();
        });
    }

    #[test]
    fn connect_and_migrate_records_database_initialization_metadata_only() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");

            let pool = connect_and_migrate(&database_path).await.unwrap();

            let initialized_at = sqlx::query_scalar::<_, String>(
                "SELECT value FROM app_metadata WHERE key = 'database_initialized'",
            )
            .fetch_optional(&pool)
            .await
            .unwrap();
            assert!(initialized_at.is_some());

            let metadata_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM app_metadata")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(metadata_count, 1);
        });
    }

    #[test]
    fn connect_and_migrate_does_not_read_or_seed_legacy_custom_system_profiles() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");
            fs::create_dir_all(&custom_profiles_dir).unwrap();
            fs::write(custom_profiles_dir.join("broken.json"), "{not json").unwrap();

            let pool = connect_and_migrate(&database_path).await.unwrap();
            let table_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'system_profiles'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();

            assert_eq!(table_exists, 0);
            assert_eq!(
                fs::read_to_string(custom_profiles_dir.join("broken.json")).unwrap(),
                "{not json"
            );
        });
    }
}
