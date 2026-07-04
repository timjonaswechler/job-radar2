use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    ListFieldExpression, OverridableStep, PostingDetailStep, PostingDetailStrategy,
    PostingDiscoveryStep, PostingDiscoveryStrategy, SourceOverrides, StrategyOverride,
};

use super::compiler_error;

pub(super) struct EffectiveAccessPathSteps {
    pub posting_discovery: PostingDiscoveryStep,
    pub posting_detail: Option<PostingDetailStep>,
}

pub(super) fn apply_source_overrides(
    source_overrides: Option<&SourceOverrides>,
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    diagnostics: &mut Diagnostics,
) -> EffectiveAccessPathSteps {
    let mut effective = EffectiveAccessPathSteps {
        posting_discovery: posting_discovery.clone(),
        posting_detail: posting_detail.cloned(),
    };

    let Some(source_overrides) = source_overrides else {
        return effective;
    };
    let Some(strategy_overrides) = &source_overrides.strategy_overrides else {
        return effective;
    };

    let mut seen_overrides = HashSet::new();

    for (override_index, strategy_override) in strategy_overrides.iter().enumerate() {
        let step_name = step_name(strategy_override.step);
        let is_duplicate =
            !seen_overrides.insert((step_name, strategy_override.strategy_key.as_str()));
        if is_duplicate {
            diagnostics.push(compiler_error(
                "duplicate_strategy_override",
                format!(
                    "sourceOverrides contains more than one override for {step_name} Strategy `{}`",
                    strategy_override.strategy_key
                ),
                format!("/sourceOverrides/strategyOverrides/{override_index}/strategyKey"),
                serde_json::json!({
                    "step": step_name,
                    "strategyKey": strategy_override.strategy_key,
                }),
            ));
            continue;
        }

        match strategy_override.step {
            OverridableStep::PostingDiscovery => {
                let Some(strategy_index) = effective
                    .posting_discovery
                    .strategies
                    .iter()
                    .position(|strategy| strategy.key == strategy_override.strategy_key)
                else {
                    push_unknown_strategy_override(strategy_override, override_index, diagnostics);
                    continue;
                };
                apply_posting_discovery_strategy_override(
                    &mut effective.posting_discovery.strategies[strategy_index],
                    strategy_override,
                    override_index,
                    diagnostics,
                );
            }
            OverridableStep::PostingDetail => {
                let Some(posting_detail) = effective.posting_detail.as_mut() else {
                    push_unknown_strategy_override(strategy_override, override_index, diagnostics);
                    continue;
                };
                let Some(strategy_index) = posting_detail
                    .strategies
                    .iter()
                    .position(|strategy| strategy.key == strategy_override.strategy_key)
                else {
                    push_unknown_strategy_override(strategy_override, override_index, diagnostics);
                    continue;
                };
                apply_posting_detail_strategy_override(
                    &mut posting_detail.strategies[strategy_index],
                    strategy_override,
                    override_index,
                    diagnostics,
                );
            }
        }
    }

    effective
}

fn apply_posting_discovery_strategy_override(
    strategy: &mut PostingDiscoveryStrategy,
    strategy_override: &StrategyOverride,
    override_index: usize,
    diagnostics: &mut Diagnostics,
) {
    if let Some(fetch) = &strategy_override.fetch {
        strategy.fetch = fetch.clone();
    }
    if let Some(select) = &strategy_override.select {
        strategy.select = select.clone();
    }
    if let Some(extract) = &strategy_override.extract {
        for (field, expression) in extract {
            match field.as_str() {
                "title" => strategy.extract.fields.title = expression.clone(),
                "company" => strategy.extract.fields.company = expression.clone(),
                "url" => strategy.extract.fields.url = expression.clone(),
                "locations" => {
                    strategy.extract.fields.locations =
                        Some(ListFieldExpression::Single(expression.clone()))
                }
                "descriptionText" => {
                    strategy.extract.fields.description_text = Some(expression.clone())
                }
                "postingMeta" => push_unsupported_extract_override(
                    field,
                    override_index,
                    &strategy_override.strategy_key,
                    diagnostics,
                ),
                _ => push_unknown_extract_override(
                    field,
                    override_index,
                    &strategy_override.strategy_key,
                    diagnostics,
                ),
            }
        }
    }
    if let Some(accept_when) = &strategy_override.accept_when {
        strategy.accept_when = Some(accept_when.clone());
    }
}

fn apply_posting_detail_strategy_override(
    strategy: &mut PostingDetailStrategy,
    strategy_override: &StrategyOverride,
    override_index: usize,
    diagnostics: &mut Diagnostics,
) {
    if let Some(fetch) = &strategy_override.fetch {
        strategy.fetch = fetch.clone();
    }
    if let Some(select) = &strategy_override.select {
        strategy.select = select.clone();
    }
    if let Some(extract) = &strategy_override.extract {
        for (field, expression) in extract {
            match field.as_str() {
                "descriptionText" => strategy.extract.fields.description_text = expression.clone(),
                _ => push_unknown_extract_override(
                    field,
                    override_index,
                    &strategy_override.strategy_key,
                    diagnostics,
                ),
            }
        }
    }
    if let Some(accept_when) = &strategy_override.accept_when {
        strategy.accept_when = Some(accept_when.clone());
    }
}

fn push_unknown_strategy_override(
    strategy_override: &StrategyOverride,
    override_index: usize,
    diagnostics: &mut Diagnostics,
) {
    let step_name = step_name(strategy_override.step);
    diagnostics.push(compiler_error(
        "unknown_strategy_override",
        format!(
            "sourceOverrides references unknown {step_name} Strategy `{}`",
            strategy_override.strategy_key
        ),
        format!("/sourceOverrides/strategyOverrides/{override_index}/strategyKey"),
        serde_json::json!({
            "step": step_name,
            "strategyKey": strategy_override.strategy_key,
        }),
    ));
}

fn push_unknown_extract_override(
    field: &str,
    override_index: usize,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let mut diagnostic = compiler_error(
        "unknown_extract_override_field",
        format!("sourceOverrides cannot override unknown extract field `{field}`"),
        format!("/sourceOverrides/strategyOverrides/{override_index}/extract/{field}"),
        serde_json::json!({ "field": field }),
    );
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn push_unsupported_extract_override(
    field: &str,
    override_index: usize,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let mut diagnostic = compiler_error(
        "unsupported_extract_override_field",
        format!("sourceOverrides cannot override extract field `{field}` in this DSL version"),
        format!("/sourceOverrides/strategyOverrides/{override_index}/extract/{field}"),
        serde_json::json!({ "field": field }),
    );
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}

fn step_name(step: OverridableStep) -> &'static str {
    match step {
        OverridableStep::PostingDiscovery => "postingDiscovery",
        OverridableStep::PostingDetail => "postingDetail",
    }
}
