use crate::agent::conversation::{BlockSignature, ReplayMetadata};
use crate::agent::models::{Model, ModelId, ReasoningLevel};
use crate::agent::{
    AgentError, AgentErrorCategory, AssistantContent, AssistantMessage, ConversationProvider,
    ConversationRequest, FinishReason, Message, ProviderEvent, ProviderEventStream,
    ProviderTurnCompletion, TokenUsage,
};
use futures_util::stream;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::agent::sessions::{
    self, SessionCheckpoint, SessionError, SessionHandle, SessionManager,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedConversationRequest {
    system_prompt: String,
    messages: Option<Vec<Message>>,
    model: ModelId,
    reasoning: ReasoningLevel,
    max_tokens: Option<u64>,
}

impl ExpectedConversationRequest {
    pub fn new(
        system_prompt: impl Into<String>,
        messages: Vec<Message>,
        model: ModelId,
        reasoning: ReasoningLevel,
    ) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            messages: Some(messages),
            model,
            reasoning,
            max_tokens: None,
        }
    }

    pub fn any_messages(
        system_prompt: impl Into<String>,
        model: ModelId,
        reasoning: ReasoningLevel,
    ) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            messages: None,
            model,
            reasoning,
            max_tokens: None,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    fn matches(&self, request: &ConversationRequest) -> bool {
        self.system_prompt == request.system_prompt()
            && self
                .messages
                .as_ref()
                .is_none_or(|messages| messages == request.messages())
            && &self.model == request.model().id()
            && self.reasoning == request.reasoning_level()
            && self
                .max_tokens
                .is_none_or(|tokens| request.model().max_tokens() == tokens)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedTurn {
    expected: ExpectedConversationRequest,
    events: Vec<ProviderEvent>,
}

impl ScriptedTurn {
    pub fn new(expected: ExpectedConversationRequest, events: Vec<ProviderEvent>) -> Self {
        Self { expected, events }
    }
}

pub fn synthetic_model_with_limits(
    mut model: Model,
    context_window: u64,
    max_tokens: u64,
) -> Model {
    *model.parts_mut().context_window = context_window;
    *model.parts_mut().max_tokens = max_tokens;
    model
}

pub fn synthetic_assistant_message(
    content: Vec<AssistantContent>,
    model: Model,
    usage: TokenUsage,
    finish_reason: FinishReason,
) -> Message {
    Message::Assistant(AssistantMessage::synthetic(
        content,
        model,
        usage,
        finish_reason,
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntheticReplaySignature {
    Text(String),
    Reasoning { signature: String, redacted: bool },
}

pub fn synthetic_turn_completion_with_replay(
    usage: TokenUsage,
    finish_reason: FinishReason,
    response_id: Option<String>,
    signatures: Vec<Option<SyntheticReplaySignature>>,
) -> ProviderTurnCompletion {
    ProviderTurnCompletion::with_replay(
        usage,
        finish_reason,
        response_id,
        replay_signatures(signatures),
    )
}

pub fn synthetic_assistant_message_with_replay(
    content: Vec<AssistantContent>,
    model: Model,
    usage: TokenUsage,
    finish_reason: FinishReason,
    response_id: Option<String>,
    signatures: Vec<Option<SyntheticReplaySignature>>,
) -> Message {
    Message::Assistant(AssistantMessage::from_replay(
        content,
        model,
        usage,
        finish_reason,
        ReplayMetadata {
            response_id,
            block_signatures: replay_signatures(signatures),
        },
    ))
}

fn replay_signatures(
    signatures: Vec<Option<SyntheticReplaySignature>>,
) -> Vec<Option<BlockSignature>> {
    signatures
        .into_iter()
        .map(|signature| {
            signature.map(|signature| match signature {
                SyntheticReplaySignature::Text(signature) => BlockSignature::Text(signature),
                SyntheticReplaySignature::Reasoning {
                    signature,
                    redacted,
                } => BlockSignature::Reasoning {
                    signature,
                    redacted,
                },
            })
        })
        .collect()
}

#[derive(Clone)]
pub struct ScriptedProvider {
    models: Arc<Vec<Model>>,
    state: Arc<Mutex<ScriptedState>>,
}

struct ScriptedState {
    turns: VecDeque<ScriptedTurn>,
    requests: Vec<ConversationRequest>,
    conversation_id: Option<String>,
    mismatch: bool,
}

impl ScriptedProvider {
    pub fn new(models: Vec<Model>, turns: Vec<ScriptedTurn>) -> Self {
        Self {
            models: Arc::new(models),
            state: Arc::new(Mutex::new(ScriptedState {
                turns: turns.into(),
                requests: Vec::new(),
                conversation_id: None,
                mismatch: false,
            })),
        }
    }

    pub fn recorded_requests(&self) -> Vec<ConversationRequest> {
        self.state
            .lock()
            .expect("scripted provider lock poisoned")
            .requests
            .clone()
    }

    pub fn assert_exhausted(&self) -> Result<(), AgentError> {
        let state = self.state.lock().expect("scripted provider lock poisoned");
        if state.turns.is_empty() && !state.mismatch {
            Ok(())
        } else {
            Err(script_error())
        }
    }
}

impl ConversationProvider for ScriptedProvider {
    fn models(&self) -> &[Model] {
        self.models.as_slice()
    }

    fn stream(&self, request: ConversationRequest) -> ProviderEventStream {
        let mut state = self.state.lock().expect("scripted provider lock poisoned");
        let id_matches = match &state.conversation_id {
            Some(id) => id == request.conversation_id(),
            None => {
                state.conversation_id = Some(request.conversation_id().to_owned());
                !request.conversation_id().is_empty()
            }
        };
        state.requests.push(request.clone());
        let Some(turn) = state.turns.pop_front() else {
            state.mismatch = true;
            return Box::pin(stream::iter(vec![
                ProviderEvent::Started,
                ProviderEvent::Failed(script_error()),
            ]));
        };
        if !id_matches || !turn.expected.matches(&request) {
            state.mismatch = true;
            return Box::pin(stream::iter(vec![
                ProviderEvent::Started,
                ProviderEvent::Failed(script_error()),
            ]));
        }
        Box::pin(stream::iter(turn.events))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeContinuationBlock {
    role: &'static str,
    text: Vec<String>,
    signature_count: usize,
    has_response_id: bool,
    redacted_thinking_count: usize,
}

impl SafeContinuationBlock {
    pub fn role(&self) -> &'static str {
        self.role
    }
    pub fn text(&self) -> &[String] {
        &self.text
    }
    pub fn signature_count(&self) -> usize {
        self.signature_count
    }
    pub fn has_response_id(&self) -> bool {
        self.has_response_id
    }
    pub fn redacted_thinking_count(&self) -> usize {
        self.redacted_thinking_count
    }
}

#[derive(Clone)]
pub struct SessionTestHarness {
    runtime: Arc<TestSessionRuntime>,
}

struct TestSessionRuntime {
    timestamps: Mutex<VecDeque<String>>,
    uuids: Mutex<VecDeque<Uuid>>,
    trash_succeeds: bool,
    trashed: Mutex<Vec<PathBuf>>,
    failing_checkpoints: Mutex<std::collections::HashSet<SessionCheckpoint>>,
}

impl SessionTestHarness {
    pub fn new(timestamps: Vec<String>, uuids: Vec<Uuid>, trash_succeeds: bool) -> Self {
        Self {
            runtime: Arc::new(TestSessionRuntime {
                timestamps: Mutex::new(timestamps.into()),
                uuids: Mutex::new(uuids.into()),
                trash_succeeds,
                trashed: Mutex::new(Vec::new()),
                failing_checkpoints: Mutex::new(std::collections::HashSet::new()),
            }),
        }
    }

    pub fn fail_at(self, checkpoints: impl IntoIterator<Item = SessionCheckpoint>) -> Self {
        self.runtime
            .failing_checkpoints
            .lock()
            .expect("session test lock poisoned")
            .extend(checkpoints);
        self
    }

    pub fn manager(&self, agents_root: &Path) -> Result<SessionManager, SessionError> {
        sessions::manager_with_runtime(agents_root, self.runtime.clone())
    }

    pub fn continuation(&self, handle: &SessionHandle) -> Vec<SafeContinuationBlock> {
        handle
            .continuation
            .iter()
            .map(|block| match block {
                sessions::ContinuationBlock::User(text) => SafeContinuationBlock {
                    role: "user",
                    text: vec![text.clone()],
                    signature_count: 0,
                    has_response_id: false,
                    redacted_thinking_count: 0,
                },
                sessions::ContinuationBlock::Assistant {
                    blocks,
                    response_id,
                    ..
                } => SafeContinuationBlock {
                    role: "assistant",
                    text: blocks
                        .iter()
                        .filter_map(|block| match block {
                            sessions::ContinuationAssistantBlock::Text { text, .. } => {
                                Some(text.clone())
                            }
                            sessions::ContinuationAssistantBlock::Thinking {
                                thinking,
                                redacted: false,
                                ..
                            } => Some(thinking.clone()),
                            sessions::ContinuationAssistantBlock::Thinking {
                                redacted: true,
                                ..
                            } => None,
                        })
                        .collect(),
                    signature_count: blocks
                        .iter()
                        .filter(|block| match block {
                            sessions::ContinuationAssistantBlock::Text { signature, .. }
                            | sessions::ContinuationAssistantBlock::Thinking {
                                signature, ..
                            } => signature.is_some(),
                        })
                        .count(),
                    has_response_id: response_id.is_some(),
                    redacted_thinking_count: blocks
                        .iter()
                        .filter(|block| {
                            matches!(
                                block,
                                sessions::ContinuationAssistantBlock::Thinking {
                                    redacted: true,
                                    ..
                                }
                            )
                        })
                        .count(),
                },
            })
            .collect()
    }

    pub fn trashed_paths(&self) -> Vec<PathBuf> {
        self.runtime
            .trashed
            .lock()
            .expect("session test lock poisoned")
            .clone()
    }
}

impl sessions::Runtime for TestSessionRuntime {
    fn now(&self) -> String {
        self.timestamps
            .lock()
            .expect("session test lock poisoned")
            .pop_front()
            .expect("deterministic timestamp exhausted")
    }
    fn uuid(&self) -> Uuid {
        self.uuids
            .lock()
            .expect("session test lock poisoned")
            .pop_front()
            .expect("deterministic UUID exhausted")
    }
    fn trash(&self, path: &Path) -> Result<(), ()> {
        if !self.trash_succeeds {
            return Err(());
        }
        self.trashed
            .lock()
            .expect("session test lock poisoned")
            .push(path.to_owned());
        std::fs::rename(path, path.with_extension("trashed")).map_err(|_| ())
    }
    fn checkpoint(&self, checkpoint: SessionCheckpoint) -> Result<(), ()> {
        if self
            .failing_checkpoints
            .lock()
            .expect("session test lock poisoned")
            .remove(&checkpoint)
        {
            Err(())
        } else {
            Ok(())
        }
    }
}

fn script_error() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::InvalidConfiguration,
        "scripted provider request did not match",
    )
}
