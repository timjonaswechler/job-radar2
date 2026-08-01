mod adapters;
pub mod agent;
mod app;
mod background_tasks;
mod browser_runtime;
mod db;
mod geo;
mod search;

pub use crate::geo::GeoDbResolver;
pub use ::geo::{
    distance_km, matches_location_filter, prepare_location_filter, GeoPoint, GeoResolveFuture,
    GeoResolver, LocationFilterMatchReport, LocationFilterNotAppliedReason, LocationMatchOutcome,
    LocationResolutionAmbiguity, PreparedLocationFilter, ResolvedLocation,
};
pub use browser_runtime::ManagedBrowserAcquisition;
pub use search::smoke::run_dev_search_run_smoke_cli;
pub use search_resolution::{
    resolve_source_candidates, CandidateDiagnosticSummary, CompiledSearchRequirements,
    FinalizedCandidate, RequirementsCompilationFailure, ResolutionCeilings, ResolutionCompletion,
    ResolutionCounts, ResolutionFailure, ResolutionLimitDimension, ResolutionReport,
    ScriptedDiscoveryBatch, ScriptedDiscoveryOutcome, ScriptedSourceDiscoveryExecution, SearchRule,
    SearchRuleKind, SearchRuleTarget, SourceDiscovery, SourceResolution, SourceResolutionError,
    SourceResolutionRequest, CANDIDATE_DIAGNOSTIC_SAMPLE_LIMIT,
};
pub use sources::installed::{
    CreateDraft, InactiveStatus, Revision, SourceDocument, SourceStatus, SourceView,
    Store as InstalledSourceStore,
};

use tauri::{Emitter, Manager};

pub fn current_user_agents_data_root() -> std::io::Result<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return app::paths::current_user_app_data_location()
            .map(|location| location.root.join("agents"));
    }

    #[cfg(not(target_os = "macos"))]
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

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
            app::commands::get_database_info,
            app::commands::get_app_preferences,
            app::commands::set_app_preferences,
            app::commands::set_app_theme,
            app::commands::set_app_language,
            app::commands::set_default_search_radius_km,
            app::commands::set_base_font_size_px,
            app::commands::set_window_drag_region_enabled,
            app::commands::get_agent_configuration_status,
            app::commands::submit_agent_api_key,
            app::commands::create_agent_chat,
            app::commands::open_agent_chat,
            app::commands::send_agent_chat_message,
            app::commands::stop_agent_chat,
            app::commands::set_agent_chat_model,
            app::commands::set_agent_chat_reasoning_level,
            app::commands::compact_agent_chat,
            app::commands::login_agent_subscription,
            app::commands::cancel_agent_subscription_login,
            app::commands::remove_agent_authentication,
            app::commands::reload_agent_configuration,
            app::commands::open_agent_data_folder,
            app::commands::get_browser_runtime_status,
            app::commands::install_browser_runtime,
            app::commands::uninstall_browser_runtime,
            app::commands::check_browser_runtime,
            app::commands::get_source_inventory,
            app::commands::check_source,
            app::commands::check_and_activate_source,
            app::commands::get_source_live_check_report_status,
            app::commands::detect_source_proposal_from_url,
            app::commands::create_source,
            app::commands::update_source,
            app::commands::set_source_inactive,
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
            app::commands::get_job_posting,
            app::commands::get_job_posting_queue_counts,
            app::commands::update_job_posting_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
