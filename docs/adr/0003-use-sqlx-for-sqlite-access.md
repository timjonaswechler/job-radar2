# Use SQLx for SQLite access

Job Radar uses SQLx to access the local SQLite database from the Tauri backend. Although rusqlite would be simpler for a purely local desktop app, SQLx is chosen for its migration support, async-friendly API, and clearer path toward a later service mode while still keeping SQLite as the phase-one data store.
