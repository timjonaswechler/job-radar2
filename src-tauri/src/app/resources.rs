use std::path::PathBuf;
use tauri::{path::BaseDirectory, AppHandle, Manager};

const GEO_DB_PATH: &str = "resources/geo_loc.sqlite";

#[derive(Clone, Debug)]
pub struct AppResources {
    pub geo_db_path: PathBuf,
}

impl AppResources {
    pub fn from_app(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            geo_db_path: app.path().resolve(GEO_DB_PATH, BaseDirectory::Resource)?,
        })
    }

    pub fn from_geo_db_path(geo_db_path: PathBuf) -> Self {
        Self { geo_db_path }
    }
}

impl Default for AppResources {
    fn default() -> Self {
        Self::from_geo_db_path(PathBuf::from(GEO_DB_PATH))
    }
}
