use sqlx::SqlitePool;
use std::error::Error;

pub type SeedResult<T> = Result<T, Box<dyn Error>>;

pub async fn seed_database(pool: &SqlitePool) -> SeedResult<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO app_metadata (key, value)
         VALUES ('database_initialized', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .execute(pool)
    .await?;

    Ok(())
}
