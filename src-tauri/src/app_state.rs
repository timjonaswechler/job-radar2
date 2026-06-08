use sqlx::SqlitePool;

use crate::paths::AppPaths;

pub struct AppState {
    pub db: SqlitePool,
    pub paths: AppPaths,
}

impl AppState {
    pub async fn new(paths: AppPaths) -> Result<Self, Box<dyn std::error::Error>> {
        let db = crate::db::connect_and_migrate(&paths.database_path).await?;

        Ok(Self { db, paths })
    }
}
