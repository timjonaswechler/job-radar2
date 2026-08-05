pub(crate) mod openai_codex;

use crate::api::ApiKind;
use crate::models::{Model, ProviderId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthenticationMethod {
    ApiKey,
    OAuth,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderDescriptor {
    id: ProviderId,
    display_name: String,
    api: ApiKind,
    authentication_methods: Vec<AuthenticationMethod>,
    default_base_url: String,
    models: Vec<Model>,
    configured_api_key: bool,
    api_key_reference: bool,
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
        api_key_reference: bool,
    ) -> Self {
        Self {
            id,
            display_name,
            api,
            authentication_methods,
            default_base_url,
            models,
            configured_api_key,
            api_key_reference,
        }
    }

    pub(crate) fn id(&self) -> &ProviderId {
        &self.id
    }

    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    pub(crate) fn api(&self) -> ApiKind {
        self.api
    }

    pub(crate) fn authentication_methods(&self) -> &[AuthenticationMethod] {
        &self.authentication_methods
    }

    pub(crate) fn default_base_url(&self) -> &str {
        &self.default_base_url
    }

    pub(crate) fn models(&self) -> &[Model] {
        &self.models
    }

    pub(crate) fn has_configured_api_key(&self) -> bool {
        self.configured_api_key
    }

    pub(crate) fn has_api_key_reference(&self) -> bool {
        self.api_key_reference
    }
}

pub(crate) fn builtins() -> Vec<ProviderDescriptor> {
    vec![openai_codex::descriptor()]
}
