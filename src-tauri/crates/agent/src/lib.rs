pub mod api;
mod auth;
mod chat;
mod chats;
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
mod sessions;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod testing;

pub(crate) use chat::{AgentChat, AgentChatError, AgentChatEvent, AgentChatState};
pub use chats::{
    ChatContent, ChatContentKind, ChatCreateInput, ChatError, ChatEvent, ChatEventKind,
    ChatEventListener, ChatHistoryEntry, ChatId, ChatOpenInput, ChatOperationId, ChatProjection,
    ChatReasoningLevel, ChatRecoveryNotice, ChatStatus, Chats,
};
pub use configuration::Configuration;
pub use conversation::{
    AgentConversation, AssistantContent, AssistantMessage, ContentKind, ConversationEvent,
    ConversationEventStream, ConversationProvider, ConversationRequest, FinishReason, Message,
    ProviderEvent, ProviderEventStream, ProviderTurnCompletion, TokenUsage, TurnCancellation,
    UserMessage,
};
pub use error::{AgentError, AgentErrorCategory};
pub use models::{ModelId, ProviderId, ReasoningLevel};
