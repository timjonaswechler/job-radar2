use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub async fn connect_and_migrate(db_path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let database_existed = db_path.exists();

    if database_existed {
        backup_database_before_migration(db_path)?;
    }

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(options).await?;

    // Embedded at compile time, so packaged desktop builds do not need loose .sql files.
    sqlx::migrate!("./migrations").run(&pool).await?;

    if !database_existed {
        seed_new_database(&pool).await?;
    }

    Ok(pool)
}

fn backup_database_before_migration(db_path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let backup_path = db_path.with_extension(format!("before-migration-{timestamp}.db"));

    fs::copy(db_path, &backup_path)?;

    copy_if_exists(&db_path.with_extension("db-wal"), &backup_path.with_extension("db-wal"))?;
    copy_if_exists(&db_path.with_extension("db-shm"), &backup_path.with_extension("db-shm"))?;

    Ok(backup_path)
}

fn copy_if_exists(from: &Path, to: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if from.exists() {
        fs::copy(from, to)?;
    }

    Ok(())
}

async fn seed_new_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO app_metadata (key, value)
         VALUES ('database_initialized', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .execute(pool)
    .await?;

    Ok(())
}
