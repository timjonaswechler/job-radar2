use super::models::{ModelId, ProviderId, ReasoningLevel};
use super::sessions::{
    CompactionReason, RecoveryNotice, SessionAccess, SessionErrorCode, SessionId, SessionManager,
    SessionSnapshot, VisibleBlock, VisibleHistoryEntry,
};
use super::{
    AgentChat, AgentChatError, AgentChatEvent, AgentChatState, AgentError, AgentErrorCategory,
    AssistantContent, AssistantMessage, ContentKind, ConversationProvider, ConversationRequest,
    ProviderEventStream, TurnCancellation,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentChatId(String);

impl AgentChatId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AgentChatId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AgentChatId([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationReasoningLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl From<ApplicationReasoningLevel> for ReasoningLevel {
    fn from(value: ApplicationReasoningLevel) -> Self {
        match value {
            ApplicationReasoningLevel::Off => Self::Off,
            ApplicationReasoningLevel::Minimal => Self::Minimal,
            ApplicationReasoningLevel::Low => Self::Low,
            ApplicationReasoningLevel::Medium => Self::Medium,
            ApplicationReasoningLevel::High => Self::High,
            ApplicationReasoningLevel::XHigh => Self::XHigh,
            ApplicationReasoningLevel::Max => Self::Max,
        }
    }
}

impl From<ReasoningLevel> for ApplicationReasoningLevel {
    fn from(value: ReasoningLevel) -> Self {
        match value {
            ReasoningLevel::Off => Self::Off,
            ReasoningLevel::Minimal => Self::Minimal,
            ReasoningLevel::Low => Self::Low,
            ReasoningLevel::Medium => Self::Medium,
            ReasoningLevel::High => Self::High,
            ReasoningLevel::XHigh => Self::XHigh,
            ReasoningLevel::Max => Self::Max,
        }
    }
}

/// The system prompt is opaque application input. It is never projected or logged.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatCreateInput {
    pub system_prompt: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_level: ApplicationReasoningLevel,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatOpenInput {
    pub id: AgentChatId,
    pub system_prompt: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentChatStatus {
    Ready,
    Running,
    ModelUnavailable,
    ReadOnlyLocked,
    ReadOnlyUnsupported,
    Damaged,
    NotSaved,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentChatContent {
    Text { text: String },
    Reasoning { text: String },
    RedactedReasoning,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentChatHistoryEntry {
    Turn {
        user: String,
        assistant: Vec<AgentChatContent>,
    },
    Compaction {
        reason: Option<String>,
        tokens_before: u64,
    },
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatProjection {
    pub id: AgentChatId,
    pub status: AgentChatStatus,
    pub history: Vec<AgentChatHistoryEntry>,
    pub selected_provider_id: Option<String>,
    pub selected_model_id: Option<String>,
    pub reasoning_level: ApplicationReasoningLevel,
    pub recovery_notices: Vec<AgentChatRecoveryNotice>,
}

impl fmt::Debug for AgentChatProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentChatProjection")
            .field("id", &self.id)
            .field("status", &self.status)
            .field("history_entries", &self.history.len())
            .field("selected_provider_id", &self.selected_provider_id)
            .field("selected_model_id", &self.selected_model_id)
            .field("reasoning_level", &self.reasoning_level)
            .field("recovery_notices", &self.recovery_notices)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentChatRecoveryNotice {
    IncompleteFinalTurnDiscarded,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatApplicationError {
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for AgentChatApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentChatApplicationError")
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Display for AgentChatApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for AgentChatApplicationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentChatContentKind {
    Text,
    Reasoning,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatApplicationEvent {
    pub chat_id: AgentChatId,
    pub sequence: u64,
    #[serde(flatten)]
    pub event: AgentChatApplicationEventKind,
}

impl fmt::Debug for AgentChatApplicationEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentChatApplicationEvent")
            .field("chat_id", &self.chat_id)
            .field("sequence", &self.sequence)
            .field("event", &self.event.safe_name())
            .finish()
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentChatApplicationEventKind {
    Started,
    ContentStarted {
        index: usize,
        kind: AgentChatContentKind,
    },
    ContentDelta {
        index: usize,
        delta: String,
    },
    ContentFinished {
        index: usize,
    },
    Completed {
        chat: AgentChatProjection,
    },
    Failed {
        error: AgentChatApplicationError,
    },
    Aborted,
    NotSaved {
        response: Vec<AgentChatContent>,
        error: AgentChatApplicationError,
        chat: AgentChatProjection,
    },
    CompactionStarted {
        reason: String,
    },
    CompactionCompleted {
        reason: String,
        chat: AgentChatProjection,
    },
    CompactionCancelled {
        reason: String,
    },
    CompactionFailed {
        error: AgentChatApplicationError,
    },
    CompactionNotSaved {
        error: AgentChatApplicationError,
        chat: AgentChatProjection,
    },
}

impl AgentChatApplicationEventKind {
    fn safe_name(&self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::ContentStarted { .. } => "content_started",
            Self::ContentDelta { .. } => "content_delta",
            Self::ContentFinished { .. } => "content_finished",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
            Self::Aborted => "aborted",
            Self::NotSaved { .. } => "not_saved",
            Self::CompactionStarted { .. } => "compaction_started",
            Self::CompactionCompleted { .. } => "compaction_completed",
            Self::CompactionCancelled { .. } => "compaction_cancelled",
            Self::CompactionFailed { .. } => "compaction_failed",
            Self::CompactionNotSaved { .. } => "compaction_not_saved",
        }
    }
}

impl fmt::Debug for AgentChatApplicationEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_name())
    }
}

pub trait AgentChatEventListener: Send + Sync + 'static {
    fn emit(&self, event: AgentChatApplicationEvent);
}

#[derive(Clone)]
struct SharedProvider(Arc<dyn ConversationProvider>);

impl ConversationProvider for SharedProvider {
    fn models(&self) -> &[super::models::Model] {
        self.0.models()
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream {
        self.0.stream(request)
    }
}

struct Operation {
    generation: u64,
    cancellation: Option<TurnCancellation>,
    stop_requested: bool,
}

enum OperationKind {
    Send(String),
    Compact(Option<String>),
}

pub struct AgentChatApplication {
    manager: SessionManager,
    provider: SharedProvider,
    chats: Mutex<HashMap<AgentChatId, Arc<AsyncMutex<AgentChat>>>>,
    operations: Mutex<HashMap<AgentChatId, Operation>>,
    next_generation: AtomicU64,
    next_sequence: AtomicU64,
}

impl AgentChatApplication {
    pub fn new(manager: SessionManager, provider: impl ConversationProvider + 'static) -> Self {
        Self {
            manager,
            provider: SharedProvider(Arc::new(provider)),
            chats: Mutex::new(HashMap::new()),
            operations: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
            next_sequence: AtomicU64::new(1),
        }
    }

    pub fn create(
        &self,
        input: AgentChatCreateInput,
    ) -> Result<AgentChatProjection, AgentChatApplicationError> {
        let provider = ProviderId::new(input.provider_id).map_err(|_| invalid_request())?;
        let model = ModelId::new(input.model_id).map_err(|_| invalid_request())?;
        let chat = AgentChat::create(
            &self.manager,
            input.system_prompt,
            self.provider.clone(),
            provider,
            model,
            input.reasoning_level.into(),
        )
        .map_err(map_chat_error)?;
        let projection = project_chat(&chat, false);
        self.chats
            .lock()
            .expect("Agent Chat registry lock poisoned")
            .insert(projection.id.clone(), Arc::new(AsyncMutex::new(chat)));
        Ok(projection)
    }

    pub async fn open(
        &self,
        input: AgentChatOpenInput,
    ) -> Result<AgentChatProjection, AgentChatApplicationError> {
        if let Some(chat) = self.chat(&input.id) {
            let chat = chat.lock().await;
            return Ok(project_chat(&chat, self.is_running(&input.id)));
        }
        let session_id = parse_id(&input.id)?;
        let chat = AgentChat::open(
            &self.manager,
            &session_id,
            input.system_prompt,
            self.provider.clone(),
        )
        .map_err(map_chat_error)?;
        let projection = project_chat(&chat, false);
        self.chats
            .lock()
            .expect("Agent Chat registry lock poisoned")
            .insert(input.id, Arc::new(AsyncMutex::new(chat)));
        Ok(projection)
    }

    pub async fn snapshot(
        &self,
        id: &AgentChatId,
    ) -> Result<AgentChatProjection, AgentChatApplicationError> {
        let chat = self.chat(id).ok_or_else(chat_not_open)?;
        let chat = chat.lock().await;
        Ok(project_chat(&chat, self.is_running(id)))
    }

    pub fn send(
        self: &Arc<Self>,
        id: AgentChatId,
        text: String,
        listener: Arc<dyn AgentChatEventListener>,
    ) -> Result<(), AgentChatApplicationError> {
        self.start_operation(id, OperationKind::Send(text), listener)
    }

    pub fn compact(
        self: &Arc<Self>,
        id: AgentChatId,
        focus: Option<String>,
        listener: Arc<dyn AgentChatEventListener>,
    ) -> Result<(), AgentChatApplicationError> {
        self.start_operation(id, OperationKind::Compact(focus), listener)
    }

    pub fn stop(&self, id: &AgentChatId) -> bool {
        let mut operations = self
            .operations
            .lock()
            .expect("Agent Chat operation lock poisoned");
        let Some(operation) = operations.get_mut(id) else {
            return false;
        };
        operation.stop_requested = true;
        if let Some(cancellation) = &operation.cancellation {
            cancellation.cancel();
        }
        true
    }

    pub async fn select_model(
        &self,
        id: &AgentChatId,
        provider_id: String,
        model_id: String,
    ) -> Result<AgentChatProjection, AgentChatApplicationError> {
        self.ensure_idle(id)?;
        let chat = self.chat(id).ok_or_else(chat_not_open)?;
        let mut chat = chat.lock().await;
        self.ensure_idle(id)?;
        chat.select_model(
            ProviderId::new(provider_id).map_err(|_| invalid_request())?,
            ModelId::new(model_id).map_err(|_| invalid_request())?,
        )
        .map_err(map_chat_error)?;
        Ok(project_chat(&chat, false))
    }

    pub async fn set_reasoning_level(
        &self,
        id: &AgentChatId,
        reasoning_level: ApplicationReasoningLevel,
    ) -> Result<AgentChatProjection, AgentChatApplicationError> {
        self.ensure_idle(id)?;
        let chat = self.chat(id).ok_or_else(chat_not_open)?;
        let mut chat = chat.lock().await;
        self.ensure_idle(id)?;
        chat.set_reasoning_level(reasoning_level.into())
            .map_err(map_chat_error)?;
        Ok(project_chat(&chat, false))
    }

    fn start_operation(
        self: &Arc<Self>,
        id: AgentChatId,
        kind: OperationKind,
        listener: Arc<dyn AgentChatEventListener>,
    ) -> Result<(), AgentChatApplicationError> {
        let chat = self.chat(&id).ok_or_else(chat_not_open)?;
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        {
            let mut operations = self
                .operations
                .lock()
                .expect("Agent Chat operation lock poisoned");
            if operations.contains_key(&id) {
                return Err(chat_busy());
            }
            operations.insert(
                id.clone(),
                Operation {
                    generation,
                    cancellation: None,
                    stop_requested: false,
                },
            );
        }
        let application = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            application
                .run_operation(id, generation, chat, kind, listener)
                .await;
        });
        Ok(())
    }

    async fn run_operation(
        self: Arc<Self>,
        id: AgentChatId,
        generation: u64,
        chat: Arc<AsyncMutex<AgentChat>>,
        kind: OperationKind,
        listener: Arc<dyn AgentChatEventListener>,
    ) {
        let mut chat = chat.lock().await;
        let stream = match kind {
            OperationKind::Send(text) => chat.send(text),
            OperationKind::Compact(focus) => chat.compact(focus),
        };
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                self.emit(
                    &id,
                    listener.as_ref(),
                    AgentChatApplicationEventKind::Failed {
                        error: map_chat_error(error),
                    },
                );
                self.finish_operation(&id, generation);
                return;
            }
        };
        let cancellation = stream.cancellation();
        {
            let mut operations = self
                .operations
                .lock()
                .expect("Agent Chat operation lock poisoned");
            let Some(operation) = operations
                .get_mut(&id)
                .filter(|operation| operation.generation == generation)
            else {
                cancellation.cancel();
                return;
            };
            operation.cancellation = Some(cancellation.clone());
            if operation.stop_requested {
                cancellation.cancel();
            }
        }

        while let Some(event) = stream.next().await {
            let mut projected = project_snapshot(stream.snapshot(), stream.state(), false);
            projected.selected_provider_id = Some(stream.selected_provider().as_str().to_owned());
            projected.selected_model_id = Some(stream.selected_model().as_str().to_owned());
            projected.reasoning_level = stream.reasoning_level().into();
            let event = project_event(event, projected);
            if stream.is_finished() {
                self.finish_operation(&id, generation);
                self.emit(&id, listener.as_ref(), event);
                return;
            }
            self.emit(&id, listener.as_ref(), event);
        }
        self.finish_operation(&id, generation);
    }

    fn emit(
        &self,
        id: &AgentChatId,
        listener: &dyn AgentChatEventListener,
        event: AgentChatApplicationEventKind,
    ) {
        listener.emit(AgentChatApplicationEvent {
            chat_id: id.clone(),
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            event,
        });
    }

    fn finish_operation(&self, id: &AgentChatId, generation: u64) {
        let mut operations = self
            .operations
            .lock()
            .expect("Agent Chat operation lock poisoned");
        if operations
            .get(id)
            .is_some_and(|operation| operation.generation == generation)
        {
            operations.remove(id);
        }
    }

    fn chat(&self, id: &AgentChatId) -> Option<Arc<AsyncMutex<AgentChat>>> {
        self.chats
            .lock()
            .expect("Agent Chat registry lock poisoned")
            .get(id)
            .cloned()
    }

    fn ensure_idle(&self, id: &AgentChatId) -> Result<(), AgentChatApplicationError> {
        if self.is_running(id) {
            Err(chat_busy())
        } else {
            Ok(())
        }
    }

    fn is_running(&self, id: &AgentChatId) -> bool {
        self.operations
            .lock()
            .expect("Agent Chat operation lock poisoned")
            .contains_key(id)
    }
}

fn parse_id(id: &AgentChatId) -> Result<SessionId, AgentChatApplicationError> {
    SessionId::from_str(id.as_str()).map_err(|_| invalid_request())
}

fn project_chat(chat: &AgentChat, running: bool) -> AgentChatProjection {
    let mut projection = project_snapshot(chat.snapshot(), chat.state(), running);
    projection.selected_provider_id = chat
        .selected_provider()
        .map(|provider| provider.as_str().to_owned());
    projection.selected_model_id = chat.selected_model().map(|model| model.as_str().to_owned());
    projection.reasoning_level = chat.reasoning_level().into();
    projection
}

fn project_snapshot(
    snapshot: &SessionSnapshot,
    state: AgentChatState,
    running: bool,
) -> AgentChatProjection {
    AgentChatProjection {
        id: AgentChatId(snapshot.id().to_string()),
        status: if running {
            AgentChatStatus::Running
        } else {
            match state {
                AgentChatState::Ready => AgentChatStatus::Ready,
                AgentChatState::ModelUnavailable => AgentChatStatus::ModelUnavailable,
                AgentChatState::NotSaved => AgentChatStatus::NotSaved,
                AgentChatState::ReadOnly => match snapshot.access() {
                    SessionAccess::ReadOnlyLocked => AgentChatStatus::ReadOnlyLocked,
                    SessionAccess::ReadOnlyUnsupported => AgentChatStatus::ReadOnlyUnsupported,
                    SessionAccess::Damaged => AgentChatStatus::Damaged,
                    SessionAccess::Writable => AgentChatStatus::Ready,
                },
            }
        },
        history: snapshot
            .visible_history()
            .iter()
            .map(|entry| match entry {
                VisibleHistoryEntry::Turn(turn) => AgentChatHistoryEntry::Turn {
                    user: turn.user().to_owned(),
                    assistant: turn
                        .assistant()
                        .iter()
                        .map(|block| match block {
                            VisibleBlock::Text(text) => {
                                AgentChatContent::Text { text: text.clone() }
                            }
                            VisibleBlock::Thinking(text) => {
                                AgentChatContent::Reasoning { text: text.clone() }
                            }
                            VisibleBlock::RedactedThinking => AgentChatContent::RedactedReasoning,
                        })
                        .collect(),
                },
                VisibleHistoryEntry::Compaction(compaction) => AgentChatHistoryEntry::Compaction {
                    reason: compaction.reason().map(str::to_owned),
                    tokens_before: compaction.tokens_before(),
                },
            })
            .collect(),
        selected_provider_id: snapshot
            .selected_provider()
            .map(|provider| provider.as_str().to_owned()),
        selected_model_id: snapshot
            .selected_model()
            .map(|model| model.as_str().to_owned()),
        reasoning_level: snapshot.reasoning_level().into(),
        recovery_notices: snapshot
            .recovery_notices()
            .iter()
            .map(|notice| match notice {
                RecoveryNotice::IncompleteFinalTurnDiscarded => {
                    AgentChatRecoveryNotice::IncompleteFinalTurnDiscarded
                }
            })
            .collect(),
    }
}

fn project_event(
    event: AgentChatEvent,
    chat: AgentChatProjection,
) -> AgentChatApplicationEventKind {
    match event {
        AgentChatEvent::Started => AgentChatApplicationEventKind::Started,
        AgentChatEvent::ContentStarted { index, kind } => {
            AgentChatApplicationEventKind::ContentStarted {
                index,
                kind: match kind {
                    ContentKind::Text => AgentChatContentKind::Text,
                    ContentKind::Reasoning => AgentChatContentKind::Reasoning,
                },
            }
        }
        AgentChatEvent::ContentDelta { index, delta } => {
            AgentChatApplicationEventKind::ContentDelta { index, delta }
        }
        AgentChatEvent::ContentFinished { index } => {
            AgentChatApplicationEventKind::ContentFinished { index }
        }
        AgentChatEvent::Completed { .. } => AgentChatApplicationEventKind::Completed { chat },
        AgentChatEvent::Failed { error } => AgentChatApplicationEventKind::Failed {
            error: map_agent_error(error),
        },
        AgentChatEvent::Aborted => AgentChatApplicationEventKind::Aborted,
        AgentChatEvent::NotSaved { message, error } => AgentChatApplicationEventKind::NotSaved {
            response: project_message(&message),
            error: map_session_error(error.code()),
            chat,
        },
        AgentChatEvent::CompactionStarted { reason } => {
            AgentChatApplicationEventKind::CompactionStarted {
                reason: compaction_reason(reason).to_owned(),
            }
        }
        AgentChatEvent::CompactionCompleted { reason } => {
            AgentChatApplicationEventKind::CompactionCompleted {
                reason: compaction_reason(reason).to_owned(),
                chat,
            }
        }
        AgentChatEvent::CompactionCancelled { reason } => {
            AgentChatApplicationEventKind::CompactionCancelled {
                reason: compaction_reason(reason).to_owned(),
            }
        }
        AgentChatEvent::CompactionFailed { error } => {
            AgentChatApplicationEventKind::CompactionFailed {
                error: map_agent_error(error),
            }
        }
        AgentChatEvent::CompactionNotSaved { error } => {
            AgentChatApplicationEventKind::CompactionNotSaved {
                error: map_session_error(error.code()),
                chat,
            }
        }
    }
}

fn project_message(message: &AssistantMessage) -> Vec<AgentChatContent> {
    message
        .content()
        .iter()
        .map(|content| match content {
            AssistantContent::Text(text) => AgentChatContent::Text { text: text.clone() },
            AssistantContent::Reasoning(text) => AgentChatContent::Reasoning { text: text.clone() },
        })
        .collect()
}

fn compaction_reason(reason: CompactionReason) -> &'static str {
    match reason {
        CompactionReason::Manual => "manual",
        CompactionReason::Threshold => "threshold",
        CompactionReason::Overflow => "overflow",
    }
}

fn map_chat_error(error: AgentChatError) -> AgentChatApplicationError {
    match error {
        AgentChatError::Agent(error) => map_agent_error(error),
        AgentChatError::Session(error) => map_session_error(error.code()),
        AgentChatError::ModelUnavailable => model_unavailable(),
        AgentChatError::NotSaved => not_saved(),
    }
}

fn map_agent_error(error: AgentError) -> AgentChatApplicationError {
    match error.category {
        AgentErrorCategory::Authentication => AgentChatApplicationError {
            code: "authentication_unavailable",
            message: "AI provider authentication is unavailable",
        },
        AgentErrorCategory::InvalidConfiguration => AgentChatApplicationError {
            code: "invalid_configuration",
            message: "Agent Chat configuration is unavailable",
        },
        AgentErrorCategory::RateLimited => AgentChatApplicationError {
            code: "rate_limited",
            message: "AI provider rate limit reached",
        },
        AgentErrorCategory::Transport => AgentChatApplicationError {
            code: "transport_unavailable",
            message: "AI provider transport is unavailable",
        },
        AgentErrorCategory::ModelUnavailable => model_unavailable(),
        AgentErrorCategory::ContextOverflow => AgentChatApplicationError {
            code: "context_full",
            message: "Agent Chat context is full",
        },
        AgentErrorCategory::Provider => AgentChatApplicationError {
            code: "provider_failed",
            message: "AI provider request failed",
        },
    }
}

fn map_session_error(code: SessionErrorCode) -> AgentChatApplicationError {
    match code {
        SessionErrorCode::InvalidSessionId => invalid_request(),
        SessionErrorCode::NotFound => AgentChatApplicationError {
            code: "chat_not_found",
            message: "Agent Chat was not found",
        },
        SessionErrorCode::Locked => AgentChatApplicationError {
            code: "chat_read_only",
            message: "Agent Chat is read-only",
        },
        SessionErrorCode::Unsupported => AgentChatApplicationError {
            code: "chat_unsupported",
            message: "Agent Chat format is read-only",
        },
        SessionErrorCode::Damaged | SessionErrorCode::IncompleteFinalSuffix => {
            AgentChatApplicationError {
                code: "chat_damaged",
                message: "Agent Chat data is damaged",
            }
        }
        SessionErrorCode::NotSaved | SessionErrorCode::ExternalChange => not_saved(),
        SessionErrorCode::InvalidRoot
        | SessionErrorCode::SizeLimit
        | SessionErrorCode::TrashFailed => AgentChatApplicationError {
            code: "chat_unavailable",
            message: "Agent Chat is unavailable",
        },
    }
}

fn chat_not_open() -> AgentChatApplicationError {
    AgentChatApplicationError {
        code: "chat_not_open",
        message: "Agent Chat is not open",
    }
}

fn chat_busy() -> AgentChatApplicationError {
    AgentChatApplicationError {
        code: "chat_busy",
        message: "Agent Chat already has an active operation",
    }
}

fn invalid_request() -> AgentChatApplicationError {
    AgentChatApplicationError {
        code: "invalid_request",
        message: "Agent Chat request is invalid",
    }
}

fn model_unavailable() -> AgentChatApplicationError {
    AgentChatApplicationError {
        code: "model_unavailable",
        message: "Agent Model is unavailable",
    }
}

fn not_saved() -> AgentChatApplicationError {
    AgentChatApplicationError {
        code: "not_saved",
        message: "Agent Chat change was not saved",
    }
}
