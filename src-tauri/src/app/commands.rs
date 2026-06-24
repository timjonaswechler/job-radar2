use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{fs, path::Path};
use tauri::{AppHandle, Emitter, State};

use crate::app::state::AppState;

const SETTING_THEME: &str = "theme";
const SETTING_LANGUAGE: &str = "language";
const SETTING_DEFAULT_SEARCH_RADIUS_KM: &str = "default_search_radius_km";
const SETTING_BASE_FONT_SIZE_PX: &str = "base_font_size_px";
const DEFAULT_SEARCH_RADIUS_KM: u16 = 30;
const MAX_SEARCH_RADIUS_KM: u16 = 500;
const DEFAULT_BASE_FONT_SIZE_PX: u16 = 16;
const MIN_BASE_FONT_SIZE_PX: u16 = 12;
const MAX_BASE_FONT_SIZE_PX: u16 = 24;

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
    source_profiles_dir: String,
    sources_dir: String,
    initialized_at: Option<String>,
    sqlite_version: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AppLanguage {
    #[default]
    De,
    En,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPreferences {
    theme: AppTheme,
    language: AppLanguage,
    default_search_radius_km: u16,
    base_font_size_px: u16,
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
        source_profiles_dir: state
            .paths
            .source_profiles_dir
            .to_string_lossy()
            .to_string(),
        sources_dir: state.paths.sources_dir.to_string_lossy().to_string(),
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
    validate_base_font_size(preferences.base_font_size_px)?;
    write_setting(&state.db, SETTING_THEME, &preferences.theme).await?;
    write_setting(&state.db, SETTING_LANGUAGE, &preferences.language).await?;
    write_setting(
        &state.db,
        SETTING_DEFAULT_SEARCH_RADIUS_KM,
        &preferences.default_search_radius_km,
    )
    .await?;
    write_setting(
        &state.db,
        SETTING_BASE_FONT_SIZE_PX,
        &preferences.base_font_size_px,
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
pub async fn set_base_font_size_px(
    state: State<'_, AppState>,
    base_font_size_px: u16,
) -> Result<AppPreferences, String> {
    validate_base_font_size(base_font_size_px)?;
    write_setting(&state.db, SETTING_BASE_FONT_SIZE_PX, &base_font_size_px).await?;
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

fn load_source_registry_snapshot(
    app_data_dir: &Path,
) -> crate::source::registry::SourceRegistrySnapshot {
    crate::source::registry::load_snapshot(app_data_dir)
}

#[tauri::command]
pub fn list_source_registry_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source::registry::RegistrySourceProfile>, String> {
    Ok(load_source_registry_snapshot(&state.paths.app_data_dir).valid_profiles)
}

#[tauri::command]
pub fn list_source_registry_sources(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source::registry::RegistrySource>, String> {
    Ok(load_source_registry_snapshot(&state.paths.app_data_dir).valid_sources)
}

#[tauri::command]
pub fn list_source_registry_diagnostics(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source::registry::SourceRegistryDiagnostic>, String> {
    Ok(load_source_registry_snapshot(&state.paths.app_data_dir).diagnostics)
}

#[tauri::command]
pub async fn detect_source_from_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<crate::source::detection::SourceDetectionResult, String> {
    crate::source::detection::detect_source_from_url(&state.paths.app_data_dir, &url).await
}

#[tauri::command]
pub fn create_custom_source(
    state: State<'_, AppState>,
    document: crate::source::registry::SourceDocument,
) -> Result<crate::source::registry::RegistrySource, String> {
    let snapshot = load_source_registry_snapshot(&state.paths.app_data_dir);

    if snapshot
        .valid_sources
        .iter()
        .any(|source| source.document.key == document.key)
    {
        return Err(format!(
            "Eine Quelle mit dem Key `{}` existiert bereits.",
            document.key
        ));
    }

    if let crate::source::registry::SelectedAccessPath::Profile {
        profile_key,
        path_key,
    } = &document.selected_access_path
    {
        let profile = snapshot
            .profile(profile_key)
            .ok_or_else(|| format!("Das Quellenprofil `{profile_key}` wurde nicht gefunden."))?;
        let path_exists = profile
            .document
            .access_paths
            .iter()
            .any(|access_path| access_path.key == *path_key);
        if !path_exists {
            return Err(format!(
                "Der Zugriffspfad `{path_key}` wurde im Profil `{profile_key}` nicht gefunden."
            ));
        }
    }

    fs::create_dir_all(&state.paths.sources_dir)
        .map_err(|error| format!("Sources-Ordner konnte nicht angelegt werden: {error}"))?;
    let path = state
        .paths
        .sources_dir
        .join(format!("{}.json", document.key));
    if path.exists() {
        return Err(format!("Die Datei `{}` existiert bereits.", path.display()));
    }

    let contents = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("Quelle konnte nicht serialisiert werden: {error}"))?;
    fs::write(&path, format!("{contents}\n"))
        .map_err(|error| format!("Quelle konnte nicht geschrieben werden: {error}"))?;

    Ok(crate::source::registry::RegistrySource {
        origin: crate::source::registry::SourceRegistryDocumentOrigin::Custom,
        path: path.to_string_lossy().to_string(),
        document,
    })
}

#[tauri::command]
pub async fn create_search_request(
    state: State<'_, AppState>,
    input: crate::search::request::CreateSearchRequestInput,
) -> Result<crate::search::request::SearchRequest, String> {
    crate::search::request::SearchRequestService::new(&state.db, &state.running_search_runs)
        .create(input)
        .await
}

#[tauri::command]
pub async fn list_search_requests(
    state: State<'_, AppState>,
) -> Result<Vec<crate::search::request::SearchRequest>, String> {
    crate::search::request::SearchRequestService::new(&state.db, &state.running_search_runs)
        .list()
        .await
}

#[tauri::command]
pub async fn get_search_request(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::search::request::SearchRequest, String> {
    crate::search::request::SearchRequestService::new(&state.db, &state.running_search_runs)
        .get(id)
        .await
}

#[tauri::command]
pub async fn update_search_request(
    state: State<'_, AppState>,
    id: i64,
    input: crate::search::request::UpdateSearchRequestInput,
) -> Result<crate::search::request::SearchRequest, String> {
    crate::search::request::SearchRequestService::new(&state.db, &state.running_search_runs)
        .update(id, input)
        .await
}

#[tauri::command]
pub async fn delete_search_request(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    crate::search::request::SearchRequestService::new(&state.db, &state.running_search_runs)
        .delete(id)
        .await
}

#[tauri::command]
pub async fn run_search_request(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::search::run::SearchRunResult, String> {
    let source_executor =
        crate::search::run::DefaultSourceExecutor::new(state.paths.browser_runtime_dir.clone());
    crate::search::run::SearchRunService::new_with_result_artifact(
        &state.db,
        &state.running_search_runs,
        &source_executor,
        crate::search::run::default_search_run_result_artifact(),
        state.paths.app_data_dir.clone(),
    )
    .run(id)
    .await
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
        base_font_size_px: read_setting_or_default_value(
            pool,
            SETTING_BASE_FONT_SIZE_PX,
            DEFAULT_BASE_FONT_SIZE_PX,
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

fn validate_base_font_size(base_font_size_px: u16) -> Result<(), String> {
    if !(MIN_BASE_FONT_SIZE_PX..=MAX_BASE_FONT_SIZE_PX).contains(&base_font_size_px) {
        return Err(format!(
            "baseFontSizePx must be between {MIN_BASE_FONT_SIZE_PX} and {MAX_BASE_FONT_SIZE_PX}"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_starts_without_source_or_profile_domain_tables() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();

            let removed_tables = sqlx::query_scalar::<_, String>(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name IN ('system_profiles', 'browser_profiles', 'sources')
                 ORDER BY name",
            )
            .fetch_all(&state.db)
            .await
            .unwrap();
            assert!(removed_tables.is_empty());

            let registry_snapshot =
                crate::source::registry::load_snapshot(&state.paths.app_data_dir);
            assert!(registry_snapshot.diagnostics.is_empty());
            assert!(registry_snapshot.source("stepstone_de").is_some());
            assert!(registry_snapshot.profile("greenhouse").is_some());
        });
    }

    #[test]
    fn source_registry_commands_read_current_registry_snapshot() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();

            let snapshot = load_source_registry_snapshot(&state.paths.app_data_dir);

            assert!(snapshot.diagnostics.is_empty());
            assert!(snapshot
                .valid_profiles
                .iter()
                .any(|profile| profile.document.key == "greenhouse"));
            assert!(snapshot
                .valid_sources
                .iter()
                .any(|source| source.document.key == "stepstone_de"));
        });
    }

    #[test]
    fn app_preferences_include_default_search_radius() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();

            let preferences = read_app_preferences(&state.db).await.unwrap();
            assert_eq!(
                preferences.default_search_radius_km,
                DEFAULT_SEARCH_RADIUS_KM
            );
            assert_eq!(preferences.base_font_size_px, DEFAULT_BASE_FONT_SIZE_PX);

            write_setting(&state.db, SETTING_DEFAULT_SEARCH_RADIUS_KM, &50_u16)
                .await
                .unwrap();
            write_setting(&state.db, SETTING_BASE_FONT_SIZE_PX, &18_u16)
                .await
                .unwrap();
            let preferences = read_app_preferences(&state.db).await.unwrap();
            assert_eq!(preferences.default_search_radius_km, 50);
            assert_eq!(preferences.base_font_size_px, 18);
            assert!(validate_search_radius(MAX_SEARCH_RADIUS_KM + 1).is_err());
            assert!(validate_base_font_size(MIN_BASE_FONT_SIZE_PX - 1).is_err());
            assert!(validate_base_font_size(MAX_BASE_FONT_SIZE_PX + 1).is_err());
        });
    }

    #[test]
    fn browser_runtime_installing_reflects_install_lock_state() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();

            assert!(!browser_runtime_installing(&state));
            let guard = state.browser_runtime_install_lock.try_lock().unwrap();
            assert!(browser_runtime_installing(&state));
            drop(guard);
            assert!(!browser_runtime_installing(&state));
        });
    }
}
