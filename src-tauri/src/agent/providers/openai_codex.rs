pub mod models;

use super::{AuthenticationMethod, ProviderDescriptor};
use crate::agent::api::ApiKind;
use crate::agent::models::ProviderId;

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new("openai-codex").expect("pinned provider identifier must be valid"),
        "OpenAI Codex".to_owned(),
        ApiKind::OpenAiResponses,
        vec![AuthenticationMethod::OAuth],
        "https://chatgpt.com/backend-api".to_owned(),
        models::builtin_models().to_vec(),
        false,
    )
}
