use super::conversation::{BlockSignature, ReplayMetadata};
use super::models::{Model, ModelId, ProviderId, ReasoningLevel};
use super::sessions::{
    AssistantBlock, AssistantUsage, CompletedTurn, ContinuationAssistantBlock, ContinuationBlock,
    SessionAccess, SessionError, SessionErrorCode, SessionHandle, SessionId, SessionManager,
    SessionSnapshot, StopReason,
};
use super::{
    AgentConversation, AgentError, AssistantContent, AssistantMessage, ContentKind,
    ConversationEvent, ConversationEventStream, ConversationProvider, FinishReason, Message,
    TokenUsage, TurnCancellation, UserMessage,
};
use futures_util::Stream;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

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
        let Self {
            session,
            conversation,
            not_saved,
            ..
        } = self;
        let stream = conversation
            .as_mut()
            .expect("ready chat has a conversation")
            .send(text.clone())?;
        Ok(AgentChatEventStream {
            stream,
            session,
            user_text: text,
            not_saved,
            finished: false,
        })
    }

    pub fn select_model(
        &mut self,
        provider: ProviderId,
        model: ModelId,
    ) -> Result<ReasoningLevel, AgentChatError> {
        self.ensure_mutable()?;
        let selected = find_model(self.provider.as_ref(), &provider, &model)
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
            conversation.select_provider_model(&provider, model)?;
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

fn find_model<'a>(
    provider: &'a dyn ConversationProvider,
    provider_id: &ProviderId,
    model_id: &ModelId,
) -> Option<&'a Model> {
    provider
        .models()
        .iter()
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
    let model = find_model(provider.as_ref(), provider_id, model_id)?.clone();
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
                        find_model(provider, &provider_id, &model_id)
                            .cloned()
                            .or_else(|| {
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
        }
    }
}

pub struct AgentChatEventStream<'a> {
    stream: ConversationEventStream<'a>,
    session: &'a mut SessionHandle,
    user_text: String,
    not_saved: &'a mut bool,
    finished: bool,
}

impl AgentChatEventStream<'_> {
    pub fn cancellation(&self) -> TurnCancellation {
        self.stream.cancellation()
    }
}

impl Stream for AgentChatEventStream<'_> {
    type Item = AgentChatEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        let event = match Pin::new(&mut self.stream).poll_next(context) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(None) => {
                self.finished = true;
                return Poll::Ready(None);
            }
            Poll::Ready(Some(event)) => event,
        };
        let public = match event {
            ConversationEvent::Started => AgentChatEvent::Started,
            ConversationEvent::ContentStarted { index, kind } => {
                AgentChatEvent::ContentStarted { index, kind }
            }
            ConversationEvent::ContentDelta { index, delta } => {
                AgentChatEvent::ContentDelta { index, delta }
            }
            ConversationEvent::ContentFinished { index } => {
                AgentChatEvent::ContentFinished { index }
            }
            ConversationEvent::Failed { error } => {
                self.finished = true;
                AgentChatEvent::Failed { error }
            }
            ConversationEvent::Aborted => {
                self.finished = true;
                AgentChatEvent::Aborted
            }
            ConversationEvent::Completed { message } => {
                let turn = completed_turn(&self.user_text, &message);
                self.finished = true;
                match self.session.append_completed_turn(turn) {
                    Ok(()) => AgentChatEvent::Completed { message },
                    Err(error) => {
                        *self.not_saved = true;
                        AgentChatEvent::NotSaved { message, error }
                    }
                }
            }
        };
        Poll::Ready(Some(public))
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
