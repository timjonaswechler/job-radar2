pub mod api;
mod auth;
mod chat;
mod compaction;
pub mod configuration;
#[cfg(test)]
mod contract_tests;
mod conversation;
pub mod error;
pub mod models;
mod openai_codex;
mod providers;
mod registry;
pub mod sessions;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod testing;

pub use chat::{AgentChat, AgentChatError, AgentChatEvent, AgentChatEventStream, AgentChatState};
pub use configuration::Configuration;
pub use conversation::{
    AgentConversation, AssistantContent, AssistantMessage, ContentKind, ConversationEvent,
    ConversationEventStream, ConversationProvider, ConversationRequest, FinishReason, Message,
    ProviderEvent, ProviderEventStream, ProviderTurnCompletion, TokenUsage, TurnCancellation,
    UserMessage,
};
pub use error::{AgentError, AgentErrorCategory};
pub use models::{ModelId, ProviderId, ReasoningLevel};
