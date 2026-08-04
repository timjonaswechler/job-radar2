# Treat the local development database as disposable until the first stable release

Until Job Radar reaches its first stable release, the local development SQLite database is disposable runtime state. We may squash development schema migrations and reset local development databases when the schema changes materially.

This does not allow hidden data loss. A reset must be explicit, visible, and limited to debug/development builds. The application must not silently delete a user's database in release builds.

Important source knowledge must therefore not live only in SQLite. Built-in sources and source profiles are versioned JSON artifacts in the repository and embedded into the application bundle. Custom sources and source profiles are user/runtime JSON documents in the OS app data directory. Custom documents must not override bundled built-in keys.

The database remains runtime state for search requests, search runs, results, caches, and diagnostics. It may index loaded JSON documents, but it is not the authoritative store for sources or source profiles. Production-grade migration compatibility becomes a hard requirement when Job Radar starts carrying non-disposable user data across stable releases.

When a squashed development schema removes a column or lifecycle value, developers use the explicit `just db-reset-after-squash` path. This includes the Posting Occurrence identity hard cut from URL approximation to canonical Source-local provider-ID/normalized-URL identity; existing rows are reset rather than assigned invented identities. `just db-preserve-after-squash` is only for unchanged schemas and refuses incompatible data; neither path permits a release build to delete a user's database.
