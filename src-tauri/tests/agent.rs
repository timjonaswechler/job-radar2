// Desktop-owned Agent command projections and composition adapters remain
// integration-tested at the job-radar package seam. Agent behavior, storage,
// auth, registry, and recovery contracts live in crates/agent/tests.
#[cfg(unix)]
#[path = "agent/chat_application.rs"]
mod chat_application;

#[cfg(unix)]
#[path = "agent/configuration_api.rs"]
mod configuration_api;
