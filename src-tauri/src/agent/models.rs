use crate::agent::api::ApiKind;
use crate::agent::AgentError;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

const IDENTIFIER_MAX_BYTES: usize = 128;
const REASONING_ORDER: [ReasoningLevel; 7] = [
    ReasoningLevel::Off,
    ReasoningLevel::Minimal,
    ReasoningLevel::Low,
    ReasoningLevel::Medium,
    ReasoningLevel::High,
    ReasoningLevel::XHigh,
    ReasoningLevel::Max,
];

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        validate_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        validate_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_identifier(value: String) -> Result<String, AgentError> {
    let valid = !value.is_empty()
        && value.len() <= IDENTIFIER_MAX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(value)
    } else {
        Err(AgentError::invalid_model_configuration())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReasoningLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ReasoningLevel {
    pub(crate) fn from_config_key(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelInput {
    Text,
    Image,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCostTier {
    pub(crate) input_tokens_above: serde_json::Number,
    pub(crate) input: serde_json::Number,
    pub(crate) output: serde_json::Number,
    pub(crate) cache_read: serde_json::Number,
    pub(crate) cache_write: serde_json::Number,
}

impl ModelCostTier {
    pub fn input_tokens_above(&self) -> &serde_json::Number {
        &self.input_tokens_above
    }

    pub fn input(&self) -> &serde_json::Number {
        &self.input
    }

    pub fn output(&self) -> &serde_json::Number {
        &self.output
    }

    pub fn cache_read(&self) -> &serde_json::Number {
        &self.cache_read
    }

    pub fn cache_write(&self) -> &serde_json::Number {
        &self.cache_write
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCost {
    input: serde_json::Number,
    output: serde_json::Number,
    cache_read: serde_json::Number,
    cache_write: serde_json::Number,
    tiers: Option<Vec<ModelCostTier>>,
}

impl Default for ModelCost {
    fn default() -> Self {
        Self {
            input: 0.into(),
            output: 0.into(),
            cache_read: 0.into(),
            cache_write: 0.into(),
            tiers: None,
        }
    }
}

impl ModelCost {
    pub fn input(&self) -> &serde_json::Number {
        &self.input
    }

    pub fn output(&self) -> &serde_json::Number {
        &self.output
    }

    pub fn cache_read(&self) -> &serde_json::Number {
        &self.cache_read
    }

    pub fn cache_write(&self) -> &serde_json::Number {
        &self.cache_write
    }

    pub fn tiers(&self) -> Option<&[ModelCostTier]> {
        self.tiers.as_deref()
    }

    pub(crate) fn from_parts(
        input: serde_json::Number,
        output: serde_json::Number,
        cache_read: serde_json::Number,
        cache_write: serde_json::Number,
        tiers: Option<Vec<ModelCostTier>>,
    ) -> Self {
        Self {
            input,
            output,
            cache_read,
            cache_write,
            tiers,
        }
    }

    pub(crate) fn merged(
        &self,
        input: Option<serde_json::Number>,
        output: Option<serde_json::Number>,
        cache_read: Option<serde_json::Number>,
        cache_write: Option<serde_json::Number>,
        tiers: Option<Vec<ModelCostTier>>,
    ) -> Self {
        Self {
            input: input.unwrap_or_else(|| self.input.clone()),
            output: output.unwrap_or_else(|| self.output.clone()),
            cache_read: cache_read.unwrap_or_else(|| self.cache_read.clone()),
            cache_write: cache_write.unwrap_or_else(|| self.cache_write.clone()),
            tiers: tiers.or_else(|| self.tiers.clone()),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Model {
    id: ModelId,
    display_name: String,
    provider: ProviderId,
    supported_reasoning_levels: Vec<ReasoningLevel>,
    api: ApiKind,
    base_url: String,
    input: Vec<ModelInput>,
    cost: ModelCost,
    context_window: u64,
    max_tokens: u64,
    headers: BTreeMap<String, String>,
    compat: Value,
    thinking_level_map: BTreeMap<ReasoningLevel, Option<String>>,
}

impl Model {
    pub fn new(
        id: ModelId,
        display_name: impl Into<String>,
        provider: ProviderId,
        supported_reasoning_levels: Vec<ReasoningLevel>,
    ) -> Result<Self, AgentError> {
        Self::with_capabilities(
            id,
            display_name.into(),
            provider,
            supported_reasoning_levels,
            ApiKind::OpenAiResponses,
            "https://api.openai.com/v1".to_owned(),
            vec![ModelInput::Text],
            ModelCost::default(),
            128_000,
            16_384,
            BTreeMap::new(),
            Value::Object(Default::default()),
            BTreeMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_capabilities(
        id: ModelId,
        display_name: String,
        provider: ProviderId,
        supported_reasoning_levels: Vec<ReasoningLevel>,
        api: ApiKind,
        base_url: String,
        input: Vec<ModelInput>,
        cost: ModelCost,
        context_window: u64,
        max_tokens: u64,
        headers: BTreeMap<String, String>,
        compat: Value,
        thinking_level_map: BTreeMap<ReasoningLevel, Option<String>>,
    ) -> Result<Self, AgentError> {
        let ordered_levels: Vec<_> = REASONING_ORDER
            .iter()
            .copied()
            .filter(|level| supported_reasoning_levels.contains(level))
            .collect();
        if display_name.trim().is_empty()
            || ordered_levels.is_empty()
            || ordered_levels.len() != supported_reasoning_levels.len()
            || base_url.is_empty()
            || context_window == 0
            || max_tokens == 0
            || !compat.is_object()
        {
            return Err(AgentError::invalid_model_configuration());
        }
        Ok(Self {
            id,
            display_name,
            provider,
            supported_reasoning_levels: ordered_levels,
            api,
            base_url,
            input,
            cost,
            context_window,
            max_tokens,
            headers,
            compat,
            thinking_level_map,
        })
    }

    pub fn id(&self) -> &ModelId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    pub fn supported_reasoning_levels(&self) -> &[ReasoningLevel] {
        &self.supported_reasoning_levels
    }

    pub fn api(&self) -> ApiKind {
        self.api
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn input(&self) -> &[ModelInput] {
        &self.input
    }

    pub fn cost(&self) -> &ModelCost {
        &self.cost
    }

    pub fn context_window(&self) -> u64 {
        self.context_window
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn compat(&self) -> &Value {
        &self.compat
    }

    pub fn thinking_level_map(&self) -> &BTreeMap<ReasoningLevel, Option<String>> {
        &self.thinking_level_map
    }

    pub fn normalize_reasoning(&self, requested: ReasoningLevel) -> ReasoningLevel {
        if self.supported_reasoning_levels.contains(&requested) {
            return requested;
        }
        let requested_index = REASONING_ORDER
            .iter()
            .position(|level| *level == requested)
            .unwrap_or(0);
        self.supported_reasoning_levels
            .iter()
            .copied()
            .min_by_key(|level| {
                let index = REASONING_ORDER
                    .iter()
                    .position(|candidate| candidate == level)
                    .unwrap_or(0);
                (index.abs_diff(requested_index), std::cmp::Reverse(index))
            })
            .unwrap_or(ReasoningLevel::Off)
    }

    pub(crate) fn parts_mut(&mut self) -> ModelPartsMut<'_> {
        ModelPartsMut {
            display_name: &mut self.display_name,
            supported_reasoning_levels: &mut self.supported_reasoning_levels,
            base_url: &mut self.base_url,
            input: &mut self.input,
            cost: &mut self.cost,
            context_window: &mut self.context_window,
            max_tokens: &mut self.max_tokens,
            compat: &mut self.compat,
            thinking_level_map: &mut self.thinking_level_map,
        }
    }
}

impl fmt::Debug for Model {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("provider", &self.provider)
            .field(
                "supported_reasoning_levels",
                &self.supported_reasoning_levels,
            )
            .field("api", &self.api)
            .field("base_url", &self.base_url)
            .field("input", &self.input)
            .field("cost", &self.cost)
            .field("context_window", &self.context_window)
            .field("max_tokens", &self.max_tokens)
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("compat", &self.compat)
            .field("thinking_level_map", &self.thinking_level_map)
            .finish()
    }
}

pub(crate) struct ModelPartsMut<'a> {
    pub display_name: &'a mut String,
    pub supported_reasoning_levels: &'a mut Vec<ReasoningLevel>,
    pub base_url: &'a mut String,
    pub input: &'a mut Vec<ModelInput>,
    pub cost: &'a mut ModelCost,
    pub context_window: &'a mut u64,
    pub max_tokens: &'a mut u64,
    pub compat: &'a mut Value,
    pub thinking_level_map: &'a mut BTreeMap<ReasoningLevel, Option<String>>,
}

pub fn openai_codex_models() -> &'static [Model] {
    crate::agent::providers::openai_codex::models::builtin_models()
}

pub fn find_openai_codex_model(id: &str) -> Result<&'static Model, AgentError> {
    openai_codex_models()
        .iter()
        .find(|model| model.id().as_str() == id)
        .ok_or_else(AgentError::model_unavailable)
}

pub(crate) fn codex_reasoning_effort(
    model: &Model,
    level: ReasoningLevel,
) -> Result<Option<&'static str>, AgentError> {
    if model.provider().as_str() != "openai-codex" {
        return Err(AgentError::invalid_model_configuration());
    }
    let normalized = model.normalize_reasoning(level);
    Ok(match normalized {
        ReasoningLevel::Off => None,
        ReasoningLevel::Minimal | ReasoningLevel::Low => Some("low"),
        ReasoningLevel::Medium => Some("medium"),
        ReasoningLevel::High => Some("high"),
        ReasoningLevel::XHigh => Some("xhigh"),
        ReasoningLevel::Max => Some("max"),
    })
}
