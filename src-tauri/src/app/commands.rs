use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{fs, path::Path};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::app::state::AppState;

const SETTING_THEME: &str = "theme";
const SETTING_LANGUAGE: &str = "language";
const SETTING_DEFAULT_SEARCH_RADIUS_KM: &str = "default_search_radius_km";
const SETTING_BASE_FONT_SIZE_PX: &str = "base_font_size_px";
const SETTING_WINDOW_DRAG_REGION_ENABLED: &str = "window_drag_region_enabled";
const DEFAULT_SEARCH_RADIUS_KM: u16 = 30;
const MAX_SEARCH_RADIUS_KM: u16 = 500;
const DEFAULT_BASE_FONT_SIZE_PX: u16 = 16;
const MIN_BASE_FONT_SIZE_PX: u16 = 12;
const MAX_BASE_FONT_SIZE_PX: u16 = 24;
pub const AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT: &str = "agent-subscription-login-progress";

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
    source_live_checks_dir: String,
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
    window_drag_region_enabled: bool,
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
        source_live_checks_dir: state
            .paths
            .source_live_checks_dir
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
    write_setting(
        &state.db,
        SETTING_WINDOW_DRAG_REGION_ENABLED,
        &preferences.window_drag_region_enabled,
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
pub async fn set_window_drag_region_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<AppPreferences, String> {
    write_setting(&state.db, SETTING_WINDOW_DRAG_REGION_ENABLED, &enabled).await?;
    read_app_preferences(&state.db).await
}

struct TauriAgentOpener {
    app: AppHandle,
}

impl crate::agent::configuration::ExternalUrlOpener for TauriAgentOpener {
    fn open(&self, url: &str) -> Result<(), crate::agent::configuration::OpenError> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|_| crate::agent::configuration::OpenError)
    }
}

impl crate::agent::configuration::AgentDataFolderOpener for TauriAgentOpener {
    fn open(&self, path: &Path) -> Result<(), crate::agent::configuration::OpenError> {
        self.app
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|_| crate::agent::configuration::OpenError)
    }
}

struct TauriSubscriptionLoginProgressReporter {
    app: AppHandle,
}

impl crate::agent::configuration::SubscriptionLoginProgressReporter
    for TauriSubscriptionLoginProgressReporter
{
    fn report(&self, progress: crate::agent::configuration::SubscriptionLoginProgress) {
        let _ = self
            .app
            .emit(AGENT_SUBSCRIPTION_LOGIN_PROGRESS_EVENT, progress);
    }
}

#[tauri::command]
pub fn get_agent_configuration_status(
    state: State<'_, AppState>,
) -> crate::agent::configuration::AgentConfigurationStatus {
    state.agent_configuration.status()
}

#[tauri::command]
pub async fn submit_agent_api_key(
    state: State<'_, AppState>,
    provider_id: String,
    api_key: crate::agent::configuration::SecretApiKeyInput,
) -> Result<
    crate::agent::configuration::AgentConfigurationStatus,
    crate::agent::configuration::AgentConfigurationError,
> {
    let configuration = std::sync::Arc::clone(&state.agent_configuration);
    tauri::async_runtime::spawn_blocking(move || {
        configuration.submit_api_key(&provider_id, api_key)
    })
    .await
    .map_err(|_| crate::agent::configuration::AgentConfigurationError::unavailable())?
}

#[tauri::command]
pub async fn login_agent_subscription(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<
    crate::agent::configuration::AgentConfigurationStatus,
    crate::agent::configuration::AgentConfigurationError,
> {
    let opener = TauriAgentOpener { app: app.clone() };
    let progress = TauriSubscriptionLoginProgressReporter { app };
    state
        .agent_configuration
        .login_subscription(&provider_id, &opener, &progress)
        .await
}

#[tauri::command]
pub fn cancel_agent_subscription_login(state: State<'_, AppState>, provider_id: String) -> bool {
    state
        .agent_configuration
        .cancel_subscription_login(&provider_id)
}

#[tauri::command]
pub async fn remove_agent_authentication(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<
    crate::agent::configuration::AgentConfigurationStatus,
    crate::agent::configuration::AgentConfigurationError,
> {
    let configuration = std::sync::Arc::clone(&state.agent_configuration);
    tauri::async_runtime::spawn_blocking(move || configuration.remove_authentication(&provider_id))
        .await
        .map_err(|_| crate::agent::configuration::AgentConfigurationError::unavailable())?
}

#[tauri::command]
pub async fn reload_agent_configuration(
    state: State<'_, AppState>,
) -> Result<
    crate::agent::configuration::AgentConfigurationStatus,
    crate::agent::configuration::AgentConfigurationError,
> {
    let configuration = std::sync::Arc::clone(&state.agent_configuration);
    tauri::async_runtime::spawn_blocking(move || configuration.reload())
        .await
        .map_err(|_| crate::agent::configuration::AgentConfigurationError::unavailable())
}

#[tauri::command]
pub fn open_agent_data_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), crate::agent::configuration::AgentConfigurationError> {
    state
        .agent_configuration
        .open_data_folder(&TauriAgentOpener { app })
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

fn load_source_profile_registry_snapshot(
    app_data_dir: &Path,
) -> crate::source_profile::registry::SourceProfileRegistrySnapshot {
    crate::source_profile::registry::load_snapshot(app_data_dir)
}

#[tauri::command]
pub fn get_source_profile_registry_snapshot(
    state: State<'_, AppState>,
) -> Result<crate::source_profile::registry::SourceProfileRegistrySnapshot, String> {
    Ok(load_source_profile_registry_snapshot(
        &state.paths.app_data_dir,
    ))
}

#[tauri::command]
pub fn list_source_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source_profile::registry::RegistrySourceProfile>, String> {
    Ok(load_source_profile_registry_snapshot(&state.paths.app_data_dir).profiles)
}

#[tauri::command]
pub fn list_sources(
    state: State<'_, AppState>,
) -> Result<Vec<crate::source_profile::registry::RegistrySource>, String> {
    Ok(load_source_profile_registry_snapshot(&state.paths.app_data_dir).sources)
}

#[tauri::command]
pub fn list_source_diagnostics(
    state: State<'_, AppState>,
) -> Result<crate::profile_dsl::diagnostics::Diagnostics, String> {
    Ok(load_source_profile_registry_snapshot(&state.paths.app_data_dir).diagnostics)
}

#[tauri::command]
pub fn check_source(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<crate::checks::CheckReport, String> {
    let discovery_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let detail_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let browser = crate::profile_dsl::runtime::ManagedProfileBrowserClient::new(
        state.paths.browser_runtime_dir.clone(),
    );
    crate::checks::check_source_with_clients(
        &state.paths.app_data_dir,
        source_key,
        &discovery_fetcher,
        &detail_fetcher,
        &browser,
    )
}

#[tauri::command]
pub fn check_and_activate_source(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<crate::checks::CheckReport, String> {
    let discovery_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let detail_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let browser = crate::profile_dsl::runtime::ManagedProfileBrowserClient::new(
        state.paths.browser_runtime_dir.clone(),
    );
    crate::checks::check_and_activate_source_with_clients(
        &state.paths.app_data_dir,
        source_key,
        &discovery_fetcher,
        &detail_fetcher,
        &browser,
    )
}

#[tauri::command]
pub fn check_and_reactivate_source(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<crate::checks::CheckReport, String> {
    let discovery_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let detail_fetcher = crate::profile_dsl::runtime::ReqwestProfileHttpClient::new();
    let browser = crate::profile_dsl::runtime::ManagedProfileBrowserClient::new(
        state.paths.browser_runtime_dir.clone(),
    );
    crate::checks::check_and_reactivate_source_with_clients(
        &state.paths.app_data_dir,
        source_key,
        &discovery_fetcher,
        &detail_fetcher,
        &browser,
    )
}

#[tauri::command]
pub fn get_source_live_check_report_status(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<crate::checks::SourceLiveCheckReportStatus, String> {
    crate::checks::source_live_check_report_status(&state.paths.app_data_dir, source_key)
}

#[tauri::command]
pub async fn detect_source_proposal_from_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<crate::source_profile::detection::SourceProposalDetectionResult, String> {
    let http_client = crate::source_profile::detection::ReqwestDetectionHttpClient::new()?;
    let browser_client = crate::profile_dsl::runtime::ManagedProfileBrowserClient::new(
        state.paths.browser_runtime_dir.clone(),
    );
    Ok(detect_source_proposal_from_url_with_clients(
        &state.paths.app_data_dir,
        &url,
        &http_client,
        &browser_client,
    )
    .await)
}

async fn detect_source_proposal_from_url_with_clients<C, B>(
    app_data_dir: &Path,
    url: &str,
    http_client: &C,
    browser_client: &B,
) -> crate::source_profile::detection::SourceProposalDetectionResult
where
    C: crate::source_profile::detection::DetectionHttpClient + Sync,
    B: crate::profile_dsl::runtime::ProfileBrowserClient + Sync,
{
    let snapshot = load_source_profile_registry_snapshot(app_data_dir);
    let profiles = snapshot
        .profiles
        .into_iter()
        .map(|profile| profile.document)
        .collect::<Vec<_>>();
    crate::source_profile::detection::detect_source_proposal_with_clients(
        url,
        &profiles,
        http_client,
        browser_client,
    )
    .await
}

#[tauri::command]
pub fn create_source(
    state: State<'_, AppState>,
    document: crate::source::documents::SourceDocument,
) -> Result<crate::source_profile::registry::RegistrySource, String> {
    let snapshot = load_source_profile_registry_snapshot(&state.paths.app_data_dir);

    if snapshot.source(&document.key).is_some() {
        return Err(format!(
            "Eine Source mit dem Key `{}` existiert bereits.",
            document.key
        ));
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

    write_source_document(&path, &document)?;

    let snapshot = load_source_profile_registry_snapshot(&state.paths.app_data_dir);
    snapshot.source(&document.key).cloned().ok_or_else(|| {
        format!(
            "Source `{}` wurde nach dem Schreiben nicht gefunden.",
            document.key
        )
    })
}

#[tauri::command]
pub fn update_source(
    state: State<'_, AppState>,
    document: crate::source::documents::SourceDocument,
) -> Result<crate::source_profile::registry::RegistrySource, String> {
    update_source_document(
        &state.paths.app_data_dir,
        &state.paths.sources_dir,
        document,
    )
}

fn update_source_document(
    app_data_dir: &Path,
    sources_dir: &Path,
    document: crate::source::documents::SourceDocument,
) -> Result<crate::source_profile::registry::RegistrySource, String> {
    let snapshot = load_source_profile_registry_snapshot(app_data_dir);
    let existing = snapshot
        .source(&document.key)
        .ok_or_else(|| format!("Source `{}` wurde nicht gefunden.", document.key))?;

    validate_source_update_target(existing)?;

    fs::create_dir_all(sources_dir)
        .map_err(|error| format!("Sources-Ordner konnte nicht angelegt werden: {error}"))?;
    let path = source_update_path(sources_dir, &document)?;
    write_source_document(&path, &document)?;

    let snapshot = load_source_profile_registry_snapshot(app_data_dir);
    snapshot.source(&document.key).cloned().ok_or_else(|| {
        format!(
            "Source `{}` wurde nach dem Schreiben nicht gefunden.",
            document.key
        )
    })
}

fn validate_source_update_target(
    existing: &crate::source_profile::registry::RegistrySource,
) -> Result<(), String> {
    if existing.origin != "custom" {
        return Err(format!(
            "Source `{}` ist eingebaut und kann nicht überschrieben werden.",
            existing.document.key
        ));
    }
    Ok(())
}

fn source_update_path(
    sources_dir: &Path,
    document: &crate::source::documents::SourceDocument,
) -> Result<std::path::PathBuf, String> {
    let path = sources_dir.join(format!("{}.json", document.key));
    if !path.exists() {
        return Err(format!(
            "Die Datei `{}` wurde nicht gefunden.",
            path.display()
        ));
    }
    Ok(path)
}

fn write_source_document(
    path: &Path,
    document: &crate::source::documents::SourceDocument,
) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(document)
        .map_err(|error| format!("Source konnte nicht serialisiert werden: {error}"))?;
    fs::write(path, format!("{contents}\n"))
        .map_err(|error| format!("Source konnte nicht geschrieben werden: {error}"))
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
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    schedule_search_request_run(&state, id)
}

fn schedule_search_request_run(
    state: &AppState,
    id: i64,
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    let pool = state.db.clone();
    let running_search_runs = state.running_search_runs.clone();
    let browser_runtime_dir = state.paths.browser_runtime_dir.clone();
    let app_data_dir = state.paths.app_data_dir.clone();
    let geo_db_path = state.resources.geo_db_path.clone();

    state.background_tasks.schedule(
        crate::background_tasks::BackgroundTaskSpec::search_run(),
        move |context| async move {
            let _ = context.progress.report("running Search Run", None, None);
            let source_executor =
                crate::search::run::DefaultSourceExecutor::new(browser_runtime_dir);
            let result = match crate::geo::GeoDbResolver::connect(&geo_db_path).await {
                Ok(geo_resolver) => {
                    crate::search::run::SearchRunService::new_with_result_artifact(
                        &pool,
                        running_search_runs.as_ref(),
                        &source_executor,
                        crate::search::run::default_search_run_result_artifact(),
                        app_data_dir,
                    )
                    .with_geo_resolver(&geo_resolver)
                    .run_with_cancellation(id, Some(&context.cancellation_token))
                    .await
                }
                Err(error) => Err(error),
            };

            match result {
                Ok(result)
                    if result.status == crate::search::run::SearchRunStatus::Cancelled
                        || context.cancellation_token.is_cancelled() =>
                {
                    crate::background_tasks::BackgroundTaskCompletion::Cancelled {
                        error: Some("Search Run cancelled".to_string()),
                        result: serde_json::to_value(result).ok(),
                        diagnostics: Vec::new(),
                    }
                }
                Ok(result) => crate::background_tasks::BackgroundTaskCompletion::Succeeded {
                    result: serde_json::to_value(result).unwrap_or_else(
                        |error| serde_json::json!({ "serializationError": error.to_string() }),
                    ),
                },
                Err(error) => crate::background_tasks::BackgroundTaskCompletion::Failed {
                    diagnostics: vec![background_task_error_diagnostic(
                        "search_run_task_failed",
                        &error,
                    )],
                    error,
                },
            }
        },
    )
}

#[tauri::command]
pub fn get_background_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    state.background_tasks.get(&task_id)
}

#[tauri::command]
pub fn cancel_background_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    state.background_tasks.cancel(&task_id)
}

fn background_task_error_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
) -> crate::profile_dsl::diagnostics::Diagnostic {
    let message = message.into();
    crate::profile_dsl::diagnostics::Diagnostic {
        category: crate::profile_dsl::diagnostics::DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.clone(),
        severity: crate::profile_dsl::diagnostics::DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({ "message": message })),
    }
}

#[tauri::command]
pub async fn list_job_postings(
    state: State<'_, AppState>,
) -> Result<Vec<crate::search::posting::JobPosting>, String> {
    crate::search::posting::JobPostingService::new(&state.db)
        .list()
        .await
}

#[tauri::command]
pub async fn list_job_postings_for_queue(
    state: State<'_, AppState>,
    queue_id: crate::search::posting::JobPostingQueueId,
) -> Result<Vec<crate::search::posting::JobPosting>, String> {
    crate::search::posting::JobPostingService::new(&state.db)
        .list_for_queue(queue_id)
        .await
}

#[tauri::command]
pub async fn get_job_posting(
    state: State<'_, AppState>,
    posting_id: i64,
) -> Result<crate::search::posting::JobPostingView, String> {
    crate::search::posting::JobPostingService::new(&state.db)
        .get_job_posting(
            posting_id,
            &state.paths.app_data_dir,
            state.paths.browser_runtime_dir.clone(),
        )
        .await
}

#[tauri::command]
pub async fn get_job_posting_queue_counts(
    state: State<'_, AppState>,
) -> Result<crate::search::posting::JobPostingQueueCounts, String> {
    crate::search::posting::JobPostingService::new(&state.db)
        .queue_counts()
        .await
}

#[tauri::command]
pub async fn update_job_posting_state(
    state: State<'_, AppState>,
    id: i64,
    input: crate::search::posting::UpdateJobPostingStateInput,
) -> Result<crate::search::posting::JobPosting, String> {
    crate::search::posting::JobPostingService::new(&state.db)
        .update_state(id, input)
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
        window_drag_region_enabled: read_setting_or_default_value(
            pool,
            SETTING_WINDOW_DRAG_REGION_ENABLED,
            true,
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
                crate::source_profile::registry::load_snapshot(&state.paths.app_data_dir);
            assert!(registry_snapshot.profile("greenhouse").is_some());
            assert!(registry_snapshot.profile("workday").is_some());
            assert!(
                registry_snapshot.diagnostics.is_empty(),
                "built-in registry diagnostics: {:#?}",
                registry_snapshot.diagnostics
            );
        });
    }

    #[test]
    fn source_profile_registry_commands_read_current_registry_snapshot() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();

            let snapshot = load_source_profile_registry_snapshot(&state.paths.app_data_dir);

            assert!(snapshot
                .profiles
                .iter()
                .any(|profile| profile.document.key == "greenhouse"));
            assert!(snapshot
                .profiles
                .iter()
                .any(|profile| profile.document.key == "workday"));
            assert!(
                snapshot.diagnostics.is_empty(),
                "built-in registry diagnostics: {:#?}",
                snapshot.diagnostics
            );
        });
    }

    #[test]
    fn update_source_document_overwrites_custom_source_registry_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let paths =
            crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf()).unwrap();
        fs::create_dir_all(&paths.sources_dir).unwrap();

        let initial_source = command_test_source_document(
            "acme",
            "ACME",
            crate::source::documents::SourceStatus::Active,
            "acme",
        );
        let source_path = paths.sources_dir.join("acme.json");
        write_source_document(&source_path, &initial_source).unwrap();

        let updated_source = command_test_source_document(
            "acme",
            "ACME Updated",
            crate::source::documents::SourceStatus::Disabled,
            "acme-updated",
        );
        let updated_registry_source =
            update_source_document(&paths.app_data_dir, &paths.sources_dir, updated_source)
                .unwrap();

        assert_eq!(updated_registry_source.origin, "custom");
        assert_eq!(updated_registry_source.document.name, "ACME Updated");
        assert_eq!(
            updated_registry_source.document.status,
            crate::source::documents::SourceStatus::Disabled
        );
        assert_eq!(
            updated_registry_source.document.source_config["boardSlug"],
            serde_json::json!("acme-updated")
        );

        let persisted: crate::source::documents::SourceDocument =
            serde_json::from_str(&fs::read_to_string(source_path).unwrap()).unwrap();
        assert_eq!(persisted.name, "ACME Updated");
        assert_eq!(persisted.source_config["boardSlug"], "acme-updated");
    }

    #[test]
    fn update_source_document_rejects_builtin_source_target() {
        let source = command_test_source_document(
            "builtin_acme",
            "Built-in ACME",
            crate::source::documents::SourceStatus::Active,
            "builtin-acme",
        );
        let registry_source = crate::source_profile::registry::RegistrySource {
            origin: "built_in".to_string(),
            path: "resources/sources/builtin_acme.json".to_string(),
            document: source,
            validation_state: crate::source::validation::SourceValidationState {
                source_key: "builtin_acme".to_string(),
                state: crate::source::validation::ValidationStateKind::Valid,
                can_compile: true,
                can_execute: true,
                diagnostics: Vec::new(),
            },
            effective_profile: None,
            compile_outcome: None,
        };

        let error = validate_source_update_target(&registry_source).unwrap_err();
        assert!(error.contains("ist eingebaut und kann nicht überschrieben werden"));
    }

    fn command_test_source_document(
        key: &str,
        name: &str,
        status: crate::source::documents::SourceStatus,
        board_slug: &str,
    ) -> crate::source::documents::SourceDocument {
        let mut source_config = serde_json::Map::new();
        source_config.insert("boardSlug".to_string(), serde_json::json!(board_slug));

        crate::source::documents::SourceDocument {
            schema_version: 3,
            key: key.to_string(),
            name: name.to_string(),
            status,
            source_config,
            selected_access_path: crate::source::documents::SelectedAccessPath::ProfileAccessPath {
                profile_key: "greenhouse".to_string(),
                path_key: "boards_api".to_string(),
            },
            access_paths: Some(
                serde_json::from_value(serde_json::json!([{
                    "key": "boards_api",
                    "discovery": {
                        "strategies": [{
                            "key": "jobs_api",
                            "acceptWhen": { "minResults": 0 }
                        }]
                    }
                }]))
                .expect("command test direct specialization must deserialize"),
            ),
            source_support: None,
            diagnostics: None,
        }
    }

    #[test]
    fn source_proposal_command_seam_returns_actionable_proposal_without_adapter_key() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();
            let browser_client = crate::profile_dsl::runtime::UnavailableProfileBrowserClient;

            let result = detect_source_proposal_from_url_with_clients(
                &state.paths.app_data_dir,
                "https://boards.greenhouse.io/acme_corp",
                &crate::source_profile::detection::NoopDetectionHttpClient,
                &browser_client,
            )
            .await;

            assert_eq!(
                result.status,
                crate::source_profile::detection::SourceProposalDetectionStatus::Matched
            );
            let proposal = result.proposal.expect("matched detection returns proposal");
            assert_eq!(proposal.profile_key, "greenhouse");
            assert_eq!(proposal.recommended_access_path_key, "boards_api");
            assert_eq!(proposal.source_config["boardSlug"], "acme_corp");

            let serialized = serde_json::to_value(&proposal).unwrap();
            assert_no_adapter_key(&serialized);
        });
    }

    #[test]
    fn source_proposal_command_seam_executes_injected_browser_probe() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            std::fs::create_dir_all(&paths.source_profiles_dir).unwrap();
            std::fs::write(
                paths.source_profiles_dir.join("browser_jobs.json"),
                serde_json::to_string_pretty(&serde_json::json!({
                    "schemaVersion": 3,
                    "key": "browser_jobs",
                    "name": "Browser Jobs",
                    "kind": "generic",
                    "support": { "level": "experimental" },
                    "detection": {
                        "recommendedAccessPathKey": "rendered",
                        "inputUrlPatterns": [{
                            "pattern": "^https://careers\\.example\\.test/(?<tenant>[a-z0-9_-]+)$"
                        }],
                        "browserProbes": [{
                            "key": "rendered_page",
                            "url": "{{inputUrl}}",
                            "timeoutMs": 5000,
                            "htmlContains": "BrowserJobs",
                            "htmlRegex": "company=\\\"(?<organizationName>[^\\\"]+)\\\"",
                            "evidence": "Rendered career page identified BrowserJobs."
                        }],
                        "sourceConfig": {
                            "tenant": "{{capture:tenant}}",
                            "startUrl": "{{inputUrl}}"
                        },
                        "keyCandidates": ["{{capture:tenant}}"],
                        "nameCandidates": ["{{capture:organizationName}}"]
                    },
                    "sourceConfigSchema": {
                        "type": "object",
                        "required": ["tenant", "startUrl"],
                        "additionalProperties": false,
                        "properties": {
                            "tenant": { "type": "string" },
                            "startUrl": { "type": "string" }
                        }
                    },
                    "accessPaths": [{
                        "key": "rendered",
                        "name": "Rendered page",
                        "discovery": {
                            "policy": { "type": "first_accepted" },
                            "strategies": [{
                                "key": "jobs_html",
                                "fetch": {
                                    "mode": "http",
                                    "method": "GET",
                                    "url": "https://example.test/jobs",
                                    "timeoutMs": 10000
                                },
                                "parse": { "type": "html" },
                                "select": { "type": "css", "selector": ".job" },
                                "extract": {
                                    "reference": {
                                        "url": { "type": "css_attribute", "selector": "a", "attribute": "href", "cardinality": "one" }
                                    },
                                    "providerValues": {
                                        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
                                        "company": { "type": "const", "value": "Example" }
                                    }
                                }
                            }]
                        }
                    }]
                }))
                .unwrap(),
            )
            .unwrap();
            let state = AppState::new(paths).await.unwrap();
            let browser_client = StaticBrowserClient {
                body: "<html>BrowserJobs company=\"ACME GmbH\"</html>".to_string(),
            };

            let result = detect_source_proposal_from_url_with_clients(
                &state.paths.app_data_dir,
                "https://careers.example.test/acme",
                &crate::source_profile::detection::NoopDetectionHttpClient,
                &browser_client,
            )
            .await;

            assert_eq!(
                result.status,
                crate::source_profile::detection::SourceProposalDetectionStatus::Matched
            );
            let proposal = result.proposal.expect("browser detection returns proposal");
            assert_eq!(proposal.profile_key, "browser_jobs");
            assert_eq!(proposal.name_candidates, vec!["ACME GmbH"]);
            assert!(proposal.evidence.iter().any(|evidence| {
                evidence.probe_key.as_deref() == Some("rendered_page")
                    && evidence.message == "Rendered career page identified BrowserJobs."
            }));
        });
    }

    struct StaticBrowserClient {
        body: String,
    }

    impl crate::profile_dsl::runtime::ProfileBrowserClient for StaticBrowserClient {
        fn render<'a>(
            &'a self,
            _request: crate::profile_dsl::runtime::ProfileBrowserFetchRequest,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            crate::profile_dsl::runtime::ProfileBrowserFetchResponse,
                            crate::profile_dsl::runtime::ProfileBrowserFetchError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                Ok(crate::profile_dsl::runtime::ProfileBrowserFetchResponse {
                    body: self.body.clone(),
                })
            })
        }
    }

    fn assert_no_adapter_key(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                assert!(
                    !map.contains_key("adapterKey"),
                    "serialized value contains adapterKey: {value}"
                );
                for nested in map.values() {
                    assert_no_adapter_key(nested);
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values {
                    assert_no_adapter_key(nested);
                }
            }
            _ => {}
        }
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
            assert!(preferences.window_drag_region_enabled);

            write_setting(&state.db, SETTING_DEFAULT_SEARCH_RADIUS_KM, &50_u16)
                .await
                .unwrap();
            write_setting(&state.db, SETTING_BASE_FONT_SIZE_PX, &18_u16)
                .await
                .unwrap();
            write_setting(&state.db, SETTING_WINDOW_DRAG_REGION_ENABLED, &false)
                .await
                .unwrap();
            let preferences = read_app_preferences(&state.db).await.unwrap();
            assert_eq!(preferences.default_search_radius_km, 50);
            assert_eq!(preferences.base_font_size_px, 18);
            assert!(!preferences.window_drag_region_enabled);
            assert!(validate_search_radius(MAX_SEARCH_RADIUS_KM + 1).is_err());
            assert!(validate_base_font_size(MIN_BASE_FONT_SIZE_PX - 1).is_err());
            assert!(validate_base_font_size(MAX_BASE_FONT_SIZE_PX + 1).is_err());
        });
    }

    #[test]
    fn run_search_request_command_seam_returns_queued_background_task_when_search_run_is_active() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();
            let (release_active, active_released) = tokio::sync::oneshot::channel::<()>();

            let active = state
                .background_tasks
                .schedule(
                    crate::background_tasks::BackgroundTaskSpec::search_run(),
                    move |_context| async move {
                        let _ = active_released.await;
                        crate::background_tasks::BackgroundTaskCompletion::Succeeded {
                            result: serde_json::json!({ "done": true }),
                        }
                    },
                )
                .unwrap();
            assert_eq!(
                active.state,
                crate::background_tasks::BackgroundTaskState::Running
            );

            let queued = schedule_search_request_run(&state, 123).unwrap();

            assert_eq!(
                queued.kind,
                crate::background_tasks::BackgroundTaskKind::SearchRun
            );
            assert_eq!(
                queued.state,
                crate::background_tasks::BackgroundTaskState::Queued
            );
            let cancelled = state.background_tasks.cancel(&queued.task_id).unwrap();
            assert_eq!(
                cancelled.state,
                crate::background_tasks::BackgroundTaskState::Cancelled
            );
            release_active.send(()).unwrap();
        });
    }

    #[test]
    fn run_search_request_task_uses_geo_database_resource() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let missing_geo_db = temp_dir.path().join("missing-geo.sqlite");
            let state = AppState::new_with_resources_and_background_task_notifier(
                paths,
                crate::app::resources::AppResources::from_geo_db_path(missing_geo_db),
                std::sync::Arc::new(crate::background_tasks::NoopBackgroundTaskNotifier),
            )
            .await
            .unwrap();

            let task = schedule_search_request_run(&state, 123).unwrap();
            let finished = wait_for_background_task_state(
                &state.background_tasks,
                &task.task_id,
                crate::background_tasks::BackgroundTaskState::Failed,
            )
            .await;

            assert!(
                finished
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("failed to open geo database")),
                "expected geo database failure, got {finished:#?}"
            );
        });
    }

    async fn wait_for_background_task_state(
        scheduler: &crate::background_tasks::BackgroundTaskScheduler,
        task_id: &str,
        state: crate::background_tasks::BackgroundTaskState,
    ) -> crate::background_tasks::BackgroundTaskSnapshot {
        for _ in 0..100 {
            let snapshot = scheduler.get(task_id).unwrap();
            if snapshot.state == state {
                return snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task {task_id} did not reach state {state:?}");
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
