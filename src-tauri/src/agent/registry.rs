use crate::agent::api::ApiKind;
use crate::agent::auth::{
    canonical_existing_prefix_is_inside_repository, create_private_directory,
    path_below_ancestor_contains_symlink, path_is_inside_repository, read_existing_private_file,
    trusted_directory_is_real,
};
use crate::agent::models::{
    Model, ModelCost, ModelCostTier, ModelId, ModelInput, ProviderId, ReasoningLevel,
};
use crate::agent::providers::{self, AuthenticationMethod, ProviderDescriptor};
use crate::agent::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

const MODELS_FILE: &str = "models.json";

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct ProviderAvailability {
    configured: BTreeSet<ProviderId>,
}

impl ProviderAvailability {
    pub fn new(providers: impl IntoIterator<Item = ProviderId>) -> Self {
        Self {
            configured: providers.into_iter().collect(),
        }
    }

    pub fn contains(&self, provider: &ProviderId) -> bool {
        self.configured.contains(provider)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRegistrySnapshot {
    providers: Vec<ProviderDescriptor>,
    models: Vec<Model>,
}

impl ModelRegistrySnapshot {
    fn builtins() -> Self {
        Self::from_providers(providers::builtins())
    }

    fn from_providers(providers: Vec<ProviderDescriptor>) -> Self {
        let models = providers
            .iter()
            .flat_map(|provider| provider.models().iter().cloned())
            .collect();
        Self { providers, models }
    }

    pub fn providers(&self) -> &[ProviderDescriptor] {
        &self.providers
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    pub fn provider(&self, id: &ProviderId) -> Option<&ProviderDescriptor> {
        self.providers.iter().find(|provider| provider.id() == id)
    }

    pub fn model(&self, provider: &ProviderId, model: &ModelId) -> Option<&Model> {
        self.models
            .iter()
            .find(|candidate| candidate.provider() == provider && candidate.id() == model)
    }

    pub fn available_models<'a>(&'a self, availability: &ProviderAvailability) -> Vec<&'a Model> {
        self.providers
            .iter()
            .filter(|provider| {
                provider.authentication_methods().is_empty()
                    || provider.has_configured_api_key()
                    || availability.contains(provider.id())
            })
            .flat_map(ProviderDescriptor::models)
            .collect()
    }
}

#[derive(Clone)]
enum EnvironmentAvailability {
    Process,
    Names(BTreeSet<String>),
}

impl EnvironmentAvailability {
    fn configured(&self, name: &str) -> bool {
        match self {
            Self::Process => std::env::var_os(name).is_some_and(|value| !value.is_empty()),
            Self::Names(names) => names.contains(name),
        }
    }
}

#[allow(dead_code)] // Retained for request resolution by issue 214; never exposed by snapshots.
#[derive(Clone)]
struct RequestConfig {
    api_key: Option<SecretConfigValue>,
    headers: BTreeMap<String, String>,
    auth_header: bool,
}

#[derive(Clone, Default)]
struct RequestConfigs {
    providers: BTreeMap<ProviderId, RequestConfig>,
    models: BTreeMap<(ProviderId, ModelId), BTreeMap<String, String>>,
}

struct PublishedRegistry {
    snapshot: Arc<ModelRegistrySnapshot>,
    _request_configs: Arc<RequestConfigs>,
    reload_failed: bool,
}

pub struct ModelRegistry {
    agents_data_root: PathBuf,
    models_path: PathBuf,
    environment: EnvironmentAvailability,
    published: RwLock<PublishedRegistry>,
}

impl ModelRegistry {
    pub fn from_agents_data_root(agents_data_root: impl AsRef<Path>) -> Result<Self, AgentError> {
        Self::with_environment(agents_data_root.as_ref(), EnvironmentAvailability::Process)
    }

    /// Test/application seam that supplies only configured environment-variable names,
    /// never their values.
    pub fn from_agents_data_root_with_environment_names(
        agents_data_root: impl AsRef<Path>,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, AgentError> {
        Self::with_environment(
            agents_data_root.as_ref(),
            EnvironmentAvailability::Names(names.into_iter().collect()),
        )
    }

    fn with_environment(
        agents_data_root: &Path,
        environment: EnvironmentAvailability,
    ) -> Result<Self, AgentError> {
        validate_agents_root(agents_data_root)?;
        let initial = PublishedRegistry {
            snapshot: Arc::new(ModelRegistrySnapshot::builtins()),
            _request_configs: Arc::new(RequestConfigs::default()),
            reload_failed: false,
        };
        let registry = Self {
            agents_data_root: agents_data_root.to_owned(),
            models_path: agents_data_root.join(MODELS_FILE),
            environment,
            published: RwLock::new(initial),
        };
        match registry.load_candidate() {
            Ok(candidate) => {
                registry.publish(candidate, false);
            }
            Err(_) => {
                registry
                    .published
                    .write()
                    .expect("registry lock poisoned")
                    .reload_failed = true;
            }
        }
        Ok(registry)
    }

    pub fn snapshot(&self) -> Arc<ModelRegistrySnapshot> {
        Arc::clone(
            &self
                .published
                .read()
                .expect("registry lock poisoned")
                .snapshot,
        )
    }

    pub fn last_reload_failed(&self) -> bool {
        self.published
            .read()
            .expect("registry lock poisoned")
            .reload_failed
    }

    pub fn reload(&self) -> Result<Arc<ModelRegistrySnapshot>, AgentError> {
        match self.load_candidate() {
            Ok(candidate) => Ok(self.publish(candidate, false)),
            Err(_) => {
                self.published
                    .write()
                    .expect("registry lock poisoned")
                    .reload_failed = true;
                Err(AgentError::invalid_model_configuration())
            }
        }
    }

    fn publish(&self, candidate: Candidate, reload_failed: bool) -> Arc<ModelRegistrySnapshot> {
        let snapshot = Arc::new(candidate.snapshot);
        *self.published.write().expect("registry lock poisoned") = PublishedRegistry {
            snapshot: Arc::clone(&snapshot),
            _request_configs: Arc::new(candidate.request_configs),
            reload_failed,
        };
        snapshot
    }

    fn load_candidate(&self) -> Result<Candidate, AgentError> {
        validate_agents_root(&self.agents_data_root)?;
        match fs::symlink_metadata(&self.models_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Candidate {
                    snapshot: ModelRegistrySnapshot::builtins(),
                    request_configs: RequestConfigs::default(),
                });
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(AgentError::invalid_model_configuration());
            }
            Ok(_) => {}
            Err(_) => return Err(AgentError::invalid_model_configuration()),
        }
        let bytes = read_existing_private_file(&self.models_path)
            .map_err(|_| AgentError::invalid_model_configuration())?;
        let source =
            std::str::from_utf8(&bytes).map_err(|_| AgentError::invalid_model_configuration())?;
        let without_comments = strip_json_comments(source)?;
        let value: Value = serde_json::from_str(&without_comments)
            .map_err(|_| AgentError::invalid_model_configuration())?;
        reject_disallowed_nulls(&value, None)?;
        let document: ModelsDocument =
            serde_json::from_value(value).map_err(|_| AgentError::invalid_model_configuration())?;
        compose(document, &self.environment)
    }
}

fn validate_agents_root(root: &Path) -> Result<(), AgentError> {
    let app_data = root
        .parent()
        .ok_or_else(AgentError::invalid_model_configuration)?;
    let trusted = app_data
        .parent()
        .ok_or_else(AgentError::invalid_model_configuration)?;
    let valid = root.file_name() == Some(std::ffi::OsStr::new("agents"))
        && trusted.is_absolute()
        && trusted_directory_is_real(trusted).unwrap_or(false)
        && app_data.is_absolute()
        && app_data.starts_with(trusted)
        && !path_is_inside_repository(app_data)
        && !canonical_existing_prefix_is_inside_repository(app_data)
        && !path_below_ancestor_contains_symlink(trusted, app_data).unwrap_or(true)
        && !path_below_ancestor_contains_symlink(trusted, root).unwrap_or(true);
    if !valid {
        return Err(AgentError::invalid_model_configuration());
    }
    create_private_directory(root).map_err(|_| AgentError::invalid_model_configuration())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelsDocument {
    providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderConfig {
    name: Option<String>,
    base_url: Option<String>,
    api_key: Option<SecretConfigValue>,
    api: Option<String>,
    oauth: Option<RadiusOAuth>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    compat: Option<CompatConfig>,
    #[serde(default)]
    auth_header: bool,
    #[serde(default)]
    models: Vec<ModelConfig>,
    #[serde(default)]
    model_overrides: BTreeMap<String, ModelOverride>,
}

#[allow(dead_code)] // Direct values remain private until issue 214 consumes request snapshots.
#[derive(Clone)]
enum SecretConfigValue {
    Direct(String),
    Environment(String),
}

impl SecretConfigValue {
    fn configured(&self, environment: &EnvironmentAvailability) -> bool {
        match self {
            Self::Direct(_) => true,
            Self::Environment(name) => environment.configured(name),
        }
    }
}

impl<'de> Deserialize<'de> for SecretConfigValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.is_empty() || value.starts_with('!') || value.contains('\0') {
            return Err(serde::de::Error::custom(
                "invalid protected configuration value",
            ));
        }
        let Some(reference) = value.strip_prefix('$') else {
            return Ok(Self::Direct(value));
        };
        let name = reference
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .unwrap_or(reference);
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err(serde::de::Error::custom("invalid environment reference"));
        }
        Ok(Self::Environment(name.to_owned()))
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RadiusOAuth {
    Radius,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelConfig {
    id: String,
    name: Option<String>,
    api: Option<String>,
    base_url: Option<String>,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    thinking_level_map: BTreeMap<String, Option<String>>,
    input: Option<Vec<String>>,
    cost: Option<CompleteCostConfig>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    compat: Option<CompatConfig>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelOverride {
    name: Option<String>,
    reasoning: Option<bool>,
    #[serde(default)]
    thinking_level_map: BTreeMap<String, Option<String>>,
    input: Option<Vec<String>>,
    cost: Option<PartialCostConfig>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    compat: Option<CompatConfig>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CostTierConfig {
    input_tokens_above: Number,
    input: Number,
    output: Number,
    cache_read: Number,
    cache_write: Number,
}

impl From<CostTierConfig> for ModelCostTier {
    fn from(value: CostTierConfig) -> Self {
        Self {
            input_tokens_above: value.input_tokens_above,
            input: value.input,
            output: value.output,
            cache_read: value.cache_read,
            cache_write: value.cache_write,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompleteCostConfig {
    input: Number,
    output: Number,
    cache_read: Number,
    cache_write: Number,
    tiers: Option<Vec<CostTierConfig>>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PartialCostConfig {
    input: Option<Number>,
    output: Option<Number>,
    cache_read: Option<Number>,
    cache_write: Option<Number>,
    tiers: Option<Vec<CostTierConfig>>,
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
enum CompatConfig {
    OpenAiCompletions(OpenAiCompletionsCompat),
    OpenAiResponses(OpenAiResponsesCompat),
    AnthropicMessages(AnthropicMessagesCompat),
}

impl<'de> Deserialize<'de> for CompatConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Ok(config) = serde_json::from_value(value.clone()) {
            return Ok(Self::OpenAiCompletions(config));
        }
        if let Ok(config) = serde_json::from_value(value.clone()) {
            return Ok(Self::OpenAiResponses(config));
        }
        if let Ok(config) = serde_json::from_value(value) {
            return Ok(Self::AnthropicMessages(config));
        }
        Err(serde::de::Error::custom(
            "invalid compatibility configuration",
        ))
    }
}

impl CompatConfig {
    fn value(&self) -> Result<Value, AgentError> {
        let mut value =
            serde_json::to_value(self).map_err(|_| AgentError::invalid_model_configuration())?;
        prune_compat_nulls(&mut value);
        Ok(value)
    }
}

fn prune_compat_nulls(value: &mut Value) {
    let Value::Object(compat) = value else {
        return;
    };
    compat.retain(|_, value| !value.is_null());
    for key in ["openRouterRouting", "vercelGatewayRouting"] {
        if let Some(Value::Object(nested)) = compat.get_mut(key) {
            nested.retain(|_, value| !value.is_null());
            for value in nested.values_mut() {
                if let Value::Object(deep) = value {
                    deep.retain(|_, value| !value.is_null());
                }
            }
        }
    }
    if let Some(Value::Object(kwargs)) = compat.get_mut("chatTemplateKwargs") {
        for value in kwargs.values_mut() {
            if let Value::Object(variable) = value {
                variable.retain(|_, value| !value.is_null());
            }
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiCompletionsCompat {
    supports_store: Option<bool>,
    supports_developer_role: Option<bool>,
    supports_reasoning_effort: Option<bool>,
    supports_usage_in_streaming: Option<bool>,
    max_tokens_field: Option<MaxTokensField>,
    requires_tool_result_name: Option<bool>,
    requires_assistant_after_tool_result: Option<bool>,
    requires_thinking_as_text: Option<bool>,
    requires_reasoning_content_on_assistant_messages: Option<bool>,
    thinking_format: Option<ThinkingFormat>,
    chat_template_kwargs: Option<BTreeMap<String, ChatTemplateKwarg>>,
    cache_control_format: Option<CacheControlFormat>,
    open_router_routing: Option<OpenRouterRouting>,
    vercel_gateway_routing: Option<VercelGatewayRouting>,
    supports_strict_mode: Option<bool>,
    send_session_affinity_headers: Option<bool>,
    session_affinity_format: Option<SessionAffinityFormat>,
    supports_long_cache_retention: Option<bool>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenAiResponsesCompat {
    supports_developer_role: Option<bool>,
    session_affinity_format: Option<SessionAffinityFormat>,
    supports_long_cache_retention: Option<bool>,
    supports_tool_search: Option<bool>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AnthropicMessagesCompat {
    supports_eager_tool_input_streaming: Option<bool>,
    supports_long_cache_retention: Option<bool>,
    send_session_affinity_headers: Option<bool>,
    supports_cache_control_on_tools: Option<bool>,
    force_adaptive_thinking: Option<bool>,
    supports_tool_references: Option<bool>,
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Deserialize, Serialize)]
        enum $name {
            $(#[serde(rename = $value)] $variant),+
        }
    };
}

string_enum!(MaxTokensField { MaxCompletionTokens => "max_completion_tokens", MaxTokens => "max_tokens" });
string_enum!(CacheControlFormat { Anthropic => "anthropic" });
string_enum!(SessionAffinityFormat { OpenAi => "openai", OpenAiNoSession => "openai-nosession", OpenRouter => "openrouter" });
string_enum!(ThinkingFormat {
    OpenAi => "openai", OpenRouter => "openrouter", Together => "together", Deepseek => "deepseek",
    Zai => "zai", Qwen => "qwen", ChatTemplate => "chat-template", QwenChatTemplate => "qwen-chat-template",
    StringThinking => "string-thinking", AntLing => "ant-ling"
});

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum ChatTemplateKwarg {
    String(String),
    Number(Number),
    Boolean(bool),
    Null,
    Variable(ChatTemplateVariable),
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChatTemplateVariable {
    #[serde(rename = "$var")]
    variable: ThinkingVariable,
    omit_when_off: Option<bool>,
}
string_enum!(ThinkingVariable { Enabled => "thinking.enabled", Effort => "thinking.effort" });

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PercentileCutoffs {
    p50: Option<Number>,
    p75: Option<Number>,
    p90: Option<Number>,
    p99: Option<Number>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum SortConfig {
    String(String),
    Detailed(SortDetails),
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SortDetails {
    by: Option<String>,
    partition: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MaxPrice {
    prompt: Option<NumberOrString>,
    completion: Option<NumberOrString>,
    image: Option<NumberOrString>,
    audio: Option<NumberOrString>,
    request: Option<NumberOrString>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum NumberOrString {
    Number(Number),
    String(String),
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum NumberOrPercentiles {
    Number(Number),
    Percentiles(PercentileCutoffs),
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OpenRouterRouting {
    allow_fallbacks: Option<bool>,
    require_parameters: Option<bool>,
    data_collection: Option<DataCollection>,
    zdr: Option<bool>,
    enforce_distillable_text: Option<bool>,
    order: Option<Vec<String>>,
    only: Option<Vec<String>>,
    ignore: Option<Vec<String>>,
    quantizations: Option<Vec<String>>,
    sort: Option<SortConfig>,
    max_price: Option<MaxPrice>,
    preferred_min_throughput: Option<NumberOrPercentiles>,
    preferred_max_latency: Option<NumberOrPercentiles>,
}
string_enum!(DataCollection { Deny => "deny", Allow => "allow" });

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VercelGatewayRouting {
    only: Option<Vec<String>>,
    order: Option<Vec<String>>,
}

struct ProviderWork {
    id: ProviderId,
    display_name: String,
    api: ApiKind,
    authentication_methods: Vec<AuthenticationMethod>,
    default_base_url: String,
    models: Vec<Model>,
    configured_api_key: bool,
}

impl ProviderWork {
    fn from_descriptor(descriptor: ProviderDescriptor) -> Self {
        Self {
            id: descriptor.id().clone(),
            display_name: descriptor.display_name().to_owned(),
            api: descriptor.api(),
            authentication_methods: descriptor.authentication_methods().to_vec(),
            default_base_url: descriptor.default_base_url().to_owned(),
            models: descriptor.models().to_vec(),
            configured_api_key: descriptor.has_configured_api_key(),
        }
    }

    fn finish(self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            self.id,
            self.display_name,
            self.api,
            self.authentication_methods,
            self.default_base_url,
            self.models,
            self.configured_api_key,
        )
    }
}

struct Candidate {
    snapshot: ModelRegistrySnapshot,
    request_configs: RequestConfigs,
}

fn compose(
    document: ModelsDocument,
    environment: &EnvironmentAvailability,
) -> Result<Candidate, AgentError> {
    let mut registry: BTreeMap<ProviderId, ProviderWork> = providers::builtins()
        .into_iter()
        .map(|descriptor| {
            let provider = ProviderWork::from_descriptor(descriptor);
            (provider.id.clone(), provider)
        })
        .collect();
    let mut requests = RequestConfigs::default();

    for (raw_provider_id, config) in document.providers {
        let provider_id = ProviderId::new(raw_provider_id)?;
        validate_headers(&config.headers)?;
        let configured_api_key = config
            .api_key
            .as_ref()
            .is_some_and(|value| value.configured(environment));
        if config.api_key.is_some() || !config.headers.is_empty() || config.auth_header {
            requests.providers.insert(
                provider_id.clone(),
                RequestConfig {
                    api_key: config.api_key.clone(),
                    headers: config.headers.clone(),
                    auth_header: config.auth_header,
                },
            );
        }
        apply_provider(
            &mut registry,
            &mut requests,
            provider_id,
            config,
            configured_api_key,
        )?;
    }

    Ok(Candidate {
        snapshot: ModelRegistrySnapshot::from_providers(
            registry.into_values().map(ProviderWork::finish).collect(),
        ),
        request_configs: requests,
    })
}

fn apply_provider(
    registry: &mut BTreeMap<ProviderId, ProviderWork>,
    requests: &mut RequestConfigs,
    provider_id: ProviderId,
    config: ProviderConfig,
    configured_api_key: bool,
) -> Result<(), AgentError> {
    if config.oauth.is_some() && config.base_url.is_none() {
        return Err(AgentError::invalid_model_configuration());
    }
    let is_builtin = registry.contains_key(&provider_id);
    let has_override = config.base_url.is_some()
        || !config.headers.is_empty()
        || config.compat.is_some()
        || !config.model_overrides.is_empty();
    if config.models.is_empty() && config.oauth.is_none() && !has_override {
        return Err(AgentError::invalid_model_configuration());
    }
    if !is_builtin && config.models.is_empty() {
        return Ok(());
    }

    let mut provider = if let Some(existing) = registry.remove(&provider_id) {
        existing
    } else {
        let api = config
            .api
            .as_deref()
            .map(ApiKind::parse)
            .transpose()?
            .or_else(|| {
                config
                    .models
                    .first()
                    .and_then(|model| model.api.as_deref())
                    .and_then(|value| ApiKind::parse(value).ok())
            })
            .ok_or_else(AgentError::invalid_model_configuration)?;
        let base_url = config
            .base_url
            .clone()
            .ok_or_else(AgentError::invalid_model_configuration)?;
        validate_base_url(&base_url)?;
        ProviderWork {
            id: provider_id.clone(),
            display_name: config
                .name
                .clone()
                .unwrap_or_else(|| provider_id.as_str().to_owned()),
            api,
            authentication_methods: vec![AuthenticationMethod::ApiKey],
            default_base_url: base_url,
            models: Vec::new(),
            configured_api_key: false,
        }
    };

    if let Some(name) = config.name.as_ref() {
        if name.is_empty() {
            return Err(AgentError::invalid_model_configuration());
        }
        provider.display_name.clone_from(name);
    }
    if let Some(api) = config.api.as_deref() {
        provider.api = ApiKind::parse(api)?;
    }
    if let Some(base_url) = config.base_url.as_ref() {
        validate_base_url(base_url)?;
        provider.default_base_url.clone_from(base_url);
    }
    provider.configured_api_key |= configured_api_key;
    if config.api_key.is_some()
        && !provider
            .authentication_methods
            .contains(&AuthenticationMethod::ApiKey)
    {
        provider
            .authentication_methods
            .push(AuthenticationMethod::ApiKey);
    }
    if config.oauth.is_some()
        && !provider
            .authentication_methods
            .contains(&AuthenticationMethod::OAuth)
    {
        provider
            .authentication_methods
            .push(AuthenticationMethod::OAuth);
    }

    let provider_compat = config
        .compat
        .as_ref()
        .map(CompatConfig::value)
        .transpose()?
        .unwrap_or(Value::Null);
    for model in &mut provider.models {
        if let Some(base_url) = config.base_url.as_ref() {
            *model.parts_mut().base_url = base_url.clone();
        }
        merge_compat(model.parts_mut().compat, &provider_compat)?;
    }

    for (raw_model_id, model_override) in config.model_overrides {
        let model_id = ModelId::new(raw_model_id)?;
        validate_headers(&model_override.headers)?;
        if !model_override.headers.is_empty() {
            requests.models.insert(
                (provider_id.clone(), model_id.clone()),
                model_override.headers.clone(),
            );
        }
        let model = provider
            .models
            .iter_mut()
            .find(|model| model.id() == &model_id)
            .ok_or_else(AgentError::invalid_model_configuration)?;
        apply_override(model, model_override)?;
    }

    for custom in config.models {
        validate_headers(&custom.headers)?;
        let model_id = ModelId::new(custom.id.clone())?;
        let request_key = (provider_id.clone(), model_id);
        requests.models.remove(&request_key);
        if !custom.headers.is_empty() {
            requests.models.insert(request_key, custom.headers.clone());
        }
        let model = custom_model(&provider, &provider_compat, custom)?;
        if let Some(index) = provider
            .models
            .iter()
            .position(|candidate| candidate.id() == model.id())
        {
            provider.models[index] = model;
        } else {
            provider.models.push(model);
        }
    }
    registry.insert(provider_id, provider);
    Ok(())
}

fn custom_model(
    provider: &ProviderWork,
    provider_compat: &Value,
    config: ModelConfig,
) -> Result<Model, AgentError> {
    let id = ModelId::new(config.id)?;
    let api = config
        .api
        .as_deref()
        .map(ApiKind::parse)
        .transpose()?
        .unwrap_or(provider.api);
    let base_url = config
        .base_url
        .unwrap_or_else(|| provider.default_base_url.clone());
    validate_base_url(&base_url)?;
    let display_name = config.name.unwrap_or_else(|| id.as_str().to_owned());
    let input = parse_input(config.input)?;
    let thinking_level_map = parse_thinking_map(config.thinking_level_map)?;
    let levels = reasoning_levels(config.reasoning, &thinking_level_map);
    let cost = complete_cost(config.cost)?;
    let context_window = config.context_window.unwrap_or(128_000);
    let max_tokens = config.max_tokens.unwrap_or(16_384);
    let mut compat = Value::Object(Map::new());
    merge_compat(&mut compat, provider_compat)?;
    if let Some(model_compat) = config.compat {
        merge_compat(&mut compat, &model_compat.value()?)?;
    }
    Model::with_capabilities(
        id,
        display_name,
        provider.id.clone(),
        levels,
        api,
        base_url,
        input,
        cost,
        context_window,
        max_tokens,
        BTreeMap::new(),
        compat,
        thinking_level_map,
    )
}

fn apply_override(model: &mut Model, config: ModelOverride) -> Result<(), AgentError> {
    let parsed_map = parse_thinking_map(config.thinking_level_map)?;
    let parts = model.parts_mut();
    if let Some(name) = config.name {
        if name.is_empty() {
            return Err(AgentError::invalid_model_configuration());
        }
        *parts.display_name = name;
    }
    if let Some(input) = config.input {
        *parts.input = parse_input(Some(input))?;
    }
    *parts.cost = partial_cost(parts.cost.clone(), config.cost);
    if let Some(context_window) = config.context_window {
        *parts.context_window = context_window;
    }
    if let Some(max_tokens) = config.max_tokens {
        *parts.max_tokens = max_tokens;
    }
    if !parsed_map.is_empty() {
        parts.thinking_level_map.extend(parsed_map);
    }
    if let Some(reasoning) = config.reasoning {
        *parts.supported_reasoning_levels = reasoning_levels(reasoning, parts.thinking_level_map);
    }
    if let Some(compat) = config.compat {
        merge_compat(parts.compat, &compat.value()?)?;
    }
    Ok(())
}

fn reasoning_levels(
    reasoning: bool,
    thinking_level_map: &BTreeMap<ReasoningLevel, Option<String>>,
) -> Vec<ReasoningLevel> {
    if !reasoning {
        return vec![ReasoningLevel::Off];
    }
    let mut levels = vec![
        ReasoningLevel::Off,
        ReasoningLevel::Minimal,
        ReasoningLevel::Low,
        ReasoningLevel::Medium,
        ReasoningLevel::High,
        ReasoningLevel::XHigh,
    ];
    if thinking_level_map.contains_key(&ReasoningLevel::Max) {
        levels.push(ReasoningLevel::Max);
    }
    levels
}

fn parse_thinking_map(
    config: BTreeMap<String, Option<String>>,
) -> Result<BTreeMap<ReasoningLevel, Option<String>>, AgentError> {
    config
        .into_iter()
        .map(|(level, value)| {
            let level = ReasoningLevel::from_config_key(&level)
                .ok_or_else(AgentError::invalid_model_configuration)?;
            Ok((level, value))
        })
        .collect()
}

fn parse_input(config: Option<Vec<String>>) -> Result<Vec<ModelInput>, AgentError> {
    config
        .unwrap_or_else(|| vec!["text".to_owned()])
        .into_iter()
        .map(|value| match value.as_str() {
            "text" => Ok(ModelInput::Text),
            "image" => Ok(ModelInput::Image),
            _ => Err(AgentError::invalid_model_configuration()),
        })
        .collect()
}

fn complete_cost(config: Option<CompleteCostConfig>) -> Result<ModelCost, AgentError> {
    let Some(config) = config else {
        return Ok(ModelCost::default());
    };
    Ok(ModelCost::from_parts(
        config.input,
        config.output,
        config.cache_read,
        config.cache_write,
        config
            .tiers
            .map(|tiers| tiers.into_iter().map(Into::into).collect()),
    ))
}

fn partial_cost(base: ModelCost, config: Option<PartialCostConfig>) -> ModelCost {
    let Some(config) = config else {
        return base;
    };
    base.merged(
        config.input,
        config.output,
        config.cache_read,
        config.cache_write,
        config
            .tiers
            .map(|tiers| tiers.into_iter().map(Into::into).collect()),
    )
}

fn merge_compat(target: &mut Value, overlay: &Value) -> Result<(), AgentError> {
    if overlay.is_null() {
        return Ok(());
    }
    let Value::Object(target) = target else {
        return Err(AgentError::invalid_model_configuration());
    };
    let Value::Object(overlay) = overlay else {
        return Err(AgentError::invalid_model_configuration());
    };
    for (key, value) in overlay {
        if matches!(
            key.as_str(),
            "openRouterRouting" | "vercelGatewayRouting" | "chatTemplateKwargs"
        ) {
            if let (Some(Value::Object(existing)), Value::Object(incoming)) =
                (target.get_mut(key), value)
            {
                existing.extend(incoming.clone());
                continue;
            }
        }
        target.insert(key.clone(), value.clone());
    }
    Ok(())
}

fn validate_headers(headers: &BTreeMap<String, String>) -> Result<(), AgentError> {
    if headers
        .iter()
        .any(|(name, value)| name.trim().is_empty() || value.contains(['\r', '\n']))
    {
        Err(AgentError::invalid_model_configuration())
    } else {
        Ok(())
    }
}

fn validate_base_url(value: &str) -> Result<(), AgentError> {
    if value.is_empty() {
        Err(AgentError::invalid_model_configuration())
    } else {
        Ok(())
    }
}

fn reject_disallowed_nulls(value: &Value, parent_key: Option<&str>) -> Result<(), AgentError> {
    match value {
        Value::Null => Err(AgentError::invalid_model_configuration()),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| reject_disallowed_nulls(value, parent_key)),
        Value::Object(values) => values.iter().try_for_each(|(key, value)| {
            if value.is_null()
                && (key == "partition"
                    || matches!(parent_key, Some("thinkingLevelMap" | "chatTemplateKwargs")))
            {
                Ok(())
            } else {
                reject_disallowed_nulls(value, Some(key))
            }
        }),
        _ => Ok(()),
    }
}

fn strip_json_comments(source: &str) -> Result<String, AgentError> {
    let bytes = source.as_bytes();
    let mut result = Vec::with_capacity(source.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            result.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            result.push(b'"');
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            result.push(b'\n');
            index += usize::from(index < bytes.len());
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            let mut closed = false;
            while index + 1 < bytes.len() {
                if bytes[index] == b'*' && bytes[index + 1] == b'/' {
                    index += 2;
                    closed = true;
                    break;
                }
                if bytes[index] == b'\n' {
                    result.push(b'\n');
                }
                index += 1;
            }
            if !closed {
                return Err(AgentError::invalid_model_configuration());
            }
            continue;
        }
        result.push(byte);
        index += 1;
    }
    if in_string {
        return Err(AgentError::invalid_model_configuration());
    }
    String::from_utf8(result).map_err(|_| AgentError::invalid_model_configuration())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_model_replacement_clears_headers_from_an_earlier_override() {
        let document: ModelsDocument = serde_json::from_str(
            r#"{
                "providers": {
                    "openai-codex": {
                        "modelOverrides": {
                            "gpt-5.4": {"headers": {"x-synthetic": "override"}}
                        },
                        "models": [{"id": "gpt-5.4"}]
                    }
                }
            }"#,
        )
        .unwrap();

        let candidate = compose(document, &EnvironmentAvailability::Names(BTreeSet::new()))
            .expect("synthetic document must compose");
        let request_key = (
            ProviderId::new("openai-codex").unwrap(),
            ModelId::new("gpt-5.4").unwrap(),
        );

        assert!(!candidate.request_configs.models.contains_key(&request_key));
    }
}
