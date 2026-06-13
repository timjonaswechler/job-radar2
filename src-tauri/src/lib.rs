mod adapter_registry;
mod app_state;
mod browser_runtime;
mod commands;
mod db;
mod paths;
mod search_request_model;
mod search_run_model;
mod source_detection;
mod source_model;

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
            let paths = paths::AppPaths::from_app(app.handle())?;
            let app_state = tauri::async_runtime::block_on(app_state::AppState::new(paths))?;
            let database_path = app_state.paths.database_path.clone();

            app.manage(app_state);
            println!("SQLite database: {}", database_path.display());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::get_database_info,
            commands::get_app_preferences,
            commands::set_app_preferences,
            commands::set_app_theme,
            commands::set_app_language,
            commands::set_default_search_radius_km,
            commands::get_browser_runtime_status,
            commands::install_browser_runtime,
            commands::uninstall_browser_runtime,
            commands::check_browser_runtime,
            commands::list_adapters,
            commands::detect_source_from_url,
            commands::test_system_profile_url,
            commands::create_search_request,
            commands::list_search_requests,
            commands::get_search_request,
            commands::update_search_request,
            commands::delete_search_request,
            commands::run_search_request,
            commands::create_browser_profile,
            commands::list_browser_profiles,
            commands::get_browser_profile,
            commands::update_browser_profile,
            commands::delete_browser_profile,
            commands::create_system_profile,
            commands::list_system_profiles,
            commands::get_system_profile,
            commands::update_system_profile,
            commands::delete_system_profile,
            commands::export_system_profile_json,
            commands::export_system_profile_json_file,
            commands::import_system_profile_json,
            commands::create_source,
            commands::list_sources,
            commands::get_source,
            commands::update_source,
            commands::delete_source,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
