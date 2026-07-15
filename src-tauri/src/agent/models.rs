use crate::agent::AgentError;
use std::sync::OnceLock;

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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentError> {
        validate_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReasoningLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    id: ModelId,
    display_name: String,
    provider: ProviderId,
    supported_reasoning_levels: Vec<ReasoningLevel>,
}

impl Model {
    pub fn new(
        id: ModelId,
        display_name: impl Into<String>,
        provider: ProviderId,
        supported_reasoning_levels: Vec<ReasoningLevel>,
    ) -> Result<Self, AgentError> {
        let display_name = display_name.into();
        let ordered_levels: Vec<_> = REASONING_ORDER
            .iter()
            .copied()
            .filter(|level| supported_reasoning_levels.contains(level))
            .collect();
        if display_name.trim().is_empty()
            || ordered_levels.is_empty()
            || ordered_levels.len() != supported_reasoning_levels.len()
        {
            return Err(AgentError::invalid_model_configuration());
        }
        Ok(Self {
            id,
            display_name,
            provider,
            supported_reasoning_levels: ordered_levels,
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
}

pub fn openai_codex_models() -> &'static [Model] {
    static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
    MODELS.get_or_init(|| {
        // Capability snapshot ported from Pi at dcfe36c79702ec240b146c45f167ab75ecddd205.
        [
            ("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark", false),
            ("gpt-5.4", "GPT-5.4", false),
            ("gpt-5.4-mini", "GPT-5.4 mini", false),
            ("gpt-5.5", "GPT-5.5", false),
            ("gpt-5.6-luna", "GPT-5.6 Luna", true),
            ("gpt-5.6-sol", "GPT-5.6 Sol", true),
            ("gpt-5.6-terra", "GPT-5.6 Terra", true),
        ]
        .into_iter()
        .map(|(id, name, supports_max)| {
            let mut levels = REASONING_ORDER[..6].to_vec();
            if supports_max {
                levels.push(ReasoningLevel::Max);
            }
            Model::new(
                ModelId::new(id).expect("pinned model identifier must be valid"),
                name,
                ProviderId::new("openai-codex").expect("pinned provider identifier must be valid"),
                levels,
            )
            .expect("pinned model metadata must be valid")
        })
        .collect()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_codex_catalog_exposes_exact_models_and_reasoning_levels() {
        let models = openai_codex_models();

        assert_eq!(
            models
                .iter()
                .map(|model| model.id().as_str())
                .collect::<Vec<_>>(),
            vec![
                "gpt-5.3-codex-spark",
                "gpt-5.4",
                "gpt-5.4-mini",
                "gpt-5.5",
                "gpt-5.6-luna",
                "gpt-5.6-sol",
                "gpt-5.6-terra",
            ]
        );
        assert_eq!(models[0].display_name(), "GPT-5.3 Codex Spark");
        assert_eq!(models[0].provider().as_str(), "openai-codex");
        assert_eq!(
            models[0].supported_reasoning_levels(),
            &[
                ReasoningLevel::Off,
                ReasoningLevel::Minimal,
                ReasoningLevel::Low,
                ReasoningLevel::Medium,
                ReasoningLevel::High,
                ReasoningLevel::XHigh,
            ]
        );
        assert_eq!(
            models[4].supported_reasoning_levels(),
            &[
                ReasoningLevel::Off,
                ReasoningLevel::Minimal,
                ReasoningLevel::Low,
                ReasoningLevel::Medium,
                ReasoningLevel::High,
                ReasoningLevel::XHigh,
                ReasoningLevel::Max,
            ]
        );
    }

    #[test]
    fn reasoning_normalization_uses_nearest_level_and_breaks_ties_upward() {
        let sparse = Model::new(
            ModelId::new("synthetic-model").unwrap(),
            "Synthetic model",
            ProviderId::new("synthetic-provider").unwrap(),
            vec![
                ReasoningLevel::Off,
                ReasoningLevel::Medium,
                ReasoningLevel::XHigh,
            ],
        )
        .unwrap();

        assert_eq!(
            sparse.normalize_reasoning(ReasoningLevel::Minimal),
            ReasoningLevel::Off
        );
        assert_eq!(
            sparse.normalize_reasoning(ReasoningLevel::Low),
            ReasoningLevel::Medium
        );
        assert_eq!(
            sparse.normalize_reasoning(ReasoningLevel::Max),
            ReasoningLevel::XHigh
        );
        assert_eq!(
            sparse.normalize_reasoning(ReasoningLevel::Medium),
            ReasoningLevel::Medium
        );
    }

    #[test]
    fn identifiers_and_model_capabilities_fail_closed() {
        assert!(ModelId::new("").is_err());
        assert!(ProviderId::new("contains a space").is_err());
        assert!(Model::new(
            ModelId::new("synthetic-model").unwrap(),
            "Synthetic model",
            ProviderId::new("synthetic-provider").unwrap(),
            Vec::new(),
        )
        .is_err());
        assert!(find_openai_codex_model("missing-model").is_err());
    }

    #[test]
    fn codex_reasoning_maps_minimal_to_low_and_preserves_extended_levels() {
        let standard = find_openai_codex_model("gpt-5.4").unwrap();
        let extended = find_openai_codex_model("gpt-5.6-luna").unwrap();
        assert_eq!(
            codex_reasoning_effort(standard, ReasoningLevel::Off).unwrap(),
            None
        );
        assert_eq!(
            codex_reasoning_effort(standard, ReasoningLevel::Minimal).unwrap(),
            Some("low")
        );
        assert_eq!(
            codex_reasoning_effort(standard, ReasoningLevel::Max).unwrap(),
            Some("xhigh")
        );
        assert_eq!(
            codex_reasoning_effort(extended, ReasoningLevel::Max).unwrap(),
            Some("max")
        );
    }
}
