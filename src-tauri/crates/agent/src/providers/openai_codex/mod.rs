mod authentication;
pub(crate) mod loopback;
mod models;
mod streaming;

use super::{AuthenticationMethod, ProviderDescriptor};
use crate::api::ApiKind;
use crate::models::ProviderId;

#[cfg(test)]
pub(crate) use authentication::AuthStatus;
pub(crate) use authentication::{
    AgentAuthentication, AuthFuture, AuthInteraction, BrowserAuthorization, DeviceAuthorization,
    LoginMethod, ProviderCredential, SecretAuthorizationInput, PROVIDER_ID,
};
pub(crate) use streaming::OpenAiCodexProvider;

pub(crate) fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new(PROVIDER_ID).expect("pinned provider identifier must be valid"),
        "OpenAI Codex".to_owned(),
        ApiKind::OpenAiResponses,
        vec![AuthenticationMethod::OAuth],
        "https://chatgpt.com/backend-api".to_owned(),
        models::builtin_models().to_vec(),
        false,
        false,
    )
}
