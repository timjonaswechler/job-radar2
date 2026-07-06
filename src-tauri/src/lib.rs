mod app;
mod background_tasks;
mod browser_runtime;
mod db;
mod geo;
mod profile_dsl;
mod search;
mod simple_json_path;
mod source;
mod source_profile;

pub use geo::{
    distance_km, matches_location_filter, prepare_location_filter, GeoDbResolver, GeoPoint,
    LocationFilterNotAppliedReason, LocationMatchOutcome, PreparedLocationFilter, ResolvedLocation,
};
pub use profile_dsl::compiler::{
    compile_source_execution_plan, CompileSourceExecutionPlanResult, ProfileCompilerSnapshot,
};
pub use profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
pub use profile_dsl::documents::{
    Fetch, FieldExpression, HttpMethod, RequestBody, Select, SupportLevel,
};
pub use profile_dsl::execution_plan::capabilities::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ExecutionPlanFetch,
    ExecutionPlanPagination,
};
pub use profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
pub use profile_dsl::runtime::{
    execute_posting_detail, execute_posting_detail_with_clients,
    execute_posting_detail_with_fetcher, execute_posting_discovery,
    execute_posting_discovery_with_clients, execute_posting_discovery_with_fetcher,
    ManagedProfileBrowserClient, PostingDetailExecutionResult, PostingDetailFetchError,
    PostingDetailFetchRequest, PostingDetailFetchResponse, PostingDetailFetcher,
    PostingDetailPostingOccurrence, PostingDiscoveryCandidate, PostingDiscoveryExecutionResult,
    PostingDiscoveryFetchError, PostingDiscoveryFetchRequest, PostingDiscoveryFetchResponse,
    PostingDiscoveryFetcher, ProfileBrowserClient, ProfileBrowserFetchError,
    ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    ReqwestPostingDetailFetcher, ReqwestPostingDiscoveryFetcher, UnavailableProfileBrowserClient,
};
pub use search::smoke::run_dev_search_run_smoke_cli;
pub use source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
pub use source::validation::{SourceValidationState, ValidationStateKind};
pub use source_profile::detection::{
    detect_source_proposal, detect_source_proposal_with_clients,
    detect_source_proposal_with_http_client, DetectionHttpClient, DetectionHttpError,
    DetectionHttpResponse, ReqwestDetectionHttpClient, SourceProposal,
    SourceProposalDetectionResult, SourceProposalDetectionStatus, SourceProposalEvidence,
    UnsupportedSourceProfile,
};
pub use source_profile::documents::SourceProfileDocument;
pub use source_profile::registry::{
    load_snapshot as load_source_profile_registry_snapshot, RegistrySource, RegistrySourceProfile,
    SourceProfileRegistrySnapshot,
};

use tauri::{Emitter, Manager};

struct TauriBackgroundTaskNotifier {
    app: tauri::AppHandle,
}

impl background_tasks::BackgroundTaskNotifier for TauriBackgroundTaskNotifier {
    fn task_updated(&self, snapshot: &background_tasks::BackgroundTaskSnapshot) {
        let _ = self
            .app
            .emit(background_tasks::BACKGROUND_TASK_UPDATED_EVENT, snapshot);
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = app::paths::AppPaths::from_app(app.handle())?;
            let resources = app::resources::AppResources::from_app(app.handle())?;
            let notifier = std::sync::Arc::new(TauriBackgroundTaskNotifier {
                app: app.handle().clone(),
            });
            let app_state = tauri::async_runtime::block_on(
                app::state::AppState::new_with_resources_and_background_task_notifier(
                    paths, resources, notifier,
                ),
            )?;
            let database_path = app_state.paths.database_path.clone();
            let geo_db_path = app_state.resources.geo_db_path.clone();

            app.manage(app_state);
            println!("SQLite database: {}", database_path.display());
            println!("Geo database: {}", geo_db_path.display());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            app::commands::get_database_info,
            app::commands::get_app_preferences,
            app::commands::set_app_preferences,
            app::commands::set_app_theme,
            app::commands::set_app_language,
            app::commands::set_default_search_radius_km,
            app::commands::set_base_font_size_px,
            app::commands::get_browser_runtime_status,
            app::commands::install_browser_runtime,
            app::commands::uninstall_browser_runtime,
            app::commands::check_browser_runtime,
            app::commands::get_source_profile_registry_snapshot,
            app::commands::list_source_profiles,
            app::commands::list_sources,
            app::commands::list_source_diagnostics,
            app::commands::detect_source_proposal_from_url,
            app::commands::create_source,
            app::commands::create_search_request,
            app::commands::list_search_requests,
            app::commands::get_search_request,
            app::commands::update_search_request,
            app::commands::delete_search_request,
            app::commands::run_search_request,
            app::commands::get_background_task,
            app::commands::cancel_background_task,
            app::commands::list_job_postings,
            app::commands::list_job_postings_for_queue,
            app::commands::get_posting_detail,
            app::commands::get_job_posting_queue_counts,
            app::commands::update_job_posting_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
