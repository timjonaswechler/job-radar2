use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};

use crate::app_state::AppState;

const SETTING_THEME: &str = "theme";
const SETTING_LANGUAGE: &str = "language";
const SETTING_DEFAULT_SEARCH_RADIUS_KM: &str = "default_search_radius_km";
const DEFAULT_SEARCH_RADIUS_KM: u16 = 30;
const MAX_SEARCH_RADIUS_KM: u16 = 500;

struct TauriBrowserRuntimeProgressReporter {
    app: AppHandle,
}

impl crate::browser_runtime::BrowserRuntimeInstallProgressReporter
    for TauriBrowserRuntimeProgressReporter
{
    fn emit(&self, progress: crate::browser_runtime::BrowserRuntimeInstallProgress) {
        let _ = self
            .app
            .emit(crate::browser_runtime::INSTALL_PROGRESS_EVENT, progress);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    app_data_dir: String,
    database_path: String,
    system_profiles_dir: String,
    initialized_at: Option<String>,
    sqlite_version: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    Light,
    Dark,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    De,
    En,
}

impl Default for AppLanguage {
    fn default() -> Self {
        Self::De
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    theme: AppTheme,
    language: AppLanguage,
    default_search_radius_km: u16,
}

#[tauri::command]
pub async fn get_database_info(state: State<'_, AppState>) -> Result<DatabaseInfo, String> {
    let initialized_at = sqlx::query_scalar::<_, String>(
        "SELECT value FROM app_metadata WHERE key = 'database_initialized'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|error| error.to_string())?;

    let sqlite_version = sqlx::query_scalar::<_, String>("SELECT sqlite_version()")
        .fetch_one(&state.db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(DatabaseInfo {
        app_data_dir: state.paths.app_data_dir.to_string_lossy().to_string(),
        database_path: state.paths.database_path.to_string_lossy().to_string(),
        system_profiles_dir: state
            .paths
            .system_profiles_dir
            .to_string_lossy()
            .to_string(),
        initialized_at,
        sqlite_version,
    })
}

#[tauri::command]
pub async fn get_app_preferences(state: State<'_, AppState>) -> Result<AppPreferences, String> {
    read_app_preferences(&state.db).await
}

#[tauri::command]
pub async fn set_app_preferences(
    state: State<'_, AppState>,
    preferences: AppPreferences,
) -> Result<AppPreferences, String> {
    validate_search_radius(preferences.default_search_radius_km)?;
    write_setting(&state.db, SETTING_THEME, &preferences.theme).await?;
    write_setting(&state.db, SETTING_LANGUAGE, &preferences.language).await?;
    write_setting(
        &state.db,
        SETTING_DEFAULT_SEARCH_RADIUS_KM,
        &preferences.default_search_radius_km,
    )
    .await?;

    read_app_preferences(&state.db).await
}

#[tauri::command]
pub async fn set_app_theme(
    state: State<'_, AppState>,
    theme: AppTheme,
) -> Result<AppPreferences, String> {
    write_setting(&state.db, SETTING_THEME, &theme).await?;
    read_app_preferences(&state.db).await
}

#[tauri::command]
pub async fn set_app_language(
    state: State<'_, AppState>,
    language: AppLanguage,
) -> Result<AppPreferences, String> {
    write_setting(&state.db, SETTING_LANGUAGE, &language).await?;
    read_app_preferences(&state.db).await
}

#[tauri::command]
pub async fn set_default_search_radius_km(
    state: State<'_, AppState>,
    radius_km: u16,
) -> Result<AppPreferences, String> {
    validate_search_radius(radius_km)?;
    write_setting(&state.db, SETTING_DEFAULT_SEARCH_RADIUS_KM, &radius_km).await?;
    read_app_preferences(&state.db).await
}

#[tauri::command]
pub async fn get_browser_runtime_status(
    state: State<'_, AppState>,
) -> Result<crate::browser_runtime::BrowserRuntimeStatus, String> {
    let installing = browser_runtime_installing(&state);
    let spec = crate::browser_runtime::current_runtime_spec();
    Ok(crate::browser_runtime::status_for_runtime_dir(
        &state.paths.browser_runtime_dir,
        spec.as_ref(),
        installing,
    ))
}

#[tauri::command]
pub async fn install_browser_runtime(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::browser_runtime::BrowserRuntimeStatus, String> {
    let spec = crate::browser_runtime::current_runtime_spec().ok_or_else(|| {
        format!(
            "managed browser runtime is unsupported on {}",
            crate::browser_runtime::current_platform()
        )
    })?;
    let _install_guard = state
        .browser_runtime_install_lock
        .try_lock()
        .map_err(|_| "managed browser runtime installation is already running".to_string())?;
    let progress = TauriBrowserRuntimeProgressReporter { app };
    let downloader = crate::browser_runtime::ReqwestRuntimeDownloader::default();

    crate::browser_runtime::install_runtime(
        &state.paths.browser_runtime_dir,
        &spec,
        &downloader,
        &crate::browser_runtime::ZipRuntimeArchiveExtractor,
        &progress,
    )
    .await
}

#[tauri::command]
pub async fn uninstall_browser_runtime(
    state: State<'_, AppState>,
) -> Result<crate::browser_runtime::BrowserRuntimeStatus, String> {
    let _install_guard = state
        .browser_runtime_install_lock
        .try_lock()
        .map_err(|_| "managed browser runtime installation is already running".to_string())?;
    let spec = crate::browser_runtime::current_runtime_spec();
    crate::browser_runtime::uninstall_runtime(&state.paths.browser_runtime_dir, spec.as_ref())
}

#[tauri::command]
pub async fn check_browser_runtime(
    state: State<'_, AppState>,
) -> Result<crate::browser_runtime::BrowserRuntimeCheckResult, String> {
    let installing = browser_runtime_installing(&state);
    let spec = crate::browser_runtime::current_runtime_spec();
    Ok(crate::browser_runtime::check_runtime(
        &state.paths.browser_runtime_dir,
        spec.as_ref(),
        installing,
    )
    .await)
}

fn browser_runtime_installing(state: &AppState) -> bool {
    match state.browser_runtime_install_lock.try_lock() {
        Ok(guard) => {
            drop(guard);
            false
        }
        Err(_) => true,
    }
}

#[tauri::command]
pub fn list_adapters() -> Result<Vec<crate::adapter_registry::AdapterMetadata>, String> {
    Ok(crate::adapter_registry::list_adapters())
}

#[tauri::command]
pub async fn detect_source_from_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<crate::source_detection::SourceDetectionResult, String> {
    crate::source_detection::detect_source_from_url(&state.db, &url).await
}

async fn read_app_preferences(pool: &SqlitePool) -> Result<AppPreferences, String> {
    Ok(AppPreferences {
        theme: read_setting_or_default(pool, SETTING_THEME).await?,
        language: read_setting_or_default(pool, SETTING_LANGUAGE).await?,
        default_search_radius_km: read_setting_or_default_value(
            pool,
            SETTING_DEFAULT_SEARCH_RADIUS_KM,
            DEFAULT_SEARCH_RADIUS_KM,
        )
        .await?,
    })
}

async fn read_setting_or_default_value<T>(
    pool: &SqlitePool,
    key: &str,
    default_value: T,
) -> Result<T, String>
where
    T: DeserializeOwned + Serialize + Copy,
{
    let value_json =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|error| error.to_string())?;

    match value_json {
        Some(value_json) => serde_json::from_str(&value_json).map_err(|error| error.to_string()),
        None => {
            write_setting(pool, key, &default_value).await?;
            Ok(default_value)
        }
    }
}

fn validate_search_radius(radius_km: u16) -> Result<(), String> {
    if radius_km > MAX_SEARCH_RADIUS_KM {
        return Err(format!(
            "defaultSearchRadiusKm must be less than or equal to {MAX_SEARCH_RADIUS_KM}"
        ));
    }

    Ok(())
}

async fn read_setting_or_default<T>(pool: &SqlitePool, key: &str) -> Result<T, String>
where
    T: DeserializeOwned + Default + Serialize,
{
    let value_json =
        sqlx::query_scalar::<_, String>("SELECT value_json FROM app_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|error| error.to_string())?;

    match value_json {
        Some(value_json) => serde_json::from_str(&value_json).map_err(|error| error.to_string()),
        None => {
            let default_value = T::default();
            write_setting(pool, key, &default_value).await?;
            Ok(default_value)
        }
    }
}

async fn write_setting<T>(pool: &SqlitePool, key: &str, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let value_json = serde_json::to_string(value).map_err(|error| error.to_string())?;

    sqlx::query(
        "INSERT INTO app_settings (key, value_json)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(key)
    .bind(value_json)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn create_browser_profile(
    state: State<'_, AppState>,
    input: crate::source_model::CreateBrowserProfileInput,
) -> Result<crate::source_model::BrowserProfile, String> {
    crate::source_model::create_browser_profile(&state.db, input).await
}

#[tauri::command]
pub async fn list_browser_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source_model::BrowserProfile>, String> {
    crate::source_model::list_browser_profiles(&state.db).await
}

#[tauri::command]
pub async fn get_browser_profile(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::source_model::BrowserProfile, String> {
    crate::source_model::get_browser_profile(&state.db, id).await
}

#[tauri::command]
pub async fn update_browser_profile(
    state: State<'_, AppState>,
    id: i64,
    input: crate::source_model::UpdateBrowserProfileInput,
) -> Result<crate::source_model::BrowserProfile, String> {
    crate::source_model::update_browser_profile(&state.db, id, input).await
}

#[tauri::command]
pub async fn delete_browser_profile(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    crate::source_model::delete_browser_profile(&state.db, id).await
}

#[tauri::command]
pub async fn create_system_profile(
    state: State<'_, AppState>,
    input: crate::source_model::CreateSystemProfileInput,
) -> Result<crate::source_model::SystemProfile, String> {
    crate::source_model::create_system_profile(&state.db, input).await
}

#[tauri::command]
pub async fn list_system_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source_model::SystemProfile>, String> {
    crate::source_model::list_system_profiles(&state.db).await
}

#[tauri::command]
pub async fn get_system_profile(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::source_model::SystemProfile, String> {
    crate::source_model::get_system_profile(&state.db, id).await
}

#[tauri::command]
pub async fn update_system_profile(
    state: State<'_, AppState>,
    id: i64,
    input: crate::source_model::UpdateSystemProfileInput,
) -> Result<crate::source_model::SystemProfile, String> {
    crate::source_model::update_system_profile(&state.db, id, input).await
}

#[tauri::command]
pub async fn delete_system_profile(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    crate::source_model::delete_system_profile(&state.db, id).await
}

#[tauri::command]
pub async fn create_source(
    state: State<'_, AppState>,
    input: crate::source_model::CreateSourceInput,
) -> Result<crate::source_model::Source, String> {
    crate::source_model::create_source(&state.db, input).await
}

#[tauri::command]
pub async fn list_sources(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source_model::Source>, String> {
    crate::source_model::list_sources(&state.db).await
}

#[tauri::command]
pub async fn get_source(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::source_model::Source, String> {
    crate::source_model::get_source(&state.db, id).await
}

#[tauri::command]
pub async fn update_source(
    state: State<'_, AppState>,
    id: i64,
    input: crate::source_model::UpdateSourceInput,
) -> Result<crate::source_model::Source, String> {
    crate::source_model::update_source(&state.db, id, input).await
}

#[tauri::command]
pub async fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    crate::source_model::delete_source(&state.db, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_seeds_builtin_job_portal_sources() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf()).unwrap();
            let state = AppState::new(paths).await.unwrap();

            let browser_profiles = crate::source_model::list_browser_profiles(&state.db)
                .await
                .unwrap();
            assert!(browser_profiles
                .iter()
                .any(|profile| profile.key == "job_portal_manual_release"));

            let system_profiles = crate::source_model::list_system_profiles(&state.db)
                .await
                .unwrap();
            assert!(system_profiles
                .iter()
                .any(|profile| profile.key == "muz_global_jobboard" && profile.built_in));
            assert!(system_profiles.iter().all(|profile| !matches!(
                profile.adapter_key.as_str(),
                "greenhouse" | "lever" | "ashby"
            )));

            let sources = crate::source_model::list_sources(&state.db).await.unwrap();
            let stepstone = sources
                .iter()
                .find(|source| source.key == "stepstone_de")
                .expect("missing built-in StepStone source");
            assert_eq!(stepstone.adapter_key, "stepstone_search");
            assert!(stepstone.browser_profile_id.is_some());
            assert!(stepstone.built_in);
            assert_eq!(stepstone.source_config, serde_json::json!({}));

            let indeed = sources
                .iter()
                .find(|source| source.key == "indeed_de")
                .expect("missing built-in Indeed source");
            assert_eq!(indeed.adapter_key, "indeed_search");
            assert!(indeed.browser_profile_id.is_some());
            assert!(indeed.built_in);

            assert!(crate::source_model::delete_source(&state.db, stepstone.id)
                .await
                .unwrap_err()
                .contains("built-in sources cannot be deleted"));
        });
    }

    #[test]
    fn app_preferences_include_default_search_radius() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf()).unwrap();
            let state = AppState::new(paths).await.unwrap();

            let preferences = read_app_preferences(&state.db).await.unwrap();
            assert_eq!(
                preferences.default_search_radius_km,
                DEFAULT_SEARCH_RADIUS_KM
            );

            write_setting(&state.db, SETTING_DEFAULT_SEARCH_RADIUS_KM, &50_u16)
                .await
                .unwrap();
            let preferences = read_app_preferences(&state.db).await.unwrap();
            assert_eq!(preferences.default_search_radius_km, 50);
            assert!(validate_search_radius(MAX_SEARCH_RADIUS_KM + 1).is_err());
        });
    }

    #[test]
    fn browser_runtime_installing_reflects_install_lock_state() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf()).unwrap();
            let state = AppState::new(paths).await.unwrap();

            assert!(!browser_runtime_installing(&state));
            let guard = state.browser_runtime_install_lock.try_lock().unwrap();
            assert!(browser_runtime_installing(&state));
            drop(guard);
            assert!(!browser_runtime_installing(&state));
        });
    }
}
