use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentErrorCategory {
    Authentication,
    ModelUnavailable,
    Transport,
    RateLimited,
    Provider,
    InvalidConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentError {
    pub category: AgentErrorCategory,
    pub message: String,
    pub retry_after: Option<Duration>,
}

impl AgentError {
    pub(crate) fn authentication() -> Self {
        Self::fixed(AgentErrorCategory::Authentication, "authentication failed")
    }

    pub(crate) fn transport() -> Self {
        Self::fixed(
            AgentErrorCategory::Transport,
            "authentication transport is unavailable",
        )
    }

    pub(crate) fn model_unavailable() -> Self {
        Self::fixed(
            AgentErrorCategory::ModelUnavailable,
            "agent model is unavailable",
        )
    }

    pub(crate) fn invalid_model_configuration() -> Self {
        Self::fixed(
            AgentErrorCategory::InvalidConfiguration,
            "agent model configuration is invalid",
        )
    }

    pub(crate) fn invalid_authentication_configuration() -> Self {
        Self::fixed(
            AgentErrorCategory::InvalidConfiguration,
            "authentication is not securely configured",
        )
    }

    pub(crate) fn provider_transport() -> Self {
        Self::fixed(
            AgentErrorCategory::Transport,
            "provider transport is unavailable",
        )
    }

    pub(crate) fn provider() -> Self {
        Self::fixed(AgentErrorCategory::Provider, "provider request failed")
    }

    pub(crate) fn invalid_provider_configuration() -> Self {
        Self::fixed(
            AgentErrorCategory::InvalidConfiguration,
            "provider configuration is invalid",
        )
    }

    pub(crate) fn rate_limited(retry_after: Option<Duration>) -> Self {
        Self {
            category: AgentErrorCategory::RateLimited,
            message: "provider rate limit reached".to_owned(),
            retry_after,
        }
    }

    pub(crate) fn fixed(category: AgentErrorCategory, message: &'static str) -> Self {
        Self {
            category,
            message: message.to_owned(),
            retry_after: None,
        }
    }
}
