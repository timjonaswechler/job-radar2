// #294 owns native Windows Agent auth/session durability and re-enabling
// persistence-backed integration contracts there. POSIX storage contracts
// remain exercised on macOS and Linux instead of being silently weakened.
#[cfg(unix)]
#[path = "agent/chat_application.rs"]
mod chat_application;

#[cfg(unix)]
#[path = "agent/chats.rs"]
mod chats;

#[cfg(unix)]
#[path = "agent/sessions.rs"]
mod sessions;

#[cfg(unix)]
#[path = "agent/model_registry.rs"]
mod model_registry;

#[cfg(unix)]
#[path = "agent/data_root.rs"]
mod data_root;

#[path = "agent/conversation.rs"]
mod conversation;

#[cfg(unix)]
#[path = "agent/configuration_api.rs"]
mod configuration_api;
