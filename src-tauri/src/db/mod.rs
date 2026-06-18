pub mod migrations;
pub mod seed;
use crate::db::seed::seed_database;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{error::Error, path::Path};

pub async fn connect_and_migrate(
    db_path: &Path,
    _custom_system_profiles_dir: &Path,
) -> Result<SqlitePool, Box<dyn Error>> {
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
    use std::fs;

    #[test]
    fn fresh_dev_schema_has_no_source_or_profile_domain_tables() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");

            let pool = connect_and_migrate(&database_path, &custom_profiles_dir)
                .await
                .unwrap();
            let table_names = sqlx::query_scalar::<_, String>(
                "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            assert!(table_names.contains(&"app_metadata".to_string()));
            assert!(table_names.contains(&"app_settings".to_string()));
            assert!(table_names.contains(&"search_requests".to_string()));
            assert!(!table_names.contains(&"system_profiles".to_string()));
            assert!(!table_names.contains(&"browser_profiles".to_string()));
            assert!(!table_names.contains(&"sources".to_string()));
        });
    }

    #[test]
    fn connect_and_migrate_records_database_initialization_metadata_only() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let database_path = temp_dir.path().join("job_radar.db");
            let custom_profiles_dir = temp_dir.path().join("system-profiles");

            let pool = connect_and_migrate(&database_path, &custom_profiles_dir)
                .await
                .unwrap();

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

            let pool = connect_and_migrate(&database_path, &custom_profiles_dir)
                .await
                .unwrap();
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
