#[path = "agent/chat_application.rs"]
mod chat_application;

#[path = "agent/chats.rs"]
mod chats;

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
