use super::*;

pub(super) fn accept_posting_detail_result(
    description: &str,
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some((ratio, owner_path)) =
        detail_first_max_error_ratio(step_acceptance, strategy_acceptance, base_path)
    {
        diagnostics.push(runtime_error(
            "acceptance_max_error_ratio_unsupported",
            "acceptWhen.maxErrorRatio is not supported by the postingDetail runtime result model yet",
            format!("{owner_path}/acceptWhen/maxErrorRatio"),
            strategy_key,
            json!({ "maxErrorRatio": ratio }),
        ));
        return false;
    }

    for (field, owner_path) in
        detail_required_field_rules(step_acceptance, strategy_acceptance, base_path)
    {
        if field != "descriptionText" || description.trim().is_empty() {
            diagnostics.push(runtime_error(
                "acceptance_required_field_missing",
                format!("postingDetail result is missing required normalized field `{field}`"),
                format!("{owner_path}/acceptWhen/requiredFields"),
                strategy_key,
                json!({ "field": field }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = detail_stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_description_length),
        strategy_acceptance.and_then(|acceptance| acceptance.min_description_length),
        base_path,
    ) {
        if description.chars().count() < minimum as usize {
            diagnostics.push(runtime_error(
                "description_too_short",
                format!(
                    "postingDetail descriptionText is shorter than the configured minimum of {minimum} characters"
                ),
                format!("{owner_path}/acceptWhen/minDescriptionLength"),
                strategy_key,
                json!({
                    "minDescriptionLength": minimum,
                    "actualLength": description.chars().count(),
                }),
            ));
            return false;
        }
    }

    true
}

fn detail_first_max_error_ratio(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Option<(f64, String)> {
    step_acceptance
        .and_then(|acceptance| acceptance.max_error_ratio)
        .map(|ratio| (ratio, "/postingDetail".to_string()))
        .or_else(|| {
            strategy_acceptance
                .and_then(|acceptance| acceptance.max_error_ratio)
                .map(|ratio| (ratio, base_path.to_string()))
        })
}

fn detail_required_field_rules(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    if let Some(fields) = step_acceptance.and_then(|acceptance| acceptance.required_fields.as_ref())
    {
        rules.extend(
            fields
                .iter()
                .map(|field| (field.clone(), "/postingDetail".to_string())),
        );
    }
    if let Some(fields) =
        strategy_acceptance.and_then(|acceptance| acceptance.required_fields.as_ref())
    {
        for field in fields {
            if !rules.iter().any(|(existing, _)| existing == field) {
                rules.push((field.clone(), base_path.to_string()));
            }
        }
    }
    rules
}

fn detail_stricter_u64_acceptance(
    step_value: Option<u64>,
    strategy_value: Option<u64>,
    base_path: &str,
) -> Option<(u64, String)> {
    match (step_value, strategy_value) {
        (Some(step), Some(strategy)) if strategy >= step => Some((strategy, base_path.to_string())),
        (Some(step), Some(_)) | (Some(step), None) => Some((step, "/postingDetail".to_string())),
        (None, Some(strategy)) => Some((strategy, base_path.to_string())),
        (None, None) => None,
    }
}
