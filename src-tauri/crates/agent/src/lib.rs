mod api;
mod chat;
mod chats;
mod compaction;
mod configuration;
#[cfg(test)]
mod contract_tests;
mod conversation;
mod error;
mod models;
mod providers;
mod secure_fs;
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
pub use configuration::{
    AuthenticationKind, Capability, Configuration, ConfigurationState, DataFolderOpener,
    Diagnostic as ConfigurationDiagnostic, Error as ConfigurationError,
    ErrorKind as ConfigurationErrorKind, InteractionError, InteractionFuture, LoginAttemptId,
    LoginInteraction, LoginMethod, LoginProgress, LoginStage,
    ModelStatus as ConfigurationModelStatus, OpenError,
    ProviderStatus as ConfigurationProviderStatus, SecretInput, Status as ConfigurationStatus,
};
pub use conversation::{
    AssistantContent, AssistantMessage, ContentKind, Conversation, ConversationEvent,
    ConversationEventStream, ConversationProvider, ConversationRequest, FinishReason, Message,
    ProviderEvent, ProviderEventStream, ProviderTurnCompletion, TokenUsage, TurnCancellation,
    UserMessage,
};
pub use error::{AgentError, AgentErrorCategory};
pub use models::{Model, ModelId, ProviderId, ReasoningLevel};
