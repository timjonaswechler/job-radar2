mod reconstruct;
mod storage;
mod wire;

use crate::agent::models::{ModelId, ProviderId, ReasoningLevel};
use std::{fmt, path::Path, str::FromStr, sync::Arc};
use uuid::Uuid;

pub use storage::SessionManager;

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(String);
impl SessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn uuid(&self) -> Uuid {
        Uuid::parse_str(&self.0).expect("validated session id")
    }
}
impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionId([redacted])")
    }
}
impl FromStr for SessionId {
    type Err = SessionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)
            .map_err(|_| SessionError::new(SessionErrorCode::InvalidSessionId))?;
        if id.get_version_num() != 7 || s != id.hyphenated().to_string() {
            return Err(SessionError::new(SessionErrorCode::InvalidSessionId));
        }
        Ok(Self(s.to_owned()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionAccess {
    Writable,
    ReadOnlyLocked,
    ReadOnlyUnsupported,
    Damaged,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryNotice {
    IncompleteFinalTurnDiscarded,
}
#[derive(Clone, Eq, PartialEq)]
pub enum VisibleBlock {
    Text(String),
    Thinking(String),
    RedactedThinking,
}
#[derive(Clone, Eq, PartialEq)]
pub struct VisibleTurn {
    user: String,
    assistant: Vec<VisibleBlock>,
}
impl fmt::Debug for VisibleBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(_) => f.write_str("Text([redacted])"),
            Self::Thinking(_) => f.write_str("Thinking([redacted])"),
            Self::RedactedThinking => f.write_str("RedactedThinking"),
        }
    }
}
impl fmt::Debug for VisibleTurn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisibleTurn")
            .field("user", &"[redacted]")
            .field("assistant_blocks", &self.assistant.len())
            .finish()
    }
}
impl VisibleTurn {
    pub fn user(&self) -> &str {
        &self.user
    }
    pub fn assistant(&self) -> &[VisibleBlock] {
        &self.assistant
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct CompactionSnapshot {
    summary: String,
    first_kept_entry_id: String,
    tokens_before: u64,
    reason: Option<String>,
}
impl fmt::Debug for CompactionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactionSnapshot")
            .field("summary", &"[redacted]")
            .field("tokens_before", &self.tokens_before)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
impl CompactionSnapshot {
    pub fn summary(&self) -> &str {
        &self.summary
    }
    pub fn tokens_before(&self) -> u64 {
        self.tokens_before
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
    pub fn first_kept_entry_id(&self) -> &str {
        &self.first_kept_entry_id
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct SessionSnapshot {
    id: SessionId,
    access: SessionAccess,
    display_name: String,
    created_at: String,
    modified_at: String,
    turns: Vec<VisibleTurn>,
    selected_provider: Option<ProviderId>,
    selected_model: Option<ModelId>,
    reasoning_level: ReasoningLevel,
    compactions: Vec<CompactionSnapshot>,
    recovery_notices: Vec<RecoveryNotice>,
}
impl fmt::Debug for SessionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSnapshot")
            .field("id", &self.id)
            .field("access", &self.access)
            .field("turn_count", &self.turns.len())
            .field("reasoning_level", &self.reasoning_level)
            .field("compaction_count", &self.compactions.len())
            .field("recovery_notices", &self.recovery_notices)
            .finish_non_exhaustive()
    }
}
impl SessionSnapshot {
    fn empty(id: SessionId, created_at: String, access: SessionAccess) -> Self {
        Self {
            id,
            access,
            display_name: "Untitled session".into(),
            created_at: created_at.clone(),
            modified_at: created_at,
            turns: Vec::new(),
            selected_provider: None,
            selected_model: None,
            reasoning_level: ReasoningLevel::Off,
            compactions: Vec::new(),
            recovery_notices: Vec::new(),
        }
    }
    pub fn id(&self) -> SessionId {
        self.id.clone()
    }
    pub fn access(&self) -> SessionAccess {
        self.access
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn created_at(&self) -> &str {
        &self.created_at
    }
    pub fn modified_at(&self) -> &str {
        &self.modified_at
    }
    pub fn turns(&self) -> &[VisibleTurn] {
        &self.turns
    }
    pub fn selected_provider(&self) -> Option<&ProviderId> {
        self.selected_provider.as_ref()
    }
    pub fn selected_model(&self) -> Option<&ModelId> {
        self.selected_model.as_ref()
    }
    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.reasoning_level
    }
    pub fn compactions(&self) -> &[CompactionSnapshot] {
        &self.compactions
    }
    pub fn recovery_notices(&self) -> &[RecoveryNotice] {
        &self.recovery_notices
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct SessionSummary {
    snapshot: SessionSnapshot,
}
impl fmt::Debug for SessionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSummary")
            .field("id", &self.snapshot.id)
            .field("access", &self.snapshot.access)
            .field("turn_count", &self.snapshot.turns.len())
            .finish_non_exhaustive()
    }
}
impl SessionSummary {
    pub fn id(&self) -> SessionId {
        self.snapshot.id()
    }
    pub fn access(&self) -> SessionAccess {
        self.snapshot.access()
    }
    pub fn display_name(&self) -> &str {
        self.snapshot.display_name()
    }
    pub fn created_at(&self) -> &str {
        self.snapshot.created_at()
    }
    pub fn modified_at(&self) -> &str {
        self.snapshot.modified_at()
    }
    pub fn turn_count(&self) -> usize {
        self.snapshot.turns.len()
    }
}

#[derive(Clone)]
pub struct AssistantBlock(pub(crate) AssistantBlockKind);
#[derive(Clone)]
pub(crate) enum AssistantBlockKind {
    Text {
        text: String,
        text_signature: Option<String>,
    },
    Thinking {
        thinking: String,
        thinking_signature: Option<String>,
        redacted: bool,
    },
}
impl AssistantBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self(AssistantBlockKind::Text {
            text: text.into(),
            text_signature: None,
        })
    }
    pub fn signed_text(text: impl Into<String>, signature: impl Into<String>) -> Self {
        Self(AssistantBlockKind::Text {
            text: text.into(),
            text_signature: Some(signature.into()),
        })
    }
    pub fn thinking(
        thinking: impl Into<String>,
        signature: Option<String>,
        redacted: bool,
    ) -> Self {
        Self(AssistantBlockKind::Thinking {
            thinking: thinking.into(),
            thinking_signature: signature,
            redacted,
        })
    }
}
impl fmt::Debug for AssistantBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            AssistantBlockKind::Text { .. } => f.write_str("Text([redacted])"),
            AssistantBlockKind::Thinking { redacted, .. } => f
                .debug_struct("Thinking")
                .field("redacted", redacted)
                .field("payload", &"[redacted]")
                .finish(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct UsageCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total: f64,
}
#[derive(Clone, Debug, Default)]
pub struct AssistantUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cache_write_1h: Option<u64>,
    pub reasoning: Option<u64>,
    pub total_tokens: u64,
    pub cost: UsageCost,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Stop,
    Length,
}
#[derive(Clone)]
pub struct CompletedTurn {
    user_text: String,
    assistant_blocks: Vec<AssistantBlock>,
    api: String,
    provider: ProviderId,
    model: ModelId,
    response_model: Option<String>,
    response_id: Option<String>,
    usage: AssistantUsage,
    stop_reason: StopReason,
}
impl CompletedTurn {
    pub fn new(
        user_text: impl Into<String>,
        assistant_blocks: Vec<AssistantBlock>,
        api: impl Into<String>,
        provider: ProviderId,
        model: ModelId,
        usage: AssistantUsage,
        stop_reason: StopReason,
    ) -> Self {
        Self {
            user_text: user_text.into(),
            assistant_blocks,
            api: api.into(),
            provider,
            model,
            response_model: None,
            response_id: None,
            usage,
            stop_reason,
        }
    }
    pub fn with_replay(
        mut self,
        response_model: Option<String>,
        response_id: Option<String>,
    ) -> Self {
        self.response_model = response_model;
        self.response_id = response_id;
        self
    }
}
impl fmt::Debug for CompletedTurn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletedTurn")
            .field("content", &"[redacted]")
            .field("provider", &"[redacted]")
            .field("model", &"[redacted]")
            .field("replay", &"[redacted]")
            .field("stop_reason", &self.stop_reason)
            .finish()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactionReason {
    Manual,
    Threshold,
    Overflow,
}
impl CompactionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Threshold => "threshold",
            Self::Overflow => "overflow",
        }
    }
}
#[derive(Clone)]
pub struct CompactionRecord {
    pub summary: String,
    pub first_kept_entry_id: String,
    pub tokens_before: u64,
    pub reason: Option<CompactionReason>,
}
impl fmt::Debug for CompactionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactionRecord")
            .field("summary", &"[redacted]")
            .field("first_kept_entry_id", &self.first_kept_entry_id)
            .field("tokens_before", &self.tokens_before)
            .field("reason", &self.reason)
            .finish()
    }
}
impl CompactionRecord {
    pub fn new(
        summary: impl Into<String>,
        first_kept_entry_id: impl Into<String>,
        tokens_before: u64,
        reason: Option<CompactionReason>,
    ) -> Self {
        Self {
            summary: summary.into(),
            first_kept_entry_id: first_kept_entry_id.into(),
            tokens_before,
            reason,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionErrorCode {
    InvalidRoot,
    InvalidSessionId,
    NotFound,
    Locked,
    Unsupported,
    Damaged,
    IncompleteFinalSuffix,
    SizeLimit,
    ExternalChange,
    NotSaved,
    TrashFailed,
}
#[derive(Clone, Eq, PartialEq)]
pub struct SessionError {
    code: SessionErrorCode,
    line: Option<u32>,
    entry_type: Option<&'static str>,
    field: Option<&'static str>,
}
impl SessionError {
    pub(crate) fn new(code: SessionErrorCode) -> Self {
        Self {
            code,
            line: None,
            entry_type: None,
            field: None,
        }
    }
    pub(crate) fn diagnostic(
        code: SessionErrorCode,
        line: Option<u32>,
        entry_type: Option<&str>,
        field: Option<&str>,
    ) -> Self {
        Self {
            code,
            line,
            entry_type: entry_type.and_then(known_label),
            field: field.and_then(known_label),
        }
    }
    pub fn code(&self) -> SessionErrorCode {
        self.code
    }
    pub fn line(&self) -> Option<u32> {
        self.line
    }
    pub fn entry_type(&self) -> Option<&str> {
        self.entry_type
    }
    pub fn field(&self) -> Option<&str> {
        self.field
    }
}
fn known_label(s: &str) -> Option<&'static str> {
    match s {
        "session" => Some("session"),
        "entry" => Some("entry"),
        "message" => Some("message"),
        "model_change" => Some("model_change"),
        "thinking_level_change" => Some("thinking_level_change"),
        "session_info" => Some("session_info"),
        "compaction" => Some("compaction"),
        "type" => Some("type"),
        "version" => Some("version"),
        "id" => Some("id"),
        "timestamp" => Some("timestamp"),
        "cwd" => Some("cwd"),
        "parentSession" => Some("parentSession"),
        "parentId" => Some("parentId"),
        "content" => Some("content"),
        "usage" => Some("usage"),
        "thinkingLevel" => Some("thinkingLevel"),
        "tokensBefore" => Some("tokensBefore"),
        "details" => Some("details"),
        "firstKeptEntryId" => Some("firstKeptEntryId"),
        _ => None,
    }
}
impl fmt::Debug for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionError")
            .field("code", &self.code)
            .field("line", &self.line)
            .field("entry_type", &self.entry_type)
            .field("field", &self.field)
            .finish()
    }
}
impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.code {
            SessionErrorCode::InvalidRoot => "session storage is unavailable",
            SessionErrorCode::InvalidSessionId => "invalid session identifier",
            SessionErrorCode::NotFound => "session was not found",
            SessionErrorCode::Locked => "session is read-only",
            SessionErrorCode::Unsupported => "session format is read-only",
            SessionErrorCode::Damaged => "session data is damaged",
            SessionErrorCode::IncompleteFinalSuffix => "session has an incomplete final write",
            SessionErrorCode::SizeLimit => "session data exceeds the supported limit",
            SessionErrorCode::ExternalChange => "session changed outside this writer",
            SessionErrorCode::NotSaved => "session change was not saved",
            SessionErrorCode::TrashFailed => "session could not be moved to Trash",
        })
    }
}
impl std::error::Error for SessionError {}

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum ContinuationBlock {
    User(String),
    Assistant {
        blocks: Vec<ContinuationAssistantBlock>,
        provider: String,
        model: String,
        response_id: Option<String>,
    },
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) enum ContinuationAssistantBlock {
    Text {
        text: String,
        signature: Option<String>,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
        redacted: bool,
    },
}

pub struct SessionHandle {
    pub(crate) manager: SessionManager,
    pub(crate) snapshot: SessionSnapshot,
    pub(crate) continuation: Vec<ContinuationBlock>,
    pub(crate) state: storage::HandleState,
    pub(crate) poisoned: bool,
}
impl SessionHandle {
    pub fn snapshot(&self) -> &SessionSnapshot {
        &self.snapshot
    }
    pub(crate) fn continuation(&self) -> &[ContinuationBlock] {
        &self.continuation
    }
    pub fn reload(&mut self) -> Result<(), SessionError> {
        storage::reload(self)
    }
    pub fn append_completed_turn(&mut self, turn: CompletedTurn) -> Result<(), SessionError> {
        storage::append_turn(self, turn)
    }
    pub fn select_model(
        &mut self,
        provider: ProviderId,
        model: ModelId,
    ) -> Result<(), SessionError> {
        storage::append_model(self, provider, model)
    }
    pub fn set_reasoning_level(&mut self, level: ReasoningLevel) -> Result<(), SessionError> {
        storage::append_reasoning(self, level)
    }
    pub fn set_name(&mut self, name: String) -> Result<(), SessionError> {
        storage::append_name(self, name)
    }
    pub fn append_compaction(&mut self, record: CompactionRecord) -> Result<(), SessionError> {
        storage::append_compaction(self, record)
    }
}
impl fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionHandle")
            .field("snapshot", &self.snapshot)
            .finish_non_exhaustive()
    }
}
impl Drop for SessionHandle {
    fn drop(&mut self) {
        if let storage::HandleState::Existing {
            owner: Some(owner), ..
        } = &self.state
        {
            let _ = fs2::FileExt::unlock(owner);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SessionCheckpoint {
    TempWrite,
    TempSync,
    Publish,
    DirectorySync,
    AppendWrite,
    AppendSync,
    Truncate,
    TruncateSync,
    Lock,
    Trash,
}
impl SessionCheckpoint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TempWrite => "temp-write",
            Self::TempSync => "temp-sync",
            Self::Publish => "publish",
            Self::DirectorySync => "directory-sync",
            Self::AppendWrite => "append-write",
            Self::AppendSync => "append-sync",
            Self::Truncate => "truncate",
            Self::TruncateSync => "truncate-sync",
            Self::Lock => "lock",
            Self::Trash => "trash",
        }
    }
}

pub(crate) trait Runtime: Send + Sync {
    fn now(&self) -> String;
    fn uuid(&self) -> Uuid;
    fn trash(&self, path: &Path) -> Result<(), ()>;
    fn checkpoint(&self, checkpoint: SessionCheckpoint) -> Result<(), ()> {
        let _ = checkpoint;
        Ok(())
    }
}
pub(crate) struct SystemRuntime;
impl Runtime for SystemRuntime {
    fn now(&self) -> String {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
    }
    fn uuid(&self) -> Uuid {
        Uuid::now_v7()
    }
    fn trash(&self, path: &Path) -> Result<(), ()> {
        trash::delete(path).map_err(|_| ())
    }
    fn checkpoint(&self, checkpoint: SessionCheckpoint) -> Result<(), ()> {
        use std::io::{Read, Write};
        if std::env::var("JOB_RADAR_SESSION_CHECKPOINT")
            .ok()
            .as_deref()
            == Some(checkpoint.as_str())
        {
            let mut stdout = std::io::stdout().lock();
            writeln!(stdout, "CHECKPOINT {}", checkpoint.as_str()).map_err(|_| ())?;
            stdout.flush().map_err(|_| ())?;
            let mut release = [0_u8; 1];
            std::io::stdin()
                .lock()
                .read_exact(&mut release)
                .map_err(|_| ())?;
        }
        Ok(())
    }
}
pub(crate) fn manager_with_runtime(
    root: &Path,
    runtime: Arc<dyn Runtime>,
) -> Result<SessionManager, SessionError> {
    SessionManager::with_runtime(root, runtime)
}
