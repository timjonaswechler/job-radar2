mod app_state;
// mod commands;
mod db;
mod paths;
// mod deep_links;
// mod domain;
// mod repositories;
// mod search;
// mod services;

use app_state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let paths = paths::AppPaths::from_app(app.handle())?;

            let app_state = tauri::async_runtime::block_on(AppState::new(paths)).map_err(|error| {
                eprintln!("failed to initialize app state: {error}");
                error
            })?;
            println!("database path: {}", app_state.paths.database_path.display());
            app.manage(app_state);

            Ok(())
        });

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("error while running tauri application: {error}");
    }
}
