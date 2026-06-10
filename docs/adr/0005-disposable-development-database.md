# Treat the local development database as disposable until the first stable release

Until Job Radar reaches its first stable release, the local development SQLite database is disposable runtime state. We may squash development schema migrations and reset local development databases when the schema changes materially.

This does not allow hidden data loss. A reset must be explicit, visible, and limited to debug/development builds. The application must not silently delete a user's database in release builds.

Important system knowledge must therefore not live only in SQLite. Built-in system profiles are versioned JSON source artifacts in the repository and are embedded into the application bundle. On startup the app seeds/upserts those bundled profiles into `system_profiles` with `built_in = 1`.

Custom system profiles are user/runtime data. They live next to the database in the OS app data directory under `system-profiles/*.json` and are loaded after built-ins with `built_in = 0`. Custom profiles must not override a bundled built-in key.

The database remains the runtime index, editable copy, and cache layer. Production-grade migration compatibility becomes a hard requirement when Job Radar starts carrying non-disposable user data across stable releases.
