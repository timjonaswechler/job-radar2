mod app_state;
mod commands;
mod db;
mod paths;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
