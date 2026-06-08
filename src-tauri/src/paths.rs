use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DB_NAME: &str = "job_radar.db";

pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl AppPaths {
    pub fn from_app(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&app_data_dir)?;

        let database_path = app_data_dir.join(DB_NAME);

        Ok(Self {
            app_data_dir,
            database_path,
        })
    }
}
