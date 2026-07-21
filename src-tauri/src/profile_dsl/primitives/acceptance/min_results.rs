use serde_json::json;

use super::*;

pub(super) const DESCRIPTOR: AcceptanceDescriptor = AcceptanceDescriptor {
    key: "minResults",
    phases: &[AcceptancePhase::Discovery],
};

pub(super) fn validate_placement(
    value: Option<u64>,
    phase: AcceptancePhase,
) -> Result<(), AcceptanceCompileError> {
    if value.is_some() && !key_is_admitted(DESCRIPTOR.key, phase) {
        return Err(AcceptanceCompileError {
            phase,
            key: DESCRIPTOR.key,
            field: None,
            message: "minResults is available only in Discovery acceptance".into(),
        });
    }
    Ok(())
}

pub(super) fn evaluate_discovery(
    candidates: &[PostingOccurrence],
    phase: Option<&CompiledAcceptance>,
    strategy: Option<&CompiledAcceptance>,
    strategy_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(rule) = stricter(
        phase.and_then(|plan| plan.min_results),
        strategy.and_then(|plan| plan.min_results),
        "/discovery",
        strategy_path,
    ) else {
        return true;
    };
    if (candidates.len() as u64) < rule.value {
        diagnostics.push(acceptance_diagnostic(
            "acceptance_min_results_not_met",
            "Discovery returned fewer occurrences than required",
            format!("{}/acceptWhen/{}", rule.owner_path, DESCRIPTOR.key),
            strategy_key,
            json!({ "minResults": rule.value, "actualResults": candidates.len() }),
        ));
        return false;
    }
    true
}
