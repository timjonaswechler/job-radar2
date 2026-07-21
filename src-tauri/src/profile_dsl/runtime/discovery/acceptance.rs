use super::*;

pub(super) fn accept_discovery_result(
    candidates: &[PostingOccurrence],
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some((ratio, owner_path)) =
        first_max_error_ratio(step_acceptance, strategy_acceptance, base_path)
    {
        diagnostics.push(runtime_error(
            "acceptance_max_error_ratio_unsupported",
            "acceptWhen.maxErrorRatio is not supported by the discovery runtime result model yet",
            format!("{owner_path}/acceptWhen/maxErrorRatio"),
            strategy_key,
            json!({ "maxErrorRatio": ratio }),
        ));
        return false;
    }

    for (field, owner_path) in required_field_rules(step_acceptance, strategy_acceptance, base_path)
    {
        if let Some((item_index, _)) = candidates
            .iter()
            .enumerate()
            .find(|(_, candidate)| !discovery_field_present(candidate, &field))
        {
            diagnostics.push(runtime_error(
                "acceptance_required_field_missing",
                format!("discovery candidate is missing required normalized field `{field}`"),
                format!("{owner_path}/acceptWhen/requiredFields"),
                strategy_key,
                json!({ "field": field, "itemIndex": item_index }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_description_length),
        strategy_acceptance.and_then(|acceptance| acceptance.min_description_length),
        base_path,
    ) {
        if let Some((item_index, actual_length)) =
            candidates
                .iter()
                .enumerate()
                .find_map(|(item_index, candidate)| {
                    let description = candidate.provider_values.description_text.as_ref()?;
                    let actual_length = description.chars().count() as u64;
                    (actual_length < minimum).then_some((item_index, actual_length))
                })
        {
            diagnostics.push(runtime_error(
                "acceptance_min_description_length_not_met",
                format!(
                    "discovery descriptionText is shorter than the configured minimum of {minimum} characters"
                ),
                format!("{owner_path}/acceptWhen/minDescriptionLength"),
                strategy_key,
                json!({
                    "minDescriptionLength": minimum,
                    "actualLength": actual_length,
                    "itemIndex": item_index,
                }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_results),
        strategy_acceptance.and_then(|acceptance| acceptance.min_results),
        base_path,
    ) {
        if candidates.len() < minimum as usize {
            diagnostics.push(runtime_error(
                "acceptance_min_results_not_met",
                format!(
                    "discovery returned fewer than the required minimum of {minimum} candidates"
                ),
                format!("{owner_path}/acceptWhen/minResults"),
                strategy_key,
                json!({
                    "minResults": minimum,
                    "actualResults": candidates.len(),
                }),
            ));
            return false;
        }
    }

    true
}

fn first_max_error_ratio(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Option<(f64, String)> {
    step_acceptance
        .and_then(|acceptance| acceptance.max_error_ratio)
        .map(|ratio| (ratio, "/discovery".to_string()))
        .or_else(|| {
            strategy_acceptance
                .and_then(|acceptance| acceptance.max_error_ratio)
                .map(|ratio| (ratio, base_path.to_string()))
        })
}

fn required_field_rules(
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
                .map(|field| (field.clone(), "/discovery".to_string())),
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

fn discovery_field_present(candidate: &PostingOccurrence, field: &str) -> bool {
    match field {
        "title" => candidate
            .provider_values
            .title
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        "company" => candidate
            .provider_values
            .company
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        "url" => !candidate.reference.provider_url.is_empty(),
        "descriptionText" => candidate
            .provider_values
            .description_text
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        "locations" => !candidate.provider_values.locations.is_empty(),
        field => field
            .strip_prefix("postingMeta.")
            .and_then(|key| candidate.posting_meta.get(key))
            .is_some_and(|value| !value.trim().is_empty()),
    }
}

fn stricter_u64_acceptance(
    step_value: Option<u64>,
    strategy_value: Option<u64>,
    base_path: &str,
) -> Option<(u64, String)> {
    match (step_value, strategy_value) {
        (Some(step), Some(strategy)) if strategy >= step => Some((strategy, base_path.to_string())),
        (Some(step), Some(_)) | (Some(step), None) => Some((step, "/discovery".to_string())),
        (None, Some(strategy)) => Some((strategy, base_path.to_string())),
        (None, None) => None,
    }
}
