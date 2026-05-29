# Keep the React UI separate from the Tauri desktop shell

Job Radar keeps the React interface in `apps/web` as the frontend bundle used by the Tauri desktop app, not as a standalone phase-one web product. `apps/desktop` contains the Tauri v2 shell and Rust backend. This keeps Tauri-specific backend code, SQLite access, deep links, and desktop packaging isolated from the React UI.
