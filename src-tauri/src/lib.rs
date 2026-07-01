mod adapter_registry;
mod app;
mod browser_runtime;
mod db;
mod declarative;
mod profile_dsl;
mod search;
mod simple_json_path;
mod source;
mod source_profile;

pub use profile_dsl::compiler::{
    compile_source_execution_plan, CompileSourceExecutionPlanResult, ProfileCompilerSnapshot,
};
pub use profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
pub use profile_dsl::documents::{Fetch, FieldExpression, HttpMethod, Select, SupportLevel};
pub use profile_dsl::execution_plan::capabilities::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ExecutionPlanFetch,
    ExecutionPlanPagination,
};
pub use profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
pub use search::smoke::run_dev_search_run_smoke_cli;
pub use source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
pub use source_profile::documents::SourceProfileDocument;

use tauri::Manager;

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
            let app_state = tauri::async_runtime::block_on(app::state::AppState::new(paths))?;
            let database_path = app_state.paths.database_path.clone();

            app.manage(app_state);
            println!("SQLite database: {}", database_path.display());

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
            app::commands::list_adapters,
            app::commands::list_source_registry_profiles,
            app::commands::list_source_registry_sources,
            app::commands::list_source_registry_diagnostics,
            app::commands::detect_source_from_url,
            app::commands::create_custom_source,
            app::commands::create_search_request,
            app::commands::list_search_requests,
            app::commands::get_search_request,
            app::commands::update_search_request,
            app::commands::delete_search_request,
            app::commands::run_search_request,
            app::commands::list_job_postings,
            app::commands::list_job_postings_for_queue,
            app::commands::get_posting_detail,
            app::commands::get_job_posting_queue_counts,
            app::commands::update_job_posting_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
