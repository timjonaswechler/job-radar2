use crate::agent::models::{Model, ModelId, ReasoningLevel};
use crate::agent::{
    AgentError, AgentErrorCategory, AssistantContent, AssistantMessage, ConversationProvider,
    ConversationRequest, FinishReason, Message, ProviderEvent, ProviderEventStream, TokenUsage,
};
use futures_util::stream;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedConversationRequest {
    system_prompt: String,
    messages: Vec<Message>,
    model: ModelId,
    reasoning: ReasoningLevel,
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
            messages,
            model,
            reasoning,
        }
    }

    fn matches(&self, request: &ConversationRequest) -> bool {
        self.system_prompt == request.system_prompt()
            && self.messages == request.messages()
            && &self.model == request.model().id()
            && self.reasoning == request.reasoning_level()
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

fn script_error() -> AgentError {
    AgentError::fixed(
        AgentErrorCategory::InvalidConfiguration,
        "scripted provider request did not match",
    )
}
