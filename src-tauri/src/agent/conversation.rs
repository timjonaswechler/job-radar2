use crate::agent::models::{Model, ModelId, ReasoningLevel};
use crate::agent::{AgentError, AgentErrorCategory};
use futures_util::Stream;
use std::fmt;
use std::pin::Pin;
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
struct ReplayMetadata(Vec<u8>);

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
        Self {
            content,
            model,
            usage,
            finish_reason,
            replay: ReplayMetadata(Vec::new()),
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

    #[allow(dead_code)] // Concrete provider adapters consume this without exposing opaque payloads.
    pub(crate) fn replay_metadata(&self) -> &[u8] {
        &self.replay.0
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
            replay: ReplayMetadata(Vec::new()),
        }
    }

    #[allow(dead_code)] // Concrete provider adapters attach opaque replay data through this path.
    pub(crate) fn with_replay(
        usage: TokenUsage,
        finish_reason: FinishReason,
        replay: Vec<u8>,
    ) -> Self {
        Self {
            usage,
            finish_reason,
            replay: ReplayMetadata(replay),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationRequest {
    system_prompt: String,
    messages: Vec<Message>,
    model: Model,
    reasoning: ReasoningLevel,
    conversation_id: String,
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
}

pub trait ConversationProvider: Send + 'static {
    fn models(&self) -> &[Model];
    fn stream(&self, request: ConversationRequest) -> ProviderEventStream;
}

pub struct AgentConversation {
    system_prompt: String,
    provider: Box<dyn ConversationProvider>,
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
        let models = provider.models().to_vec();
        let selected = models
            .iter()
            .find(|candidate| candidate.id() == &model)
            .cloned()
            .ok_or_else(AgentError::model_unavailable)?;
        let reasoning = selected.normalize_reasoning(reasoning);
        Ok(Self {
            system_prompt,
            provider: Box::new(provider),
            models,
            model: selected,
            reasoning,
            messages: Vec::new(),
            conversation_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn available_models(&self) -> &[Model] {
        &self.models
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn reasoning_level(&self) -> ReasoningLevel {
        self.reasoning
    }

    pub fn select_model(&mut self, model: ModelId) -> Result<(), AgentError> {
        let selected = self
            .models
            .iter()
            .find(|candidate| candidate.id() == &model)
            .cloned()
            .ok_or_else(AgentError::model_unavailable)?;
        self.reasoning = selected.normalize_reasoning(self.reasoning);
        self.model = selected;
        Ok(())
    }

    pub fn set_reasoning_level(&mut self, level: ReasoningLevel) -> ReasoningLevel {
        self.reasoning = self.model.normalize_reasoning(level);
        self.reasoning
    }

    pub fn send(&mut self, text: String) -> Result<ConversationEventStream<'_>, AgentError> {
        let user = UserMessage::new(text);
        let mut request_messages = self.messages.clone();
        request_messages.push(Message::User(user.clone()));
        let request = ConversationRequest {
            system_prompt: self.system_prompt.clone(),
            messages: request_messages,
            model: self.model.clone(),
            reasoning: self.reasoning,
            conversation_id: self.conversation_id.clone(),
        };
        let provider_stream = self.provider.stream(request);
        Ok(ConversationEventStream {
            conversation: self,
            provider_stream,
            user,
            state: StreamState::AwaitStarted,
            blocks: Vec::new(),
            active: None,
        })
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
    provider_stream: ProviderEventStream,
    user: UserMessage,
    state: StreamState,
    blocks: Vec<AssistantContent>,
    active: Option<(usize, ContentKind, String)>,
}

enum StreamState {
    AwaitStarted,
    Streaming,
    AwaitProviderEnd(Terminal),
    Finished,
}

enum Terminal {
    Completed(ProviderTurnCompletion),
    Failed(AgentError),
    Aborted,
}

impl ConversationEventStream<'_> {
    fn protocol_failure(&mut self) -> ConversationEvent {
        self.state = StreamState::Finished;
        ConversationEvent::Failed {
            error: AgentError::fixed(
                AgentErrorCategory::Provider,
                "provider stream protocol is invalid",
            ),
        }
    }

    fn accept_event(&mut self, event: ProviderEvent) -> Option<ConversationEvent> {
        match (&self.state, event) {
            (StreamState::AwaitStarted, ProviderEvent::Started) => {
                self.state = StreamState::Streaming;
                Some(ConversationEvent::Started)
            }
            (StreamState::Streaming, ProviderEvent::ContentStarted { index, kind })
                if self.active.is_none() && index == self.blocks.len() =>
            {
                self.active = Some((index, kind, String::new()));
                Some(ConversationEvent::ContentStarted { index, kind })
            }
            (StreamState::Streaming, ProviderEvent::ContentDelta { index, delta })
                if self.active.as_ref().is_some_and(|active| active.0 == index) =>
            {
                if let Some(active) = &mut self.active {
                    active.2.push_str(&delta);
                }
                Some(ConversationEvent::ContentDelta { index, delta })
            }
            (StreamState::Streaming, ProviderEvent::ContentFinished { index })
                if self.active.as_ref().is_some_and(|active| active.0 == index) =>
            {
                let (_, kind, content) = self.active.take().expect("active block checked");
                self.blocks.push(match kind {
                    ContentKind::Text => AssistantContent::Text(content),
                    ContentKind::Reasoning => AssistantContent::Reasoning(content),
                });
                Some(ConversationEvent::ContentFinished { index })
            }
            (StreamState::Streaming, ProviderEvent::Completed(completion))
                if self.active.is_none() =>
            {
                self.state = StreamState::AwaitProviderEnd(Terminal::Completed(completion));
                None
            }
            (StreamState::Streaming, ProviderEvent::Failed(error)) => {
                self.state = StreamState::AwaitProviderEnd(Terminal::Failed(error));
                None
            }
            (StreamState::Streaming, ProviderEvent::Aborted) => {
                self.state = StreamState::AwaitProviderEnd(Terminal::Aborted);
                None
            }
            _ => Some(self.protocol_failure()),
        }
    }

    fn finish_terminal(&mut self, terminal: Terminal) -> ConversationEvent {
        self.state = StreamState::Finished;
        match terminal {
            Terminal::Completed(completion) => {
                let message = AssistantMessage {
                    content: std::mem::take(&mut self.blocks),
                    model: self.conversation.model.clone(),
                    usage: completion.usage,
                    finish_reason: completion.finish_reason,
                    replay: completion.replay,
                };
                self.conversation
                    .messages
                    .push(Message::User(self.user.clone()));
                self.conversation
                    .messages
                    .push(Message::Assistant(message.clone()));
                ConversationEvent::Completed { message }
            }
            Terminal::Failed(error) => ConversationEvent::Failed { error },
            Terminal::Aborted => ConversationEvent::Aborted,
        }
    }
}

impl Stream for ConversationEventStream<'_> {
    type Item = ConversationEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if matches!(self.state, StreamState::Finished) {
                return Poll::Ready(None);
            }
            if matches!(self.state, StreamState::AwaitProviderEnd(_)) {
                return match self.provider_stream.as_mut().poll_next(context) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Some(_)) => {
                        let failure = self.protocol_failure();
                        Poll::Ready(Some(failure))
                    }
                    Poll::Ready(None) => {
                        let StreamState::AwaitProviderEnd(terminal) =
                            std::mem::replace(&mut self.state, StreamState::Finished)
                        else {
                            unreachable!()
                        };
                        let event = self.finish_terminal(terminal);
                        Poll::Ready(Some(event))
                    }
                };
            }

            match self.provider_stream.as_mut().poll_next(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    let failure = self.protocol_failure();
                    return Poll::Ready(Some(failure));
                }
                Poll::Ready(Some(event)) => {
                    if let Some(event) = self.accept_event(event) {
                        return Poll::Ready(Some(event));
                    }
                }
            }
        }
    }
}
