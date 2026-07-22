use crate::agent::models::{Model, ModelId, ReasoningLevel};
use crate::agent::{AgentError, AgentErrorCategory};
use futures_util::task::AtomicWaker;
use futures_util::Stream;
use std::fmt;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::task::{Context, Poll};

pub type ProviderEventStream = Pin<Box<dyn Stream<Item = ProviderEvent> + Send + 'static>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserMessage {
    text: String,
}

impl UserMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssistantContent {
    Text(String),
    Reasoning(String),
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ReplayMetadata {
    pub(crate) response_id: Option<String>,
    pub(crate) block_signatures: Vec<Option<BlockSignature>>,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum BlockSignature {
    Text(String),
    Reasoning { signature: String, redacted: bool },
}

impl ReplayMetadata {
    fn empty() -> Self {
        Self {
            response_id: None,
            block_signatures: Vec::new(),
        }
    }
}

impl fmt::Debug for ReplayMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReplayMetadata(<opaque>)")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: Option<u64>,
    pub total: u64,
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            reasoning: None,
            total: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishReason {
    Completed,
    LengthLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssistantMessage {
    content: Vec<AssistantContent>,
    model: Model,
    usage: TokenUsage,
    finish_reason: FinishReason,
    replay: ReplayMetadata,
}

impl AssistantMessage {
    pub(crate) fn synthetic(
        content: Vec<AssistantContent>,
        model: Model,
        usage: TokenUsage,
        finish_reason: FinishReason,
    ) -> Self {
        Self::from_replay(
            content,
            model,
            usage,
            finish_reason,
            ReplayMetadata::empty(),
        )
    }

    pub(crate) fn from_replay(
        content: Vec<AssistantContent>,
        model: Model,
        usage: TokenUsage,
        finish_reason: FinishReason,
        mut replay: ReplayMetadata,
    ) -> Self {
        while replay.block_signatures.last().is_some_and(Option::is_none) {
            replay.block_signatures.pop();
        }
        Self {
            content,
            model,
            usage,
            finish_reason,
            replay,
        }
    }

    pub fn content(&self) -> &[AssistantContent] {
        &self.content
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn usage(&self) -> &TokenUsage {
        &self.usage
    }

    pub fn finish_reason(&self) -> FinishReason {
        self.finish_reason
    }

    pub(crate) fn replay_metadata(&self) -> &ReplayMetadata {
        &self.replay
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    Text,
    Reasoning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTurnCompletion {
    usage: TokenUsage,
    finish_reason: FinishReason,
    replay: ReplayMetadata,
}

impl ProviderTurnCompletion {
    pub fn new(usage: TokenUsage, finish_reason: FinishReason) -> Self {
        Self {
            usage,
            finish_reason,
            replay: ReplayMetadata::empty(),
        }
    }

    pub(crate) fn with_replay(
        usage: TokenUsage,
        finish_reason: FinishReason,
        response_id: Option<String>,
        block_signatures: Vec<Option<BlockSignature>>,
    ) -> Self {
        Self {
            usage,
            finish_reason,
            replay: ReplayMetadata {
                response_id,
                block_signatures,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderEvent {
    Started,
    ContentStarted { index: usize, kind: ContentKind },
    ContentDelta { index: usize, delta: String },
    ContentFinished { index: usize },
    Completed(ProviderTurnCompletion),
    Failed(AgentError),
    Aborted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationEvent {
    Started,
    ContentStarted { index: usize, kind: ContentKind },
    ContentDelta { index: usize, delta: String },
    ContentFinished { index: usize },
    Completed { message: AssistantMessage },
    Failed { error: AgentError },
    Aborted,
}

#[derive(Clone)]
pub struct TurnCancellation {
    inner: Arc<TurnCancellationInner>,
}

struct TurnCancellationInner {
    cancelled: AtomicBool,
    stream_waker: AtomicWaker,
    provider_waiters: tokio::sync::Notify,
}

impl TurnCancellation {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(TurnCancellationInner {
                cancelled: AtomicBool::new(false),
                stream_waker: AtomicWaker::new(),
                provider_waiters: tokio::sync::Notify::new(),
            }),
        }
    }

    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        self.inner.stream_waker.wake();
        self.inner.provider_waiters.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let notified = self.inner.provider_waiters.notified();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

impl fmt::Debug for TurnCancellation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TurnCancellation")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[derive(Clone)]
pub struct ConversationRequest {
    system_prompt: String,
    messages: Vec<Message>,
    model: Model,
    reasoning: ReasoningLevel,
    conversation_id: String,
    cancellation: TurnCancellation,
}

impl ConversationRequest {
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.reasoning
    }

    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn cancellation(&self) -> TurnCancellation {
        self.cancellation.clone()
    }
}

pub trait ConversationProvider: Send + Sync + 'static {
    fn models(&self) -> &[Model];

    fn model_snapshot(&self) -> Vec<Model> {
        self.models().to_vec()
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream;
}

pub struct AgentConversation {
    system_prompt: String,
    provider: Arc<dyn ConversationProvider>,
    models: Vec<Model>,
    model: Model,
    reasoning: ReasoningLevel,
    messages: Vec<Message>,
    conversation_id: String,
}

impl AgentConversation {
    pub fn new(
        system_prompt: String,
        provider: impl ConversationProvider,
        model: ModelId,
        reasoning: ReasoningLevel,
    ) -> Result<Self, AgentError> {
        let provider = Arc::new(provider);
        let provider_id = provider
            .model_snapshot()
            .iter()
            .find(|candidate| candidate.id() == &model)
            .map(|candidate| candidate.provider().clone())
            .ok_or_else(AgentError::model_unavailable)?;
        Self::from_shared(
            system_prompt,
            provider,
            provider_id,
            model,
            reasoning,
            Vec::new(),
            uuid::Uuid::new_v4().to_string(),
        )
    }

    pub(crate) fn from_shared(
        system_prompt: String,
        provider: Arc<dyn ConversationProvider>,
        provider_id: crate::agent::models::ProviderId,
        model: ModelId,
        reasoning: ReasoningLevel,
        messages: Vec<Message>,
        conversation_id: String,
    ) -> Result<Self, AgentError> {
        let models = provider.model_snapshot();
        let selected = models
            .iter()
            .find(|candidate| candidate.provider() == &provider_id && candidate.id() == &model)
            .cloned()
            .ok_or_else(AgentError::model_unavailable)?;
        let reasoning = selected.normalize_reasoning(reasoning);
        Ok(Self {
            system_prompt,
            provider,
            models,
            model: selected,
            reasoning,
            messages,
            conversation_id,
        })
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub(crate) fn replace_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    pub fn available_models(&self) -> &[Model] {
        &self.models
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub(crate) fn provider(&self) -> &dyn ConversationProvider {
        self.provider.as_ref()
    }

    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.reasoning
    }

    pub fn select_model(&mut self, model: ModelId) -> Result<(), AgentError> {
        let provider = self.model.provider().clone();
        self.select_provider_model(&provider, model)
    }

    pub(crate) fn select_provider_model(
        &mut self,
        provider: &crate::agent::models::ProviderId,
        model: ModelId,
    ) -> Result<(), AgentError> {
        let models = self.provider.model_snapshot();
        let selected = models
            .iter()
            .find(|candidate| candidate.provider() == provider && candidate.id() == &model)
            .cloned()
            .ok_or_else(AgentError::model_unavailable)?;
        self.apply_model_snapshot(selected, models);
        Ok(())
    }

    pub(crate) fn apply_model_snapshot(&mut self, selected: Model, models: Vec<Model>) {
        self.reasoning = selected.normalize_reasoning(self.reasoning);
        self.model = selected;
        self.models = models;
    }

    pub fn set_reasoning_level(&mut self, level: ReasoningLevel) -> ReasoningLevel {
        self.reasoning = self.model.normalize_reasoning(level);
        self.reasoning
    }

    pub(crate) fn begin_attempt(
        &self,
        text: String,
        cancellation: TurnCancellation,
    ) -> ConversationAttempt {
        let user = UserMessage::new(text);
        let mut messages = self.messages.clone();
        messages.push(Message::User(user.clone()));
        let request = ConversationRequest {
            system_prompt: self.system_prompt.clone(),
            messages,
            model: self.model.clone(),
            reasoning: self.reasoning,
            conversation_id: self.conversation_id.clone(),
            cancellation: cancellation.clone(),
        };
        ConversationAttempt {
            provider_stream: self.provider.stream(request),
            model: self.model.clone(),
            state: AttemptState::AwaitStarted,
            blocks: Vec::new(),
            active: None,
            cancellation,
        }
    }

    pub(crate) fn commit(&mut self, user: UserMessage, message: AssistantMessage) {
        self.messages.push(Message::User(user));
        self.messages.push(Message::Assistant(message));
    }

    pub(crate) fn begin_compaction(
        &self,
        prompt: String,
        cancellation: TurnCancellation,
        output_cap: u64,
    ) -> ConversationAttempt {
        let mut summary_model = self.model.clone();
        let capped_output = summary_model.max_tokens().min(output_cap);
        *summary_model.parts_mut().max_tokens = capped_output;
        let request = ConversationRequest {
            system_prompt: "You summarize prior conversation context. Do not answer or continue the conversation.".to_owned(),
            messages: vec![Message::User(UserMessage::new(prompt))],
            model: summary_model.clone(),
            reasoning: self.reasoning,
            conversation_id: self.conversation_id.clone(),
            cancellation: cancellation.clone(),
        };
        ConversationAttempt {
            provider_stream: self.provider.stream(request),
            model: summary_model,
            state: AttemptState::AwaitStarted,
            blocks: Vec::new(),
            active: None,
            cancellation,
        }
    }

    pub fn send(&mut self, text: String) -> Result<ConversationEventStream<'_>, AgentError> {
        let user = UserMessage::new(text.clone());
        let cancellation = TurnCancellation::new();
        let attempt = self.begin_attempt(text, cancellation);
        Ok(ConversationEventStream {
            conversation: self,
            attempt,
            user,
        })
    }
}

pub(crate) struct ConversationAttempt {
    provider_stream: ProviderEventStream,
    model: Model,
    state: AttemptState,
    blocks: Vec<AssistantContent>,
    active: Option<(usize, ContentKind, String)>,
    cancellation: TurnCancellation,
}

enum AttemptState {
    AwaitStarted,
    Streaming,
    AwaitEnd(AttemptTerminal),
    Finished,
}

enum AttemptTerminal {
    Completed(ProviderTurnCompletion),
    Failed(AgentError),
    Aborted,
}

impl ConversationAttempt {
    fn failure(&mut self) -> ConversationEvent {
        self.state = AttemptState::Finished;
        ConversationEvent::Failed {
            error: AgentError::fixed(
                AgentErrorCategory::Provider,
                "provider stream protocol is invalid",
            ),
        }
    }
}

impl Stream for ConversationAttempt {
    type Item = ConversationEvent;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            self.cancellation.inner.stream_waker.register(cx.waker());
            if self.cancellation.is_cancelled() && !matches!(self.state, AttemptState::Finished) {
                self.state = AttemptState::Finished;
                return Poll::Ready(Some(ConversationEvent::Aborted));
            }
            if matches!(self.state, AttemptState::Finished) {
                return Poll::Ready(None);
            }
            if matches!(self.state, AttemptState::AwaitEnd(_)) {
                return match self.provider_stream.as_mut().poll_next(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Some(_)) => {
                        let event = self.failure();
                        Poll::Ready(Some(event))
                    }
                    Poll::Ready(None) => {
                        let AttemptState::AwaitEnd(terminal) =
                            std::mem::replace(&mut self.state, AttemptState::Finished)
                        else {
                            unreachable!()
                        };
                        Poll::Ready(Some(match terminal {
                            AttemptTerminal::Completed(completion) => {
                                ConversationEvent::Completed {
                                    message: AssistantMessage {
                                        content: std::mem::take(&mut self.blocks),
                                        model: self.model.clone(),
                                        usage: completion.usage,
                                        finish_reason: completion.finish_reason,
                                        replay: completion.replay,
                                    },
                                }
                            }
                            AttemptTerminal::Failed(error) => ConversationEvent::Failed { error },
                            AttemptTerminal::Aborted => ConversationEvent::Aborted,
                        }))
                    }
                };
            }
            let event = match self.provider_stream.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    let event = self.failure();
                    return Poll::Ready(Some(event));
                }
                Poll::Ready(Some(event)) => event,
            };
            let public = match (&self.state, event) {
                (AttemptState::AwaitStarted, ProviderEvent::Started) => {
                    self.state = AttemptState::Streaming;
                    Some(ConversationEvent::Started)
                }
                (AttemptState::Streaming, ProviderEvent::ContentStarted { index, kind })
                    if self.active.is_none() && index == self.blocks.len() =>
                {
                    self.active = Some((index, kind, String::new()));
                    Some(ConversationEvent::ContentStarted { index, kind })
                }
                (AttemptState::Streaming, ProviderEvent::ContentDelta { index, delta })
                    if self.active.as_ref().is_some_and(|active| active.0 == index) =>
                {
                    self.active.as_mut().unwrap().2.push_str(&delta);
                    Some(ConversationEvent::ContentDelta { index, delta })
                }
                (AttemptState::Streaming, ProviderEvent::ContentFinished { index })
                    if self.active.as_ref().is_some_and(|active| active.0 == index) =>
                {
                    let (_, kind, text) = self.active.take().unwrap();
                    self.blocks.push(match kind {
                        ContentKind::Text => AssistantContent::Text(text),
                        ContentKind::Reasoning => AssistantContent::Reasoning(text),
                    });
                    Some(ConversationEvent::ContentFinished { index })
                }
                (AttemptState::Streaming, ProviderEvent::Completed(completion))
                    if self.active.is_none() =>
                {
                    self.state = AttemptState::AwaitEnd(AttemptTerminal::Completed(completion));
                    None
                }
                (AttemptState::Streaming, ProviderEvent::Failed(error)) => {
                    self.state = AttemptState::AwaitEnd(AttemptTerminal::Failed(error));
                    None
                }
                (AttemptState::Streaming, ProviderEvent::Aborted) => {
                    self.state = AttemptState::AwaitEnd(AttemptTerminal::Aborted);
                    None
                }
                _ => {
                    let event = self.failure();
                    return Poll::Ready(Some(event));
                }
            };
            if let Some(event) = public {
                return Poll::Ready(Some(event));
            }
        }
    }
}

/// A streamed turn holds the conversation's mutable borrow until it is dropped.
/// Model and Reasoning Level changes therefore cannot overlap an active turn.
///
/// ```compile_fail
/// use job_radar_lib::agent::AgentConversation;
/// use job_radar_lib::agent::models::ReasoningLevel;
///
/// fn change_during_turn(conversation: &mut AgentConversation) {
///     let stream = conversation.send("hello".to_owned()).unwrap();
///     conversation.set_reasoning_level(ReasoningLevel::High);
///     drop(stream);
/// }
/// ```
pub struct ConversationEventStream<'a> {
    conversation: &'a mut AgentConversation,
    attempt: ConversationAttempt,
    user: UserMessage,
}

impl ConversationEventStream<'_> {
    pub fn cancellation(&self) -> TurnCancellation {
        self.attempt.cancellation.clone()
    }
}

impl Stream for ConversationEventStream<'_> {
    type Item = ConversationEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.attempt).poll_next(context) {
            Poll::Ready(Some(ConversationEvent::Completed { message })) => {
                let user = self.user.clone();
                self.conversation.commit(user, message.clone());
                Poll::Ready(Some(ConversationEvent::Completed { message }))
            }
            other => other,
        }
    }
}
