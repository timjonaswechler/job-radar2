pub mod openai_codex;

use crate::agent::api::ApiKind;
use crate::agent::models::{Model, ProviderId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationMethod {
    ApiKey,
    OAuth,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    id: ProviderId,
    display_name: String,
    api: ApiKind,
    authentication_methods: Vec<AuthenticationMethod>,
    default_base_url: String,
    models: Vec<Model>,
    configured_api_key: bool,
}

impl ProviderDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: ProviderId,
        display_name: String,
        api: ApiKind,
        authentication_methods: Vec<AuthenticationMethod>,
        default_base_url: String,
        models: Vec<Model>,
        configured_api_key: bool,
    ) -> Self {
        Self {
            id,
            display_name,
            api,
            authentication_methods,
            default_base_url,
            models,
            configured_api_key,
        }
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn api(&self) -> ApiKind {
        self.api
    }

    pub fn authentication_methods(&self) -> &[AuthenticationMethod] {
        &self.authentication_methods
    }

    pub fn default_base_url(&self) -> &str {
        &self.default_base_url
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    pub(crate) fn has_configured_api_key(&self) -> bool {
        self.configured_api_key
    }
}

pub(crate) fn builtins() -> Vec<ProviderDescriptor> {
    vec![openai_codex::descriptor()]
}
