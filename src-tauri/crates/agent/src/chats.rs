use super::models::{ModelId, ProviderId, ReasoningLevel};
use super::secure_fs::{canonical_existing_prefix_is_inside_repository, path_is_inside_repository};
use super::sessions::{
    CompactionReason, RecoveryNotice, SessionAccess, SessionErrorCode, SessionId, SessionManager,
    SessionSnapshot, VisibleBlock, VisibleHistoryEntry,
};
use super::{
    AgentChat, AgentChatError, AgentChatEvent, AgentChatState, AgentError, AgentErrorCategory,
    AssistantContent, AssistantMessage, ContentKind, ConversationProvider, ConversationRequest,
    ProviderEventStream, TurnCancellation,
};
use futures_util::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChatId(String);

impl ChatId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ChatId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ChatId([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ChatOperationId(u64);

impl ChatOperationId {
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatReasoningLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl From<ChatReasoningLevel> for ReasoningLevel {
    fn from(value: ChatReasoningLevel) -> Self {
        match value {
            ChatReasoningLevel::Off => Self::Off,
            ChatReasoningLevel::Minimal => Self::Minimal,
            ChatReasoningLevel::Low => Self::Low,
            ChatReasoningLevel::Medium => Self::Medium,
            ChatReasoningLevel::High => Self::High,
            ChatReasoningLevel::XHigh => Self::XHigh,
            ChatReasoningLevel::Max => Self::Max,
        }
    }
}

impl From<ReasoningLevel> for ChatReasoningLevel {
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
pub struct ChatCreateInput {
    pub system_prompt: String,
    pub provider_id: String,
    pub model_id: String,
    pub reasoning_level: ChatReasoningLevel,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatOpenInput {
    pub id: ChatId,
    pub system_prompt: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatStatus {
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
pub enum ChatContent {
    Text { text: String },
    Reasoning { text: String },
    RedactedReasoning,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatHistoryEntry {
    Turn {
        user: String,
        assistant: Vec<ChatContent>,
    },
    Compaction {
        reason: Option<String>,
        tokens_before: u64,
    },
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatProjection {
    pub id: ChatId,
    pub status: ChatStatus,
    pub history: Vec<ChatHistoryEntry>,
    pub selected_provider_id: Option<String>,
    pub selected_model_id: Option<String>,
    pub reasoning_level: ChatReasoningLevel,
    pub context_tokens: u64,
    pub context_window: Option<u64>,
    pub recovery_notices: Vec<ChatRecoveryNotice>,
}

impl fmt::Debug for ChatProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatProjection")
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
pub enum ChatRecoveryNotice {
    IncompleteFinalTurnDiscarded,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatError {
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for ChatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatError")
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Display for ChatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ChatError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatContentKind {
    Text,
    Reasoning,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub chat_id: ChatId,
    pub sequence: u64,
    #[serde(flatten)]
    pub event: ChatEventKind,
}

impl fmt::Debug for ChatEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChatEvent")
            .field("chat_id", &self.chat_id)
            .field("sequence", &self.sequence)
            .field("event", &self.event.safe_name())
            .finish()
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEventKind {
    Started,
    ContentStarted {
        index: usize,
        kind: ChatContentKind,
    },
    ContentDelta {
        index: usize,
        delta: String,
    },
    ContentFinished {
        index: usize,
    },
    Completed {
        chat: ChatProjection,
    },
    Failed {
        error: ChatError,
    },
    Aborted,
    NotSaved {
        response: Vec<ChatContent>,
        error: ChatError,
        chat: ChatProjection,
    },
    CompactionStarted {
        reason: String,
    },
    CompactionCompleted {
        reason: String,
        chat: ChatProjection,
    },
    CompactionCancelled {
        reason: String,
    },
    CompactionFailed {
        error: ChatError,
    },
    CompactionNotSaved {
        error: ChatError,
        chat: ChatProjection,
    },
}

impl ChatEventKind {
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

impl fmt::Debug for ChatEventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_name())
    }
}

pub trait ChatEventListener: Send + Sync + 'static {
    fn emit(&self, event: ChatEvent);
}

#[derive(Clone)]
struct SharedProvider(Arc<dyn ConversationProvider>);

impl ConversationProvider for SharedProvider {
    fn models(&self) -> &[super::models::Model] {
        self.0.models()
    }

    fn model_snapshot(&self) -> Vec<super::models::Model> {
        self.0.model_snapshot()
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

struct ChatSlot {
    chat: AsyncMutex<AgentChat>,
    projection: Mutex<ChatProjection>,
}

struct OperationGuard {
    chats: Arc<Chats>,
    slot: Arc<ChatSlot>,
    id: ChatId,
    generation: u64,
    released: bool,
}

impl OperationGuard {
    fn release(&mut self) -> bool {
        if self.released {
            return false;
        }
        let current = self
            .chats
            .release_operation(&self.slot, &self.id, self.generation);
        self.released = true;
        current
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        self.release();
    }
}

pub struct Chats {
    manager: SessionManager,
    provider: SharedProvider,
    chats: Mutex<HashMap<ChatId, Arc<ChatSlot>>>,
    operations: Mutex<HashMap<ChatId, Operation>>,
    next_generation: AtomicU64,
    next_sequence: AtomicU64,
}

impl Chats {
    pub fn new(
        agents_data_root: impl AsRef<Path>,
        provider: impl ConversationProvider + 'static,
    ) -> Result<Self, ChatError> {
        let root = prepare_agents_root(agents_data_root.as_ref())?;
        let manager = SessionManager::from_agents_data_root(&root)
            .map_err(|error| map_session_error(error.code()))?;
        Ok(Self {
            manager,
            provider: SharedProvider(Arc::new(provider)),
            chats: Mutex::new(HashMap::new()),
            operations: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
            next_sequence: AtomicU64::new(1),
        })
    }

    pub fn create(&self, input: ChatCreateInput) -> Result<ChatProjection, ChatError> {
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
        let id = projection.id.clone();
        self.chats
            .lock()
            .expect("Agent Chat registry lock poisoned")
            .insert(
                id,
                Arc::new(ChatSlot {
                    chat: AsyncMutex::new(chat),
                    projection: Mutex::new(projection.clone()),
                }),
            );
        Ok(projection)
    }

    pub async fn open(&self, input: ChatOpenInput) -> Result<ChatProjection, ChatError> {
        if let Some(slot) = self.chat(&input.id) {
            let projection = slot
                .projection
                .lock()
                .expect("Agent Chat projection lock poisoned")
                .clone();
            return Ok(projection);
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
        let mut chats = self
            .chats
            .lock()
            .expect("Agent Chat registry lock poisoned");
        if let Some(slot) = chats.get(&input.id) {
            return Ok(slot
                .projection
                .lock()
                .expect("Agent Chat projection lock poisoned")
                .clone());
        }
        chats.insert(
            input.id,
            Arc::new(ChatSlot {
                chat: AsyncMutex::new(chat),
                projection: Mutex::new(projection.clone()),
            }),
        );
        Ok(projection)
    }

    pub async fn snapshot(&self, id: &ChatId) -> Result<ChatProjection, ChatError> {
        let slot = self.chat(id).ok_or_else(chat_not_open)?;
        let projection = slot
            .projection
            .lock()
            .expect("Agent Chat projection lock poisoned")
            .clone();
        Ok(projection)
    }

    pub fn send(
        self: &Arc<Self>,
        id: ChatId,
        text: String,
        listener: Arc<dyn ChatEventListener>,
    ) -> Result<ChatOperationId, ChatError> {
        self.start_operation(id, OperationKind::Send(text), listener)
    }

    pub fn compact(
        self: &Arc<Self>,
        id: ChatId,
        focus: Option<String>,
        listener: Arc<dyn ChatEventListener>,
    ) -> Result<ChatOperationId, ChatError> {
        self.start_operation(id, OperationKind::Compact(focus), listener)
    }

    pub fn stop(&self, id: &ChatId, operation_id: ChatOperationId) -> bool {
        self.stop_matching(id, Some(operation_id))
    }

    pub fn stop_current(&self, id: &ChatId) -> bool {
        self.stop_matching(id, None)
    }

    fn stop_matching(&self, id: &ChatId, expected: Option<ChatOperationId>) -> bool {
        let mut operations = self
            .operations
            .lock()
            .expect("Agent Chat operation lock poisoned");
        let Some(operation) = operations.get_mut(id) else {
            return false;
        };
        if expected.is_some_and(|expected| expected.0 != operation.generation) {
            return false;
        }
        operation.stop_requested = true;
        if let Some(cancellation) = &operation.cancellation {
            cancellation.cancel();
        }
        true
    }

    pub async fn select_model(
        &self,
        id: &ChatId,
        provider_id: String,
        model_id: String,
    ) -> Result<ChatProjection, ChatError> {
        let slot = self.chat(id).ok_or_else(chat_not_open)?;
        let mut chat = slot.chat.lock().await;
        self.ensure_idle(id)?;
        if let Err(error) = chat.select_model(
            ProviderId::new(provider_id).map_err(|_| invalid_request())?,
            ModelId::new(model_id).map_err(|_| invalid_request())?,
        ) {
            let projection = project_chat(&chat, false);
            self.replace_projection(&slot, projection);
            return Err(map_chat_error(error));
        }
        let projection = project_chat(&chat, false);
        self.replace_projection(&slot, projection.clone());
        Ok(projection)
    }

    pub async fn set_reasoning_level(
        &self,
        id: &ChatId,
        reasoning_level: ChatReasoningLevel,
    ) -> Result<ChatProjection, ChatError> {
        let slot = self.chat(id).ok_or_else(chat_not_open)?;
        let mut chat = slot.chat.lock().await;
        self.ensure_idle(id)?;
        if let Err(error) = chat.set_reasoning_level(reasoning_level.into()) {
            let projection = project_chat(&chat, false);
            self.replace_projection(&slot, projection);
            return Err(map_chat_error(error));
        }
        let projection = project_chat(&chat, false);
        self.replace_projection(&slot, projection.clone());
        Ok(projection)
    }

    pub async fn reload(&self, id: &ChatId) -> Result<ChatProjection, ChatError> {
        let slot = self.chat(id).ok_or_else(chat_not_open)?;
        let mut chat = slot.chat.lock().await;
        self.ensure_idle(id)?;
        chat.reload().map_err(map_chat_error)?;
        let projection = project_chat(&chat, false);
        self.replace_projection(&slot, projection.clone());
        Ok(projection)
    }

    fn start_operation(
        self: &Arc<Self>,
        id: ChatId,
        kind: OperationKind,
        listener: Arc<dyn ChatEventListener>,
    ) -> Result<ChatOperationId, ChatError> {
        let slot = self.chat(&id).ok_or_else(chat_not_open)?;
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
            let mut projection = slot
                .projection
                .lock()
                .expect("Agent Chat projection lock poisoned");
            projection.status = ChatStatus::Running;
        }
        let application = Arc::clone(self);
        let guard = OperationGuard {
            chats: Arc::clone(self),
            slot: Arc::clone(&slot),
            id: id.clone(),
            generation,
            released: false,
        };
        let cleanup = Arc::clone(self);
        let cleanup_slot = Arc::clone(&slot);
        let cleanup_id = id.clone();
        tokio::spawn(async move {
            let result = std::panic::AssertUnwindSafe(
                application.run_operation(id, generation, slot, kind, listener, guard),
            )
            .catch_unwind()
            .await;
            if result.is_err() {
                cleanup.release_operation(&cleanup_slot, &cleanup_id, generation);
            }
        });
        Ok(ChatOperationId(generation))
    }

    async fn run_operation(
        self: Arc<Self>,
        id: ChatId,
        generation: u64,
        slot: Arc<ChatSlot>,
        kind: OperationKind,
        listener: Arc<dyn ChatEventListener>,
        mut guard: OperationGuard,
    ) {
        let mut chat = slot.chat.lock().await;
        let stream = match kind {
            OperationKind::Send(text) => chat.send(text),
            OperationKind::Compact(focus) => chat.compact(focus),
        };
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                let projected = project_chat(&chat, false);
                self.replace_projection(&slot, projected);
                if guard.release() {
                    self.emit(
                        &id,
                        listener.as_ref(),
                        ChatEventKind::Failed {
                            error: map_chat_error(error),
                        },
                    );
                }
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
            let terminal = stream.is_finished();
            let mut projected = project_snapshot(
                stream.snapshot(),
                stream.state(),
                !terminal,
                Some(stream.context_window()),
            );
            projected.selected_provider_id = Some(stream.selected_provider().as_str().to_owned());
            projected.selected_model_id = Some(stream.selected_model().as_str().to_owned());
            projected.reasoning_level = stream.reasoning_level().into();
            self.replace_projection(&slot, projected.clone());
            let event = project_event(event, projected);
            if terminal {
                if guard.release() {
                    self.emit(&id, listener.as_ref(), event);
                }
                return;
            }
            self.emit(&id, listener.as_ref(), event);
        }
        self.replace_projection(&slot, project_chat(&chat, false));
        guard.release();
    }

    fn emit(&self, id: &ChatId, listener: &dyn ChatEventListener, event: ChatEventKind) {
        listener.emit(ChatEvent {
            chat_id: id.clone(),
            sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
            event,
        });
    }

    fn release_operation(&self, slot: &ChatSlot, id: &ChatId, generation: u64) -> bool {
        let mut operations = self
            .operations
            .lock()
            .expect("Agent Chat operation lock poisoned");
        let current = operations
            .get(id)
            .is_some_and(|operation| operation.generation == generation);
        if current {
            operations.remove(id);
            let mut projection = slot
                .projection
                .lock()
                .expect("Agent Chat projection lock poisoned");
            if projection.status == ChatStatus::Running {
                projection.status = ChatStatus::Ready;
            }
        }
        current
    }

    fn replace_projection(&self, slot: &ChatSlot, projection: ChatProjection) {
        *slot
            .projection
            .lock()
            .expect("Agent Chat projection lock poisoned") = projection;
    }

    fn chat(&self, id: &ChatId) -> Option<Arc<ChatSlot>> {
        self.chats
            .lock()
            .expect("Agent Chat registry lock poisoned")
            .get(id)
            .cloned()
    }

    fn ensure_idle(&self, id: &ChatId) -> Result<(), ChatError> {
        if self.is_running(id) {
            Err(chat_busy())
        } else {
            Ok(())
        }
    }

    fn is_running(&self, id: &ChatId) -> bool {
        self.operations
            .lock()
            .expect("Agent Chat operation lock poisoned")
            .contains_key(id)
    }
}

fn prepare_agents_root(root: &Path) -> Result<std::path::PathBuf, ChatError> {
    if !root.is_absolute()
        || root.file_name().and_then(|name| name.to_str()) != Some("agents")
        || path_is_inside_repository(root)
        || canonical_existing_prefix_is_inside_repository(root)
    {
        return Err(map_session_error(SessionErrorCode::InvalidRoot));
    }
    let _existing = root
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .ok_or_else(|| map_session_error(SessionErrorCode::InvalidRoot))?;
    std::fs::create_dir_all(root).map_err(|_| map_session_error(SessionErrorCode::InvalidRoot))?;
    std::fs::canonicalize(root).map_err(|_| map_session_error(SessionErrorCode::InvalidRoot))
}

fn parse_id(id: &ChatId) -> Result<SessionId, ChatError> {
    SessionId::from_str(id.as_str()).map_err(|_| invalid_request())
}

fn project_chat(chat: &AgentChat, running: bool) -> ChatProjection {
    let mut projection = project_snapshot(
        chat.snapshot(),
        chat.state(),
        running,
        chat.context_window(),
    );
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
    context_window: Option<u64>,
) -> ChatProjection {
    ChatProjection {
        id: ChatId(snapshot.id().to_string()),
        status: if running {
            ChatStatus::Running
        } else {
            match state {
                AgentChatState::Ready => ChatStatus::Ready,
                AgentChatState::ModelUnavailable => ChatStatus::ModelUnavailable,
                AgentChatState::NotSaved => ChatStatus::NotSaved,
                AgentChatState::ReadOnly => match snapshot.access() {
                    SessionAccess::ReadOnlyLocked => ChatStatus::ReadOnlyLocked,
                    SessionAccess::ReadOnlyUnsupported => ChatStatus::ReadOnlyUnsupported,
                    SessionAccess::Damaged => ChatStatus::Damaged,
                    SessionAccess::Writable => ChatStatus::Ready,
                },
            }
        },
        history: snapshot
            .visible_history()
            .iter()
            .map(|entry| match entry {
                VisibleHistoryEntry::Turn(turn) => ChatHistoryEntry::Turn {
                    user: turn.user().to_owned(),
                    assistant: turn
                        .assistant()
                        .iter()
                        .map(|block| match block {
                            VisibleBlock::Text(text) => ChatContent::Text { text: text.clone() },
                            VisibleBlock::Thinking(text) => {
                                ChatContent::Reasoning { text: text.clone() }
                            }
                            VisibleBlock::RedactedThinking => ChatContent::RedactedReasoning,
                        })
                        .collect(),
                },
                VisibleHistoryEntry::Compaction(compaction) => ChatHistoryEntry::Compaction {
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
        context_tokens: snapshot.context_tokens(),
        context_window,
        recovery_notices: snapshot
            .recovery_notices()
            .iter()
            .map(|notice| match notice {
                RecoveryNotice::IncompleteFinalTurnDiscarded => {
                    ChatRecoveryNotice::IncompleteFinalTurnDiscarded
                }
            })
            .collect(),
    }
}

fn project_event(event: AgentChatEvent, chat: ChatProjection) -> ChatEventKind {
    match event {
        AgentChatEvent::Started => ChatEventKind::Started,
        AgentChatEvent::ContentStarted { index, kind } => ChatEventKind::ContentStarted {
            index,
            kind: match kind {
                ContentKind::Text => ChatContentKind::Text,
                ContentKind::Reasoning => ChatContentKind::Reasoning,
            },
        },
        AgentChatEvent::ContentDelta { index, delta } => {
            ChatEventKind::ContentDelta { index, delta }
        }
        AgentChatEvent::ContentFinished { index } => ChatEventKind::ContentFinished { index },
        AgentChatEvent::Completed { .. } => ChatEventKind::Completed { chat },
        AgentChatEvent::Failed { error } => ChatEventKind::Failed {
            error: map_agent_error(error),
        },
        AgentChatEvent::Aborted => ChatEventKind::Aborted,
        AgentChatEvent::NotSaved { message, error } => ChatEventKind::NotSaved {
            response: project_message(&message),
            error: map_session_error(error.code()),
            chat,
        },
        AgentChatEvent::CompactionStarted { reason } => ChatEventKind::CompactionStarted {
            reason: compaction_reason(reason).to_owned(),
        },
        AgentChatEvent::CompactionCompleted { reason } => ChatEventKind::CompactionCompleted {
            reason: compaction_reason(reason).to_owned(),
            chat,
        },
        AgentChatEvent::CompactionCancelled { reason } => ChatEventKind::CompactionCancelled {
            reason: compaction_reason(reason).to_owned(),
        },
        AgentChatEvent::CompactionFailed { error } => ChatEventKind::CompactionFailed {
            error: map_agent_error(error),
        },
        AgentChatEvent::CompactionNotSaved { error } => ChatEventKind::CompactionNotSaved {
            error: map_session_error(error.code()),
            chat,
        },
    }
}

fn project_message(message: &AssistantMessage) -> Vec<ChatContent> {
    message
        .content()
        .iter()
        .map(|content| match content {
            AssistantContent::Text(text) => ChatContent::Text { text: text.clone() },
            AssistantContent::Reasoning(text) => ChatContent::Reasoning { text: text.clone() },
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

fn map_chat_error(error: AgentChatError) -> ChatError {
    match error {
        AgentChatError::Agent(error) => map_agent_error(error),
        AgentChatError::Session(error) => map_session_error(error.code()),
        AgentChatError::ModelUnavailable => model_unavailable(),
        AgentChatError::NotSaved => not_saved(),
    }
}

fn map_agent_error(error: AgentError) -> ChatError {
    match error.category {
        AgentErrorCategory::Authentication => ChatError {
            code: "authentication_unavailable",
            message: "AI provider authentication is unavailable",
        },
        AgentErrorCategory::InvalidConfiguration => ChatError {
            code: "invalid_configuration",
            message: "Agent Chat configuration is unavailable",
        },
        AgentErrorCategory::RateLimited => ChatError {
            code: "rate_limited",
            message: "AI provider rate limit reached",
        },
        AgentErrorCategory::Transport => ChatError {
            code: "transport_unavailable",
            message: "AI provider transport is unavailable",
        },
        AgentErrorCategory::ModelUnavailable => model_unavailable(),
        AgentErrorCategory::ContextOverflow => ChatError {
            code: "context_full",
            message: "Agent Chat context is full",
        },
        AgentErrorCategory::Provider => ChatError {
            code: "provider_failed",
            message: "AI provider request failed",
        },
    }
}

fn map_session_error(code: SessionErrorCode) -> ChatError {
    match code {
        SessionErrorCode::InvalidSessionId => invalid_request(),
        SessionErrorCode::NotFound => ChatError {
            code: "chat_not_found",
            message: "Agent Chat was not found",
        },
        SessionErrorCode::Locked => ChatError {
            code: "chat_read_only",
            message: "Agent Chat is read-only",
        },
        SessionErrorCode::Unsupported => ChatError {
            code: "chat_unsupported",
            message: "Agent Chat format is read-only",
        },
        SessionErrorCode::Damaged | SessionErrorCode::IncompleteFinalSuffix => ChatError {
            code: "chat_damaged",
            message: "Agent Chat data is damaged",
        },
        SessionErrorCode::NotSaved | SessionErrorCode::ExternalChange => not_saved(),
        SessionErrorCode::InvalidRoot
        | SessionErrorCode::SizeLimit
        | SessionErrorCode::TrashFailed => ChatError {
            code: "chat_unavailable",
            message: "Agent Chat is unavailable",
        },
    }
}

fn chat_not_open() -> ChatError {
    ChatError {
        code: "chat_not_open",
        message: "Agent Chat is not open",
    }
}

fn chat_busy() -> ChatError {
    ChatError {
        code: "chat_busy",
        message: "Agent Chat already has an active operation",
    }
}

fn invalid_request() -> ChatError {
    ChatError {
        code: "invalid_request",
        message: "Agent Chat request is invalid",
    }
}

fn model_unavailable() -> ChatError {
    ChatError {
        code: "model_unavailable",
        message: "Agent Model is unavailable",
    }
}

fn not_saved() -> ChatError {
    ChatError {
        code: "not_saved",
        message: "Agent Chat change was not saved",
    }
}
