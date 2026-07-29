pub mod api;
mod auth;
mod chat;
mod compaction;
#[cfg(test)]
mod contract_tests;
mod conversation;
pub mod error;
pub mod models;
pub mod openai_codex;
pub mod providers;
pub mod registry;
pub mod sessions;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod testing;

pub use chat::{AgentChat, AgentChatError, AgentChatEvent, AgentChatEventStream, AgentChatState};
pub use conversation::{
    AgentConversation, AssistantContent, AssistantMessage, ContentKind, ConversationEvent,
    ConversationEventStream, ConversationProvider, ConversationRequest, FinishReason, Message,
    ProviderEvent, ProviderEventStream, ProviderTurnCompletion, TokenUsage, TurnCancellation,
    UserMessage,
};
pub use error::{AgentError, AgentErrorCategory};
pub use registry::{ModelRegistry, ModelRegistrySnapshot, ProviderAvailability};
