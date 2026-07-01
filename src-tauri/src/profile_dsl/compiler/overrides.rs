use std::collections::HashSet;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    OverridableStep, PostingDetailStep, PostingDiscoveryStep, SourceOverrides,
};

use super::compiler_error;

pub(super) fn validate_source_overrides(
    source_overrides: Option<&SourceOverrides>,
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    diagnostics: &mut Diagnostics,
) {
    let Some(source_overrides) = source_overrides else {
        return;
    };
    let Some(strategy_overrides) = &source_overrides.strategy_overrides else {
        return;
    };

    let discovery_keys = posting_discovery
        .strategies
        .iter()
        .map(|strategy| strategy.key.as_str())
        .collect::<HashSet<_>>();
    let detail_keys = posting_detail
        .map(|step| {
            step.strategies
                .iter()
                .map(|strategy| strategy.key.as_str())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let mut seen_overrides = HashSet::new();

    for (index, strategy_override) in strategy_overrides.iter().enumerate() {
        let step_name = match strategy_override.step {
            OverridableStep::PostingDiscovery => "postingDiscovery",
            OverridableStep::PostingDetail => "postingDetail",
        };
        if !seen_overrides.insert((step_name, strategy_override.strategy_key.as_str())) {
            diagnostics.push(compiler_error(
                "duplicate_strategy_override",
                format!(
                    "sourceOverrides contains more than one override for {step_name} Strategy `{}`",
                    strategy_override.strategy_key
                ),
                format!("/sourceOverrides/strategyOverrides/{index}/strategyKey"),
                serde_json::json!({
                    "step": step_name,
                    "strategyKey": strategy_override.strategy_key,
                }),
            ));
        }
        let known = match strategy_override.step {
            OverridableStep::PostingDiscovery => {
                discovery_keys.contains(strategy_override.strategy_key.as_str())
            }
            OverridableStep::PostingDetail => {
                detail_keys.contains(strategy_override.strategy_key.as_str())
            }
        };
        if !known {
            diagnostics.push(compiler_error(
                "unknown_strategy_override",
                format!(
                    "sourceOverrides references unknown {step_name} Strategy `{}`",
                    strategy_override.strategy_key
                ),
                format!("/sourceOverrides/strategyOverrides/{index}/strategyKey"),
                serde_json::json!({
                    "step": step_name,
                    "strategyKey": strategy_override.strategy_key,
                }),
            ));
        }
    }
}
