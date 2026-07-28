# agent

Internal, non-publishable Agent subsystem crate for Job Radar.

The crate owns provider-neutral conversation and chat behavior, provider/auth adapters,
the model registry, durable sessions, and compaction. Callers supply an explicit
`agents` data root to persistence-backed constructors. Streaming operations use
`tokio::spawn` and therefore must be started from an active Tokio runtime.

Desktop commands, Tauri events/openers, app-path discovery, and frontend DTO
projections remain in the `job-radar` package.
