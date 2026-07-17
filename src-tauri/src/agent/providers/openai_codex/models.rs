use crate::agent::api::ApiKind;
use crate::agent::models::{
    Model, ModelCost, ModelCostTier, ModelId, ModelInput, ProviderId, ReasoningLevel,
};
use serde_json::{Number, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const BASE_URL: &str = "https://chatgpt.com/backend-api";

pub fn builtin_models() -> &'static [Model] {
    static MODELS: OnceLock<Vec<Model>> = OnceLock::new();
    MODELS.get_or_init(|| {
        // Exact capability snapshot from Pi at dcfe36c79702ec240b146c45f167ab75ecddd205.
        vec![
            model(
                "gpt-5.3-codex-spark",
                "GPT-5.3 Codex Spark",
                false,
                false,
                vec![ModelInput::Text],
                cost(1.75, 14.0, 0.175, 0.0, None),
                128_000,
            ),
            model(
                "gpt-5.4",
                "GPT-5.4",
                false,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(
                    2.5,
                    15.0,
                    0.25,
                    0.0,
                    Some(tier(272_000.0, 5.0, 22.5, 0.5, 0.0)),
                ),
                272_000,
            ),
            model(
                "gpt-5.4-mini",
                "GPT-5.4 mini",
                false,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(0.75, 4.5, 0.075, 0.0, None),
                272_000,
            ),
            model(
                "gpt-5.5",
                "GPT-5.5",
                false,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(
                    5.0,
                    30.0,
                    0.5,
                    0.0,
                    Some(tier(272_000.0, 10.0, 45.0, 1.0, 0.0)),
                ),
                272_000,
            ),
            model(
                "gpt-5.6-luna",
                "GPT-5.6 Luna",
                true,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(
                    1.0,
                    6.0,
                    0.1,
                    1.25,
                    Some(tier(272_000.0, 2.0, 9.0, 0.2, 2.5)),
                ),
                372_000,
            ),
            model(
                "gpt-5.6-sol",
                "GPT-5.6 Sol",
                true,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(
                    5.0,
                    30.0,
                    0.5,
                    6.25,
                    Some(tier(272_000.0, 10.0, 45.0, 1.0, 12.5)),
                ),
                372_000,
            ),
            model(
                "gpt-5.6-terra",
                "GPT-5.6 Terra",
                true,
                true,
                vec![ModelInput::Text, ModelInput::Image],
                cost(
                    2.5,
                    15.0,
                    0.25,
                    3.125,
                    Some(tier(272_000.0, 5.0, 22.5, 0.5, 6.25)),
                ),
                372_000,
            ),
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn model(
    id: &str,
    name: &str,
    supports_max: bool,
    supports_tool_search: bool,
    input: Vec<ModelInput>,
    cost: ModelCost,
    context_window: u64,
) -> Model {
    let mut levels = vec![
        ReasoningLevel::Off,
        ReasoningLevel::Minimal,
        ReasoningLevel::Low,
        ReasoningLevel::Medium,
        ReasoningLevel::High,
        ReasoningLevel::XHigh,
    ];
    let mut thinking = BTreeMap::from([
        (ReasoningLevel::Minimal, Some("low".to_owned())),
        (ReasoningLevel::XHigh, Some("xhigh".to_owned())),
    ]);
    if supports_max {
        levels.push(ReasoningLevel::Max);
        thinking.insert(ReasoningLevel::Max, Some("max".to_owned()));
    }
    let compat = if supports_tool_search {
        serde_json::json!({"supportsToolSearch": true})
    } else {
        Value::Object(Default::default())
    };
    Model::with_capabilities(
        ModelId::new(id).expect("pinned model identifier must be valid"),
        name.to_owned(),
        ProviderId::new("openai-codex").expect("pinned provider identifier must be valid"),
        levels,
        ApiKind::OpenAiResponses,
        BASE_URL.to_owned(),
        input,
        cost,
        context_window,
        128_000,
        BTreeMap::new(),
        compat,
        thinking,
    )
    .expect("pinned model metadata must be valid")
}

fn cost(
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    tier: Option<ModelCostTier>,
) -> ModelCost {
    ModelCost::from_parts(
        number(input),
        number(output),
        number(cache_read),
        number(cache_write),
        tier.map(|value| vec![value]),
    )
}

fn tier(
    input_tokens_above: f64,
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
) -> ModelCostTier {
    ModelCostTier {
        input_tokens_above: number(input_tokens_above),
        input: number(input),
        output: number(output),
        cache_read: number(cache_read),
        cache_write: number(cache_write),
    }
}

fn number(value: f64) -> Number {
    if value.fract() == 0.0 && value >= 0.0 && value <= u64::MAX as f64 {
        Number::from(value as u64)
    } else {
        Number::from_f64(value).expect("pinned model cost must be finite")
    }
}
