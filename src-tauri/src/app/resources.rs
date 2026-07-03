use std::path::PathBuf;
use tauri::{path::BaseDirectory, AppHandle, Manager};

const GEO_SEED_DB_PATH: &str = "resources/geo_seed.sqlite";

#[derive(Clone, Debug)]
pub struct AppResources {
    pub geo_seed_path: PathBuf,
}

impl AppResources {
    pub fn from_app(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            geo_seed_path: app
                .path()
                .resolve(GEO_SEED_DB_PATH, BaseDirectory::Resource)?,
        })
    }

    pub fn from_geo_seed_path(geo_seed_path: PathBuf) -> Self {
        Self { geo_seed_path }
    }
}

impl Default for AppResources {
    fn default() -> Self {
        Self::from_geo_seed_path(PathBuf::from(GEO_SEED_DB_PATH))
    }
}
