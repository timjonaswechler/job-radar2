use serde_json::json;

use super::*;

pub(super) const DESCRIPTOR: AcceptanceDescriptor = AcceptanceDescriptor {
    key: "minDescriptionLength",
    phases: &[AcceptancePhase::Discovery, AcceptancePhase::Detail],
};

pub(super) fn evaluate_discovery(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(rule) = stricter(
        phase.and_then(|plan| plan.min_description_length),
        strategy.and_then(|plan| plan.min_description_length),
        "/discovery",
        strategy_path,
    ) else {
        return true;
    };
    if let Some((item_index, actual)) =
        candidates
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| {
                let actual = candidate
                    .provider_values
                    .description_text
                    .as_deref()
                    .map(|value| value.chars().count())
                    .unwrap_or(0);
                ((actual as u64) < rule.value).then_some((index, actual))
            })
    {
        diagnostics.push(acceptance_diagnostic(
            "acceptance_min_description_length_not_met",
            "Discovery descriptionText is shorter than required",
            format!("{}/acceptWhen/{}", rule.owner_path, DESCRIPTOR.key),
            strategy_key,
            json!({ "minDescriptionLength": rule.value, "actualLength": actual, "itemIndex": item_index }),
        ));
        return false;
    }
    true
}

pub(super) fn evaluate_detail(
    patch: &DetailPatch,
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(rule) = stricter(
        phase.and_then(|plan| plan.min_description_length),
        strategy.and_then(|plan| plan.min_description_length),
        "/detail",
        strategy_path,
    ) else {
        return true;
    };
    let actual = patch
        .description_text
        .as_deref()
        .map(|value| value.chars().count())
        .unwrap_or(0);
    if (actual as u64) < rule.value {
        diagnostics.push(acceptance_diagnostic(
            "description_too_short",
            "Detail descriptionText is shorter than required",
            format!("{}/acceptWhen/{}", rule.owner_path, DESCRIPTOR.key),
            strategy_key,
            json!({ "minDescriptionLength": rule.value, "actualLength": actual }),
        ));
        return false;
    }
    true
}

pub(super) fn validate_detail_request(
    plan: &CompiledAcceptance,
    path: &str,
    strategy_key: Option<&str>,
    requested: &RequestedDetailFields,
) -> Option<Diagnostic> {
    if plan.min_description_length.is_some() && !requested.contains(DetailField::DescriptionText) {
        return Some(acceptance_diagnostic(
            "acceptance_field_not_requested",
            "Detail minDescriptionLength requires descriptionText in this invocation",
            format!("{path}/acceptWhen/{}", DESCRIPTOR.key),
            strategy_key,
            json!({ "field": "descriptionText" }),
        ));
    }
    None
}
