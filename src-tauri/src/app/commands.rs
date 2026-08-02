use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
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
pub const AGENT_CHAT_EVENT: &str = "agent-chat-event";

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

struct TauriAgentChatEventListener {
    app: AppHandle,
}

impl crate::agent::chat_application::AgentChatEventListener for TauriAgentChatEventListener {
    fn emit(&self, event: crate::agent::chat_application::AgentChatApplicationEvent) {
        let _ = self.app.emit(AGENT_CHAT_EVENT, event);
    }
}

#[tauri::command]
pub fn create_agent_chat(
    state: State<'_, AppState>,
    input: crate::agent::chat_application::AgentChatCreateInput,
) -> Result<
    crate::agent::chat_application::AgentChatProjection,
    crate::agent::chat_application::AgentChatApplicationError,
> {
    state.agent_chats.create(input)
}

#[tauri::command]
pub async fn open_agent_chat(
    state: State<'_, AppState>,
    input: crate::agent::chat_application::AgentChatOpenInput,
) -> Result<
    crate::agent::chat_application::AgentChatProjection,
    crate::agent::chat_application::AgentChatApplicationError,
> {
    state.agent_chats.open(input).await
}

#[tauri::command]
pub fn send_agent_chat_message(
    app: AppHandle,
    state: State<'_, AppState>,
    chat_id: crate::agent::chat_application::AgentChatId,
    text: String,
) -> Result<(), crate::agent::chat_application::AgentChatApplicationError> {
    state.agent_chats.send(
        chat_id,
        text,
        std::sync::Arc::new(TauriAgentChatEventListener { app }),
    )
}

#[tauri::command]
pub fn stop_agent_chat(
    state: State<'_, AppState>,
    chat_id: crate::agent::chat_application::AgentChatId,
) -> bool {
    state.agent_chats.stop(&chat_id)
}

#[tauri::command]
pub async fn set_agent_chat_model(
    state: State<'_, AppState>,
    chat_id: crate::agent::chat_application::AgentChatId,
    provider_id: String,
    model_id: String,
) -> Result<
    crate::agent::chat_application::AgentChatProjection,
    crate::agent::chat_application::AgentChatApplicationError,
> {
    state
        .agent_chats
        .select_model(&chat_id, provider_id, model_id)
        .await
}

#[tauri::command]
pub async fn set_agent_chat_reasoning_level(
    state: State<'_, AppState>,
    chat_id: crate::agent::chat_application::AgentChatId,
    reasoning_level: crate::agent::chat_application::ApplicationReasoningLevel,
) -> Result<
    crate::agent::chat_application::AgentChatProjection,
    crate::agent::chat_application::AgentChatApplicationError,
> {
    state
        .agent_chats
        .set_reasoning_level(&chat_id, reasoning_level)
        .await
}

#[tauri::command]
pub fn compact_agent_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    chat_id: crate::agent::chat_application::AgentChatId,
    focus: Option<String>,
) -> Result<(), crate::agent::chat_application::AgentChatApplicationError> {
    state.agent_chats.compact(
        chat_id,
        focus,
        std::sync::Arc::new(TauriAgentChatEventListener { app }),
    )
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

#[tauri::command]
pub async fn get_source_inventory(
    state: State<'_, AppState>,
) -> Result<sources::installed::SnapshotView, String> {
    let installed_sources = state.installed_sources.clone();
    tokio::task::spawn_blocking(move || installed_sources.snapshot())
        .await
        .map_err(|error| error.to_string())?
        .map(|snapshot| snapshot.view().clone())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn check_source(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<sources::live_check::RunOutcome, sources::live_check::Error> {
    state
        .source_live_check
        .run(&source_key, sources::live_check::Context::default())
        .await
}

#[tauri::command]
pub async fn check_and_activate_source(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<sources::live_check::AdmissionOutcome, sources::live_check::Error> {
    state
        .source_live_check
        .check_and_activate(&source_key, sources::live_check::Context::default())
        .await
}

#[tauri::command]
pub async fn get_source_live_check_report_status(
    state: State<'_, AppState>,
    source_key: String,
) -> Result<sources::live_check::Status, sources::live_check::Error> {
    state.source_live_check.status(&source_key).await
}

#[tauri::command]
pub async fn detect_source_proposal_from_url(
    state: State<'_, AppState>,
    url: String,
) -> Result<sources::detection::Outcome, sources::detection::Error> {
    state
        .source_detection
        .run(
            sources::detection::Request { url },
            sources::detection::Context::default(),
        )
        .await
}

#[tauri::command]
pub async fn create_source(
    state: State<'_, AppState>,
    draft: sources::installed::CreateDraft,
) -> Result<sources::installed::SourceView, sources::installed::Error> {
    let installed = state.installed_sources.clone();
    tokio::task::spawn_blocking(move || installed.create(draft))
        .await
        .expect("installed Source creation task must run to completion")
}

#[tauri::command]
pub async fn update_source(
    state: State<'_, AppState>,
    revision: sources::installed::Revision,
) -> Result<sources::installed::SourceView, sources::installed::Error> {
    let installed = state.installed_sources.clone();
    tokio::task::spawn_blocking(move || installed.revise(revision))
        .await
        .expect("installed Source revision task must run to completion")
}

#[tauri::command]
pub async fn set_source_inactive(
    state: State<'_, AppState>,
    source_key: String,
    status: sources::installed::InactiveStatus,
) -> Result<sources::installed::SourceView, sources::installed::Error> {
    let installed = state.installed_sources.clone();
    tokio::task::spawn_blocking(move || installed.set_inactive(&source_key, status))
        .await
        .expect("installed Source lifecycle task must run to completion")
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SearchRequestErrorKind {
    InvalidInput,
    NotFound,
    Busy,
    CorruptStoredRow,
    StorageUnavailable,
    InternalInvariant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequestCommandError {
    kind: SearchRequestErrorKind,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
}

impl SearchRequestCommandError {
    fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            kind: SearchRequestErrorKind::InvalidInput,
            message: message.into(),
            id: None,
        }
    }

    fn storage_unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: SearchRequestErrorKind::StorageUnavailable,
            message: message.into(),
            id: None,
        }
    }
}

impl From<search_requests::Error> for SearchRequestCommandError {
    fn from(error: search_requests::Error) -> Self {
        let (kind, id) = match &error {
            search_requests::Error::InvalidInput { .. } => {
                (SearchRequestErrorKind::InvalidInput, None)
            }
            search_requests::Error::NotFound { id } => {
                (SearchRequestErrorKind::NotFound, Some(id.get()))
            }
            search_requests::Error::Busy { id } => (SearchRequestErrorKind::Busy, Some(id.get())),
            search_requests::Error::CorruptStoredRow { id, .. } => (
                SearchRequestErrorKind::CorruptStoredRow,
                id.map(search_requests::Id::get),
            ),
            search_requests::Error::StorageUnavailable { .. } => {
                (SearchRequestErrorKind::StorageUnavailable, None)
            }
            search_requests::Error::InternalInvariant { .. } => {
                (SearchRequestErrorKind::InternalInvariant, None)
            }
        };
        Self {
            kind,
            message: error.to_string(),
            id,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequestInput {
    status: search_requests::Status,
    include_rules: Vec<SearchRuleInput>,
    exclude_rules: Vec<SearchRuleInput>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_keys: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchRuleInput {
    target: String,
    kind: String,
    value: String,
}

impl TryFrom<SearchRequestInput> for search_requests::Input {
    type Error = String;
    fn try_from(input: SearchRequestInput) -> Result<Self, Self::Error> {
        fn rules(
            values: Vec<SearchRuleInput>,
            field: &str,
        ) -> Result<Vec<search_resolution::SearchRule>, String> {
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    Ok(search_resolution::SearchRule {
                        target: search_resolution::SearchRuleTarget::try_from(
                            value.target.as_str(),
                        )
                        .map_err(|error| format!("{field}[{index}].target {error}"))?,
                        kind: search_resolution::SearchRuleKind::try_from(value.kind.as_str())
                            .map_err(|error| format!("{field}[{index}].kind {error}"))?,
                        value: value.value,
                    })
                })
                .collect()
        }
        Ok(Self {
            status: input.status,
            include_rules: rules(input.include_rules, "includeRules")?,
            exclude_rules: rules(input.exclude_rules, "excludeRules")?,
            locations: input.locations,
            radius_km: input.radius_km,
            source_keys: input.source_keys,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequestView {
    id: i64,
    status: search_requests::Status,
    include_rules: Vec<search_resolution::SearchRule>,
    exclude_rules: Vec<search_resolution::SearchRule>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_keys: Vec<String>,
    validation_issues: Vec<search_requests::ValidationIssue>,
    last_run_at: Option<String>,
    last_run_status: Option<crate::search::run::SearchRunStatus>,
    last_run_error: Option<String>,
    created_at: String,
    updated_at: String,
}

fn search_request_view(
    record: search_requests::Record,
    latest: crate::search::run::LatestSummary,
) -> SearchRequestView {
    SearchRequestView {
        id: record.id.get(),
        status: record.status,
        include_rules: record.include_rules,
        exclude_rules: record.exclude_rules,
        locations: record.locations,
        radius_km: record.radius_km,
        source_keys: record.source_keys,
        validation_issues: record.validation.issues,
        last_run_at: latest.at,
        last_run_status: latest.status,
        last_run_error: latest.error,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn search_request_views(
    records: Vec<search_requests::Record>,
    mut latest: std::collections::HashMap<search_requests::Id, crate::search::run::LatestSummary>,
) -> Vec<SearchRequestView> {
    records
        .into_iter()
        .map(|record| {
            let summary = latest.remove(&record.id).unwrap_or_default();
            search_request_view(record, summary)
        })
        .collect()
}

#[tauri::command]
pub async fn create_search_request(
    state: State<'_, AppState>,
    input: SearchRequestInput,
) -> Result<SearchRequestView, SearchRequestCommandError> {
    let input = input
        .try_into()
        .map_err(SearchRequestCommandError::invalid_input)?;
    let record = state
        .search_requests
        .create(input)
        .await
        .map_err(SearchRequestCommandError::from)?;
    Ok(search_request_view(
        record,
        crate::search::run::LatestSummary::default(),
    ))
}

#[tauri::command]
pub async fn list_search_requests(
    state: State<'_, AppState>,
) -> Result<Vec<SearchRequestView>, SearchRequestCommandError> {
    let records = state
        .search_requests
        .list()
        .await
        .map_err(SearchRequestCommandError::from)?;
    let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    let latest = crate::search::run::latest_summaries(&state.db, &ids)
        .await
        .map_err(SearchRequestCommandError::storage_unavailable)?;
    Ok(search_request_views(records, latest))
}

#[tauri::command]
pub async fn get_search_request(
    state: State<'_, AppState>,
    id: i64,
) -> Result<SearchRequestView, SearchRequestCommandError> {
    let id = search_requests::Id::new(id).map_err(SearchRequestCommandError::from)?;
    let record = state
        .search_requests
        .get(id)
        .await
        .map_err(SearchRequestCommandError::from)?;
    let latest = crate::search::run::latest_summary(&state.db, id)
        .await
        .map_err(SearchRequestCommandError::storage_unavailable)?;
    Ok(search_request_view(record, latest))
}

#[tauri::command]
pub async fn update_search_request(
    state: State<'_, AppState>,
    id: i64,
    input: SearchRequestInput,
) -> Result<SearchRequestView, SearchRequestCommandError> {
    let id = search_requests::Id::new(id).map_err(SearchRequestCommandError::from)?;
    let input = input
        .try_into()
        .map_err(SearchRequestCommandError::invalid_input)?;
    let record = state
        .search_requests
        .update(id, input)
        .await
        .map_err(SearchRequestCommandError::from)?;
    let latest = crate::search::run::latest_summary(&state.db, id)
        .await
        .map_err(SearchRequestCommandError::storage_unavailable)?;
    Ok(search_request_view(record, latest))
}

#[tauri::command]
pub async fn delete_search_request(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), SearchRequestCommandError> {
    let id = search_requests::Id::new(id).map_err(SearchRequestCommandError::from)?;
    state
        .search_requests
        .delete(id)
        .await
        .map_err(SearchRequestCommandError::from)
}

#[tauri::command]
pub async fn run_search_request(
    state: State<'_, AppState>,
    id: i64,
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    schedule_search_request_run(&state, id).await
}

async fn schedule_search_request_run(
    state: &AppState,
    id: i64,
) -> Result<crate::background_tasks::BackgroundTaskSnapshot, String> {
    let id = search_requests::Id::new(id).map_err(|error| error.to_string())?;
    let execution = state
        .search_requests
        .begin_execution(id)
        .await
        .map_err(|error| error.to_string())?;
    let pool = state.db.clone();
    let browser_runtime_dir = state.paths.browser_runtime_dir.clone();
    let installed_sources = state.installed_sources.clone();
    let geo_db_path = state.resources.geo_db_path.clone();

    state.background_tasks.schedule(
        crate::background_tasks::BackgroundTaskSpec::search_run(),
        move |context| async move {
            let _ = context.progress.report("running Search Run", None, None);
            let source_resolver =
                crate::search::run::SearchRunResolutionRuntime::production(browser_runtime_dir);
            let result = match crate::geo::GeoDbResolver::connect(&geo_db_path).await {
                Ok(geo_resolver) => {
                    crate::search::run::SearchRunService::new_with_result_artifact(
                        &pool,
                        &source_resolver,
                        crate::search::run::default_search_run_result_artifact(),
                        installed_sources,
                    )
                    .with_geo_resolver(&geo_resolver)
                    .run_with_cancellation(execution, Some(&context.cancellation_token))
                    .await
                }
                Err(error) => Err(crate::search::run::SearchRunError::Requirements(error)),
            };

            match result {
                Ok(outcome) => search_run_task_completion(outcome),
                Err(error) => {
                    let error = error.to_string();
                    crate::background_tasks::BackgroundTaskCompletion::Failed {
                        diagnostics: vec![background_task_error_diagnostic(
                            "search_run_task_failed",
                            &error,
                        )],
                        error,
                    }
                }
            }
        },
    )
}

pub(crate) fn search_run_task_completion(
    outcome: crate::search::run::SearchRunOutcome,
) -> crate::background_tasks::BackgroundTaskCompletion {
    if outcome.status == crate::search::run::SearchRunStatus::Cancelled {
        crate::background_tasks::BackgroundTaskCompletion::Cancelled {
            error: Some("Search Run cancelled".to_string()),
            result: serde_json::to_value(outcome).ok(),
            diagnostics: Vec::new(),
        }
    } else {
        crate::background_tasks::BackgroundTaskCompletion::Succeeded {
            result: serde_json::to_value(outcome).unwrap_or_else(
                |error| serde_json::json!({ "serializationError": error.to_string() }),
            ),
        }
    }
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
) -> source_engine::definition::Diagnostic {
    let message = message.into();
    source_engine::definition::Diagnostic {
        category: source_engine::definition::DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.clone(),
        severity: source_engine::definition::DiagnosticSeverity::Error,
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
            &state.installed_sources,
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

            let registry_snapshot = state.installed_sources.snapshot().unwrap();
            assert!(registry_snapshot
                .view()
                .profiles
                .profiles
                .iter()
                .any(|profile| {
                    profile
                        .definition
                        .as_ref()
                        .map(|definition| definition.key.as_str())
                        == Some("greenhouse")
                }));
            assert!(registry_snapshot
                .view()
                .profiles
                .profiles
                .iter()
                .any(|profile| {
                    profile
                        .definition
                        .as_ref()
                        .map(|definition| definition.key.as_str())
                        == Some("workday")
                }));
            assert!(
                registry_snapshot.view().diagnostics.is_empty(),
                "built-in registry diagnostics: {:#?}",
                registry_snapshot.view().diagnostics
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

            let snapshot = state.installed_sources.snapshot().unwrap();

            assert!(snapshot
                .view()
                .profiles
                .profiles
                .iter()
                .any(|profile| profile
                    .definition
                    .as_ref()
                    .map(|definition| definition.key.as_str())
                    == Some("greenhouse")));
            assert!(snapshot
                .view()
                .profiles
                .profiles
                .iter()
                .any(|profile| profile
                    .definition
                    .as_ref()
                    .map(|definition| definition.key.as_str())
                    == Some("workday")));
            assert!(
                snapshot.view().diagnostics.is_empty(),
                "built-in registry diagnostics: {:#?}",
                snapshot.view().diagnostics
            );
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
    fn search_request_command_errors_preserve_catalog_distinctions() {
        let id = search_requests::Id::new(7).unwrap();
        let value = serde_json::to_value(SearchRequestCommandError::from(
            search_requests::Error::NotFound { id },
        ))
        .unwrap();

        assert_eq!(value["kind"], "not_found");
        assert_eq!(value["message"], "search request 7 not found");
        assert_eq!(value["id"], 7);
    }

    #[test]
    fn search_request_list_host_view_associates_batched_latest_run_summaries() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let paths =
                crate::app::paths::AppPaths::from_app_data_dir(temp_dir.path().to_path_buf())
                    .unwrap();
            let state = AppState::new(paths).await.unwrap();
            let completed = create_active_test_search_request(&state).await;
            let failed = create_active_test_search_request(&state).await;
            let never_run = create_active_test_search_request(&state).await;
            sqlx::query("UPDATE search_requests SET last_run_at='2026-01-01T00:00:00Z', last_run_status='completed', last_run_error=NULL WHERE id=?1")
                .bind(completed.id.get()).execute(&state.db).await.unwrap();
            sqlx::query("UPDATE search_requests SET last_run_at='2026-02-02T00:00:00Z', last_run_status='failed', last_run_error='source failed' WHERE id=?1")
                .bind(failed.id.get()).execute(&state.db).await.unwrap();

            let records = state.search_requests.list().await.unwrap();
            let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
            let latest = crate::search::run::latest_summaries(&state.db, &ids)
                .await
                .unwrap();
            let values = serde_json::to_value(search_request_views(records, latest)).unwrap();

            assert_eq!(values[0]["id"], completed.id.get());
            assert_eq!(values[0]["lastRunAt"], "2026-01-01T00:00:00Z");
            assert_eq!(values[0]["lastRunStatus"], "completed");
            assert!(values[0]["lastRunError"].is_null());
            assert_eq!(values[1]["id"], failed.id.get());
            assert_eq!(values[1]["lastRunAt"], "2026-02-02T00:00:00Z");
            assert_eq!(values[1]["lastRunStatus"], "failed");
            assert_eq!(values[1]["lastRunError"], "source failed");
            assert_eq!(values[2]["id"], never_run.id.get());
            assert!(values[2]["lastRunAt"].is_null());
            assert!(values[2]["lastRunStatus"].is_null());
            assert!(values[2]["lastRunError"].is_null());
            assert!(values[0].get("validationIssues").is_some());
            assert!(values[0].get("validation").is_none());
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
            let request = create_active_test_search_request(&state).await;
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

            let queued = schedule_search_request_run(&state, request.id.get())
                .await
                .unwrap();

            assert_eq!(
                queued.kind,
                crate::background_tasks::BackgroundTaskKind::SearchRun
            );
            assert_eq!(
                queued.state,
                crate::background_tasks::BackgroundTaskState::Queued
            );
            assert!(
                matches!(state.search_requests.delete(request.id).await, Err(search_requests::Error::Busy { id }) if id == request.id)
            );
            let cancelled = state.background_tasks.cancel(&queued.task_id).unwrap();
            assert_eq!(
                cancelled.state,
                crate::background_tasks::BackgroundTaskState::Cancelled
            );
            for table in ["search_runs", "matches", "job_postings"] {
                assert_eq!(
                    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                        .fetch_one(&state.db)
                        .await
                        .unwrap(),
                    0,
                    "queued cancellation must not write {table}"
                );
            }
            let released = state
                .search_requests
                .begin_execution(request.id)
                .await
                .unwrap();
            drop(released);
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
            let request = create_active_test_search_request(&state).await;

            let task = schedule_search_request_run(&state, request.id.get())
                .await
                .unwrap();
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

    async fn create_active_test_search_request(state: &AppState) -> search_requests::Record {
        state
            .search_requests
            .create(search_requests::Input {
                status: search_requests::Status::Active,
                include_rules: vec![search_resolution::SearchRule {
                    target: search_resolution::SearchRuleTarget::Title,
                    kind: search_resolution::SearchRuleKind::Text,
                    value: "engineer".into(),
                }],
                exclude_rules: Vec::new(),
                locations: Vec::new(),
                radius_km: None,
                source_keys: vec!["fixture_source".into()],
            })
            .await
            .unwrap()
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
