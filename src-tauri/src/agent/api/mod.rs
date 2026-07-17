use crate::agent::AgentError;

/// Identifies a request protocol compiled into Job Radar. Provider identity is
/// deliberately separate so multiple providers can share one protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ApiKind {
    OpenAiResponses,
}

impl ApiKind {
    pub(crate) fn parse(value: &str) -> Result<Self, AgentError> {
        match value {
            "openai-responses" => Ok(Self::OpenAiResponses),
            _ => Err(AgentError::invalid_model_configuration()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
        }
    }
}
