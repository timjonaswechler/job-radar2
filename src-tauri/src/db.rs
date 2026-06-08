use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;

pub async fn connect_and_migrate(db_path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
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

async fn seed_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO app_metadata (key, value)
         VALUES ('database_initialized', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .execute(pool)
    .await?;

    Ok(())
}
