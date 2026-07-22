use super::conversation::{BlockSignature, ReplayMetadata};
use super::models::{Model, ModelId, ProviderId, ReasoningLevel};
use super::sessions::{
    AssistantBlock, AssistantUsage, CompletedTurn, ContinuationAssistantBlock, ContinuationBlock,
    SessionAccess, SessionError, SessionErrorCode, SessionHandle, SessionId, SessionManager,
    SessionSnapshot, StopReason,
};
use super::{
    conversation::ConversationAttempt, AgentConversation, AgentError, AssistantContent,
    AssistantMessage, ContentKind, ConversationEvent, ConversationProvider, FinishReason, Message,
    TokenUsage, TurnCancellation, UserMessage,
};
use futures_util::Stream;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use super::compaction::{
    compaction_capable, prepare as prepare_compaction, requires_compaction, validate_split_summary,
    validate_summary, CompactionPreparation,
};
use super::sessions::{CompactionReason, CompactionRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentChatState {
    Ready,
    ModelUnavailable,
    ReadOnly,
    NotSaved,
}

#[derive(Clone, Debug)]
pub enum AgentChatError {
    Agent(AgentError),
    Session(SessionError),
    ModelUnavailable,
    NotSaved,
}

impl fmt::Display for AgentChatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Agent(error) => &error.message,
            Self::Session(error) => return error.fmt(formatter),
            Self::ModelUnavailable => "the selected model is unavailable",
            Self::NotSaved => "the previous turn was not saved",
        })
    }
}

impl std::error::Error for AgentChatError {}

impl From<AgentError> for AgentChatError {
    fn from(value: AgentError) -> Self {
        Self::Agent(value)
    }
}

impl From<SessionError> for AgentChatError {
    fn from(value: SessionError) -> Self {
        Self::Session(value)
    }
}

pub struct AgentChat {
    system_prompt: String,
    provider: Arc<dyn ConversationProvider>,
    session: SessionHandle,
    conversation: Option<AgentConversation>,
    not_saved: bool,
}

impl AgentChat {
    pub fn create(
        manager: &SessionManager,
        system_prompt: String,
        provider: impl ConversationProvider,
        selected_provider: ProviderId,
        selected_model: ModelId,
        reasoning: ReasoningLevel,
    ) -> Result<Self, AgentChatError> {
        let provider: Arc<dyn ConversationProvider> = Arc::new(provider);
        let mut session = manager.create()?;
        let model = find_model(provider.as_ref(), &selected_provider, &selected_model)
            .ok_or(AgentChatError::ModelUnavailable)?;
        let reasoning = model.normalize_reasoning(reasoning);
        if reasoning != ReasoningLevel::Off {
            session.set_reasoning_level(reasoning)?;
        }
        let conversation = AgentConversation::from_shared(
            system_prompt.clone(),
            Arc::clone(&provider),
            model.provider().clone(),
            model.id().clone(),
            reasoning,
            Vec::new(),
            session.snapshot().id().to_string(),
        )?;
        Ok(Self {
            system_prompt,
            provider,
            session,
            conversation: Some(conversation),
            not_saved: false,
        })
    }

    pub fn open(
        manager: &SessionManager,
        id: &SessionId,
        system_prompt: String,
        provider: impl ConversationProvider,
    ) -> Result<Self, AgentChatError> {
        let provider: Arc<dyn ConversationProvider> = Arc::new(provider);
        let session = manager.open(id)?;
        let conversation = build_conversation(&system_prompt, &provider, &session);
        Ok(Self {
            system_prompt,
            provider,
            session,
            conversation,
            not_saved: false,
        })
    }

    pub fn snapshot(&self) -> &SessionSnapshot {
        self.session.snapshot()
    }

    pub fn selected_provider(&self) -> Option<&ProviderId> {
        self.conversation
            .as_ref()
            .map(|conversation| conversation.model().provider())
            .or_else(|| self.session.snapshot().selected_provider())
    }

    pub fn selected_model(&self) -> Option<&ModelId> {
        self.conversation
            .as_ref()
            .map(|conversation| conversation.model().id())
            .or_else(|| self.session.snapshot().selected_model())
    }

    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.conversation
            .as_ref()
            .map(AgentConversation::reasoning_level)
            .unwrap_or_else(|| self.session.snapshot().reasoning_level())
    }

    pub fn state(&self) -> AgentChatState {
        if self.not_saved {
            AgentChatState::NotSaved
        } else if self.session.snapshot().access() != SessionAccess::Writable {
            AgentChatState::ReadOnly
        } else if self.conversation.is_none() {
            AgentChatState::ModelUnavailable
        } else {
            AgentChatState::Ready
        }
    }

    pub fn compact(
        &mut self,
        focus: Option<String>,
    ) -> Result<AgentChatEventStream<'_>, AgentChatError> {
        self.ensure_mutable()?;
        let conversation = self
            .conversation
            .as_mut()
            .ok_or(AgentChatError::ModelUnavailable)?;
        if !compaction_capable(conversation.model().context_window()) {
            return Err(AgentError::fixed(
                super::AgentErrorCategory::InvalidConfiguration,
                "the selected model is not compaction-capable",
            )
            .into());
        }
        let preparation = prepare_compaction(
            self.session.snapshot(),
            CompactionReason::Manual,
            focus.as_deref(),
        )
        .ok_or_else(|| {
            AgentError::fixed(
                super::AgentErrorCategory::InvalidConfiguration,
                "there is not enough history to compact",
            )
        })?;
        let cancellation = TurnCancellation::new();
        let attempt = conversation.begin_compaction(
            preparation.prompt().to_owned(),
            cancellation.clone(),
            13_107,
        );
        Ok(AgentChatEventStream {
            phase: ChatPhase::Compacting {
                attempt,
                preparation,
                intent: CompactionIntent::Finish,
                stage: SummaryStage::History,
            },
            session: &mut self.session,
            conversation,
            user_text: None,
            cancellation,
            not_saved: &mut self.not_saved,
            overflow_attempted: false,
            pending_completed: None,
            finished: false,
        })
    }

    pub fn send(&mut self, text: String) -> Result<AgentChatEventStream<'_>, AgentChatError> {
        match self.state() {
            AgentChatState::Ready => {}
            AgentChatState::ModelUnavailable => return Err(AgentChatError::ModelUnavailable),
            AgentChatState::NotSaved => return Err(AgentChatError::NotSaved),
            AgentChatState::ReadOnly => {
                return Err(SessionError::new(match self.session.snapshot().access() {
                    SessionAccess::ReadOnlyLocked => SessionErrorCode::Locked,
                    SessionAccess::ReadOnlyUnsupported => SessionErrorCode::Unsupported,
                    SessionAccess::Damaged => SessionErrorCode::Damaged,
                    SessionAccess::Writable => unreachable!(),
                })
                .into())
            }
        }
        let cancellation = TurnCancellation::new();
        let conversation = self
            .conversation
            .as_mut()
            .expect("ready chat has a conversation");
        let context_tokens = self.session.snapshot().context_tokens();
        let model = conversation.model();
        let phase = if requires_compaction(context_tokens, model.context_window()) {
            if !compaction_capable(model.context_window()) {
                return Err(AgentError::fixed(
                    super::AgentErrorCategory::InvalidConfiguration,
                    "the selected model context is full; select a model with a larger context window",
                ).into());
            }
            let preparation =
                prepare_compaction(self.session.snapshot(), CompactionReason::Threshold, None)
                    .ok_or_else(|| {
                        AgentError::fixed(
                            super::AgentErrorCategory::InvalidConfiguration,
                            "the selected model context cannot be compacted safely",
                        )
                    })?;
            let attempt = conversation.begin_compaction(
                preparation.prompt().to_owned(),
                cancellation.clone(),
                13_107,
            );
            ChatPhase::Compacting {
                attempt,
                preparation,
                intent: CompactionIntent::SendDraft,
                stage: SummaryStage::History,
            }
        } else {
            ChatPhase::Turning(conversation.begin_attempt(text.clone(), cancellation.clone()))
        };
        Ok(AgentChatEventStream {
            phase,
            session: &mut self.session,
            conversation,
            user_text: Some(text),
            cancellation,
            not_saved: &mut self.not_saved,
            overflow_attempted: false,
            pending_completed: None,
            finished: false,
        })
    }

    pub fn select_model(
        &mut self,
        provider: ProviderId,
        model: ModelId,
    ) -> Result<ReasoningLevel, AgentChatError> {
        self.ensure_mutable()?;
        let models = self.provider.model_snapshot();
        let selected = models
            .iter()
            .find(|candidate| candidate.provider() == &provider && candidate.id() == &model)
            .cloned()
            .ok_or(AgentChatError::ModelUnavailable)?;
        let current_reasoning = self
            .conversation
            .as_ref()
            .map(AgentConversation::reasoning_level)
            .unwrap_or_else(|| self.session.snapshot().reasoning_level());
        let effective = selected.normalize_reasoning(current_reasoning);
        if let Err(error) = self.session.select_model(provider.clone(), model.clone()) {
            self.not_saved = true;
            return Err(error.into());
        }
        if let Some(conversation) = &mut self.conversation {
            conversation.apply_model_snapshot(selected, models);
            conversation.set_reasoning_level(effective);
        } else {
            self.conversation =
                build_conversation(&self.system_prompt, &self.provider, &self.session);
        }
        Ok(effective)
    }

    pub fn set_reasoning_level(
        &mut self,
        requested: ReasoningLevel,
    ) -> Result<ReasoningLevel, AgentChatError> {
        self.ensure_mutable()?;
        let conversation = self
            .conversation
            .as_mut()
            .ok_or(AgentChatError::ModelUnavailable)?;
        let effective = conversation.model().normalize_reasoning(requested);
        if let Err(error) = self.session.set_reasoning_level(effective) {
            self.not_saved = true;
            return Err(error.into());
        }
        conversation.set_reasoning_level(effective);
        Ok(effective)
    }

    pub fn reload(&mut self) -> Result<(), AgentChatError> {
        self.session.reload()?;
        self.not_saved = false;
        if self.session.snapshot().selected_model().is_none()
            && self.session.snapshot().turns().is_empty()
        {
            if let Some(conversation) = &mut self.conversation {
                conversation.replace_messages(Vec::new());
            }
        } else {
            self.conversation =
                build_conversation(&self.system_prompt, &self.provider, &self.session);
        }
        Ok(())
    }

    fn ensure_mutable(&self) -> Result<(), AgentChatError> {
        match self.state() {
            AgentChatState::Ready | AgentChatState::ModelUnavailable => Ok(()),
            AgentChatState::NotSaved => Err(AgentChatError::NotSaved),
            AgentChatState::ReadOnly => {
                Err(SessionError::new(match self.session.snapshot().access() {
                    SessionAccess::ReadOnlyLocked => SessionErrorCode::Locked,
                    SessionAccess::ReadOnlyUnsupported => SessionErrorCode::Unsupported,
                    SessionAccess::Damaged => SessionErrorCode::Damaged,
                    SessionAccess::Writable => unreachable!(),
                })
                .into())
            }
        }
    }
}

fn find_model(
    provider: &dyn ConversationProvider,
    provider_id: &ProviderId,
    model_id: &ModelId,
) -> Option<Model> {
    provider
        .model_snapshot()
        .into_iter()
        .find(|model| model.provider() == provider_id && model.id() == model_id)
}

fn build_conversation(
    system_prompt: &str,
    provider: &Arc<dyn ConversationProvider>,
    session: &SessionHandle,
) -> Option<AgentConversation> {
    if session.snapshot().access() != SessionAccess::Writable {
        return None;
    }
    let provider_id = session.snapshot().selected_provider()?;
    let model_id = session.snapshot().selected_model()?;
    let model = find_model(provider.as_ref(), provider_id, model_id)?;
    let messages = hydrate_messages(session.continuation(), &model, provider.as_ref());
    AgentConversation::from_shared(
        system_prompt.to_owned(),
        Arc::clone(provider),
        model.provider().clone(),
        model.id().clone(),
        session.snapshot().reasoning_level(),
        messages,
        session.snapshot().id().to_string(),
    )
    .ok()
}

fn hydrate_messages(
    continuation: &[ContinuationBlock],
    selected_model: &Model,
    provider: &dyn ConversationProvider,
) -> Vec<Message> {
    continuation
        .iter()
        .map(|block| match block {
            ContinuationBlock::User(text) => Message::User(UserMessage::new(text.clone())),
            ContinuationBlock::Assistant {
                blocks,
                provider: historical_provider,
                model: historical_model,
                response_id,
            } => {
                let mut content = Vec::with_capacity(blocks.len());
                let mut signatures = Vec::with_capacity(blocks.len());
                for block in blocks {
                    match block {
                        ContinuationAssistantBlock::Text { text, signature } => {
                            content.push(AssistantContent::Text(text.clone()));
                            signatures.push(signature.clone().map(BlockSignature::Text));
                        }
                        ContinuationAssistantBlock::Thinking {
                            thinking,
                            signature,
                            redacted,
                        } => {
                            content.push(AssistantContent::Reasoning(if *redacted {
                                String::new()
                            } else {
                                thinking.clone()
                            }));
                            signatures.push(signature.clone().map(|signature| {
                                BlockSignature::Reasoning {
                                    signature,
                                    redacted: *redacted,
                                }
                            }));
                        }
                    }
                }
                let historical_model = ProviderId::new(historical_provider.clone())
                    .ok()
                    .and_then(|provider_id| {
                        ModelId::new(historical_model.clone())
                            .ok()
                            .map(|model_id| (provider_id, model_id))
                    })
                    .and_then(|(provider_id, model_id)| {
                        find_model(provider, &provider_id, &model_id).or_else(|| {
                            Model::new(
                                model_id,
                                "Unavailable historical model",
                                provider_id,
                                vec![ReasoningLevel::Off],
                            )
                            .ok()
                        })
                    })
                    .unwrap_or_else(|| selected_model.clone());
                Message::Assistant(AssistantMessage::from_replay(
                    content,
                    historical_model,
                    TokenUsage::default(),
                    FinishReason::Completed,
                    ReplayMetadata {
                        response_id: response_id.clone(),
                        block_signatures: signatures,
                    },
                ))
            }
        })
        .collect()
}

#[derive(Clone)]
pub enum AgentChatEvent {
    Started,
    ContentStarted {
        index: usize,
        kind: ContentKind,
    },
    ContentDelta {
        index: usize,
        delta: String,
    },
    ContentFinished {
        index: usize,
    },
    Completed {
        message: AssistantMessage,
    },
    Failed {
        error: AgentError,
    },
    Aborted,
    NotSaved {
        message: AssistantMessage,
        error: SessionError,
    },
    CompactionStarted {
        reason: CompactionReason,
    },
    CompactionCompleted {
        reason: CompactionReason,
    },
    CompactionCancelled {
        reason: CompactionReason,
    },
    CompactionFailed {
        error: AgentError,
    },
    CompactionNotSaved {
        error: SessionError,
    },
}

impl fmt::Debug for AgentChatEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started => formatter.write_str("Started"),
            Self::ContentStarted { index, kind } => formatter
                .debug_struct("ContentStarted")
                .field("index", index)
                .field("kind", kind)
                .finish(),
            Self::ContentDelta { index, .. } => formatter
                .debug_struct("ContentDelta")
                .field("index", index)
                .field("delta", &"[redacted]")
                .finish(),
            Self::ContentFinished { index } => formatter
                .debug_struct("ContentFinished")
                .field("index", index)
                .finish(),
            Self::Completed { .. } => formatter.write_str("Completed { message: [redacted] }"),
            Self::Failed { error } => formatter
                .debug_struct("Failed")
                .field("error", error)
                .finish(),
            Self::Aborted => formatter.write_str("Aborted"),
            Self::NotSaved { error, .. } => formatter
                .debug_struct("NotSaved")
                .field("message", &"[redacted]")
                .field("error", error)
                .finish(),
            Self::CompactionStarted { reason } => formatter
                .debug_struct("CompactionStarted")
                .field("reason", reason)
                .finish(),
            Self::CompactionCompleted { reason } => formatter
                .debug_struct("CompactionCompleted")
                .field("reason", reason)
                .finish(),
            Self::CompactionCancelled { reason } => formatter
                .debug_struct("CompactionCancelled")
                .field("reason", reason)
                .finish(),
            Self::CompactionFailed { error } => formatter
                .debug_struct("CompactionFailed")
                .field("error", error)
                .finish(),
            Self::CompactionNotSaved { error } => formatter
                .debug_struct("CompactionNotSaved")
                .field("error", error)
                .finish(),
        }
    }
}

#[derive(Clone, Copy)]
enum CompactionIntent {
    Finish,
    SendDraft,
    RetryOverflow,
}

enum SummaryStage {
    History,
    SplitPrefix(String),
}

enum ChatPhase {
    Compacting {
        attempt: ConversationAttempt,
        preparation: CompactionPreparation,
        intent: CompactionIntent,
        stage: SummaryStage,
    },
    Turning(ConversationAttempt),
    Finished,
}

pub struct AgentChatEventStream<'a> {
    phase: ChatPhase,
    session: &'a mut SessionHandle,
    conversation: &'a mut AgentConversation,
    user_text: Option<String>,
    cancellation: TurnCancellation,
    not_saved: &'a mut bool,
    overflow_attempted: bool,
    pending_completed: Option<AssistantMessage>,
    finished: bool,
}

impl AgentChatEventStream<'_> {
    pub fn cancellation(&self) -> TurnCancellation {
        self.cancellation.clone()
    }

    pub fn snapshot(&self) -> &SessionSnapshot {
        self.session.snapshot()
    }

    pub fn state(&self) -> AgentChatState {
        if *self.not_saved {
            AgentChatState::NotSaved
        } else if self.session.snapshot().access() != SessionAccess::Writable {
            AgentChatState::ReadOnly
        } else {
            AgentChatState::Ready
        }
    }

    pub fn selected_provider(&self) -> &ProviderId {
        self.conversation.model().provider()
    }

    pub fn selected_model(&self) -> &ModelId {
        self.conversation.model().id()
    }

    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.conversation.reasoning_level()
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn fail_compaction(&mut self, error: AgentError) -> Poll<Option<AgentChatEvent>> {
        self.phase = ChatPhase::Finished;
        self.finished = self.pending_completed.is_none();
        Poll::Ready(Some(AgentChatEvent::CompactionFailed { error }))
    }

    fn cancel_compaction(&mut self, reason: CompactionReason) -> Poll<Option<AgentChatEvent>> {
        self.phase = ChatPhase::Finished;
        self.finished = self.pending_completed.is_none();
        Poll::Ready(Some(AgentChatEvent::CompactionCancelled { reason }))
    }

    fn start_turn(&mut self) {
        let text = self.user_text.clone().expect("send compaction has a draft");
        self.phase = ChatPhase::Turning(
            self.conversation
                .begin_attempt(text, self.cancellation.clone()),
        );
    }
}

impl Stream for AgentChatEventStream<'_> {
    type Item = AgentChatEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.finished {
                return Poll::Ready(None);
            }
            if matches!(self.phase, ChatPhase::Finished) {
                if let Some(message) = self.pending_completed.take() {
                    self.finished = true;
                    return Poll::Ready(Some(AgentChatEvent::Completed { message }));
                }
                self.finished = true;
                return Poll::Ready(None);
            }
            if self.cancellation.is_cancelled() {
                if let ChatPhase::Compacting { preparation, .. } = &self.phase {
                    let reason = preparation.reason();
                    return self.cancel_compaction(reason);
                }
                self.phase = ChatPhase::Finished;
                self.finished = self.pending_completed.is_none();
                return Poll::Ready(Some(AgentChatEvent::Aborted));
            }
            let event = match &mut self.phase {
                ChatPhase::Compacting { attempt, .. } | ChatPhase::Turning(attempt) => {
                    match Pin::new(attempt).poll_next(context) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(None) => {
                            self.finished = true;
                            return Poll::Ready(None);
                        }
                        Poll::Ready(Some(event)) => event,
                    }
                }
                ChatPhase::Finished => return Poll::Ready(None),
            };

            if matches!(self.phase, ChatPhase::Compacting { .. }) {
                let reason = match &self.phase {
                    ChatPhase::Compacting { preparation, .. } => preparation.reason(),
                    _ => unreachable!(),
                };
                match event {
                    ConversationEvent::Started => {
                        return Poll::Ready(Some(AgentChatEvent::CompactionStarted { reason }));
                    }
                    ConversationEvent::ContentStarted { .. }
                    | ConversationEvent::ContentDelta { .. }
                    | ConversationEvent::ContentFinished { .. } => continue,
                    ConversationEvent::Aborted => return self.cancel_compaction(reason),
                    ConversationEvent::Failed { error } => return self.fail_compaction(error),
                    ConversationEvent::Completed { message } => {
                        if message.finish_reason() != FinishReason::Completed {
                            return self.fail_compaction(AgentError::fixed(
                                super::AgentErrorCategory::Provider,
                                "compaction summary was truncated",
                            ));
                        }
                        let summary = message
                            .content()
                            .iter()
                            .filter_map(|block| match block {
                                AssistantContent::Text(text) => Some(text.as_str()),
                                AssistantContent::Reasoning(_) => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        let (preparation, intent, history_summary) = match &self.phase {
                            ChatPhase::Compacting {
                                preparation,
                                intent,
                                stage: SummaryStage::History,
                                ..
                            } => {
                                if !validate_summary(&summary) {
                                    return self.fail_compaction(AgentError::fixed(
                                        super::AgentErrorCategory::Provider,
                                        "compaction summary is malformed",
                                    ));
                                }
                                if let Some(prompt) = preparation.split_prefix_prompt() {
                                    let attempt = self.conversation.begin_compaction(
                                        prompt.to_owned(),
                                        self.cancellation.clone(),
                                        8_192,
                                    );
                                    self.phase = ChatPhase::Compacting {
                                        attempt,
                                        preparation: preparation.clone(),
                                        intent: *intent,
                                        stage: SummaryStage::SplitPrefix(summary),
                                    };
                                    continue;
                                }
                                (preparation.clone(), *intent, summary)
                            }
                            ChatPhase::Compacting {
                                preparation,
                                intent,
                                stage: SummaryStage::SplitPrefix(history),
                                ..
                            } => {
                                if !validate_split_summary(&summary) {
                                    return self.fail_compaction(AgentError::fixed(
                                        super::AgentErrorCategory::Provider,
                                        "split-turn summary is malformed",
                                    ));
                                }
                                (
                                    preparation.clone(),
                                    *intent,
                                    format!("{history}\n\n---\n\n**Turn Context (split turn):**\n\n{summary}"),
                                )
                            }
                            _ => unreachable!(),
                        };
                        if self.cancellation.is_cancelled() {
                            return self.cancel_compaction(preparation.reason());
                        }
                        let record = CompactionRecord::new(
                            history_summary,
                            preparation.first_kept_entry_id().to_owned(),
                            preparation.tokens_before(),
                            Some(preparation.reason()),
                        );
                        if let Err(error) = self.session.append_compaction(record) {
                            *self.not_saved = true;
                            self.phase = ChatPhase::Finished;
                            self.finished = self.pending_completed.is_none();
                            return Poll::Ready(Some(AgentChatEvent::CompactionNotSaved { error }));
                        }
                        let messages = hydrate_messages(
                            self.session.continuation(),
                            self.conversation.model(),
                            self.conversation.provider(),
                        );
                        self.conversation.replace_messages(messages);
                        match intent {
                            CompactionIntent::SendDraft | CompactionIntent::RetryOverflow => {
                                self.start_turn()
                            }
                            CompactionIntent::Finish => {
                                self.phase = ChatPhase::Finished;
                                self.finished = self.pending_completed.is_none();
                            }
                        }
                        return Poll::Ready(Some(AgentChatEvent::CompactionCompleted { reason }));
                    }
                }
            }

            match event {
                ConversationEvent::Started => return Poll::Ready(Some(AgentChatEvent::Started)),
                ConversationEvent::ContentStarted { index, kind } => {
                    return Poll::Ready(Some(AgentChatEvent::ContentStarted { index, kind }));
                }
                ConversationEvent::ContentDelta { index, delta } => {
                    return Poll::Ready(Some(AgentChatEvent::ContentDelta { index, delta }));
                }
                ConversationEvent::ContentFinished { index } => {
                    return Poll::Ready(Some(AgentChatEvent::ContentFinished { index }));
                }
                ConversationEvent::Aborted => {
                    self.finished = true;
                    self.phase = ChatPhase::Finished;
                    return Poll::Ready(Some(AgentChatEvent::Aborted));
                }
                ConversationEvent::Failed { error }
                    if error.is_context_overflow() && !self.overflow_attempted =>
                {
                    self.overflow_attempted = true;
                    let model = self.conversation.model();
                    if !compaction_capable(model.context_window()) {
                        self.finished = true;
                        self.phase = ChatPhase::Finished;
                        return Poll::Ready(Some(AgentChatEvent::Failed { error }));
                    }
                    let Some(preparation) = prepare_compaction(
                        self.session.snapshot(),
                        CompactionReason::Overflow,
                        None,
                    ) else {
                        self.finished = true;
                        self.phase = ChatPhase::Finished;
                        return Poll::Ready(Some(AgentChatEvent::Failed { error }));
                    };
                    let attempt = self.conversation.begin_compaction(
                        preparation.prompt().to_owned(),
                        self.cancellation.clone(),
                        13_107,
                    );
                    self.phase = ChatPhase::Compacting {
                        attempt,
                        preparation,
                        intent: CompactionIntent::RetryOverflow,
                        stage: SummaryStage::History,
                    };
                    continue;
                }
                ConversationEvent::Failed { error } => {
                    self.finished = true;
                    self.phase = ChatPhase::Finished;
                    return Poll::Ready(Some(AgentChatEvent::Failed { error }));
                }
                ConversationEvent::Completed { message } => {
                    let user_text = self.user_text.clone().expect("turn has user draft");
                    let turn = completed_turn(&user_text, &message);
                    if let Err(error) = self.session.append_completed_turn(turn) {
                        *self.not_saved = true;
                        self.finished = true;
                        self.phase = ChatPhase::Finished;
                        return Poll::Ready(Some(AgentChatEvent::NotSaved { message, error }));
                    }
                    self.conversation
                        .commit(UserMessage::new(user_text), message.clone());
                    let model = self.conversation.model();
                    if requires_compaction(
                        self.session.snapshot().context_tokens(),
                        model.context_window(),
                    ) && compaction_capable(model.context_window())
                    {
                        if let Some(preparation) = prepare_compaction(
                            self.session.snapshot(),
                            CompactionReason::Threshold,
                            None,
                        ) {
                            let attempt = self.conversation.begin_compaction(
                                preparation.prompt().to_owned(),
                                self.cancellation.clone(),
                                13_107,
                            );
                            self.pending_completed = Some(message);
                            self.phase = ChatPhase::Compacting {
                                attempt,
                                preparation,
                                intent: CompactionIntent::Finish,
                                stage: SummaryStage::History,
                            };
                            continue;
                        }
                    }
                    self.finished = true;
                    self.phase = ChatPhase::Finished;
                    return Poll::Ready(Some(AgentChatEvent::Completed { message }));
                }
            }
        }
    }
}

fn completed_turn(user_text: &str, message: &AssistantMessage) -> CompletedTurn {
    let replay = message.replay_metadata();
    let blocks = message
        .content()
        .iter()
        .enumerate()
        .map(|(index, content)| {
            let signature = replay.block_signatures.get(index).and_then(Option::as_ref);
            match content {
                AssistantContent::Text(text) => match signature {
                    Some(BlockSignature::Text(signature)) => {
                        AssistantBlock::signed_text(text.clone(), signature.clone())
                    }
                    _ => AssistantBlock::text(text.clone()),
                },
                AssistantContent::Reasoning(thinking) => match signature {
                    Some(BlockSignature::Reasoning {
                        signature,
                        redacted,
                    }) => AssistantBlock::thinking(
                        thinking.clone(),
                        Some(signature.clone()),
                        *redacted,
                    ),
                    _ => AssistantBlock::thinking(thinking.clone(), None, false),
                },
            }
        })
        .collect();
    let usage = message.usage();
    CompletedTurn::new(
        user_text,
        blocks,
        message.model().api().as_str(),
        message.model().provider().clone(),
        message.model().id().clone(),
        AssistantUsage {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_write,
            cache_write_1h: None,
            reasoning: usage.reasoning,
            total_tokens: usage.total,
            ..AssistantUsage::default()
        },
        match message.finish_reason() {
            FinishReason::Completed => StopReason::Stop,
            FinishReason::LengthLimit => StopReason::Length,
        },
    )
    .with_replay(None, replay.response_id.clone())
}
