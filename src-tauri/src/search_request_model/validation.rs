use regex::Regex;
use sqlx::SqlitePool;

use super::{SearchRequestStatus, SearchRule, SearchRuleInput, SearchRuleKind, SearchRuleTarget};

pub(super) struct NormalizedSearchRequestInput {
    pub(super) status: SearchRequestStatus,
    pub(super) include_rules: Vec<SearchRule>,
    pub(super) exclude_rules: Vec<SearchRule>,
    pub(super) locations: Vec<String>,
    pub(super) radius_km: Option<i64>,
    pub(super) source_keys: Vec<String>,
    pub(super) validation_error: Option<String>,
}

pub(super) async fn validate_search_request_input(
    _pool: &SqlitePool,
    status: SearchRequestStatus,
    include_rules: Vec<SearchRuleInput>,
    exclude_rules: Vec<SearchRuleInput>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_keys: Vec<String>,
) -> Result<NormalizedSearchRequestInput, String> {
    let (include_rules, mut validation_errors) = normalize_rules(include_rules, "includeRules")?;
    let (exclude_rules, exclude_validation_errors) =
        normalize_rules(exclude_rules, "excludeRules")?;
    validation_errors.extend(exclude_validation_errors);

    if let Some(radius_km) = radius_km {
        if radius_km < 0 {
            return Err("radiusKm must be greater than or equal to 0".to_string());
        }
    }

    let source_keys = normalize_source_keys(source_keys)?;

    if status == SearchRequestStatus::Active {
        if include_rules.is_empty() {
            return Err(
                "active/executable search requests require at least one include rule".to_string(),
            );
        }
        if source_keys.is_empty() {
            return Err(
                "active/executable search requests require at least one sourceKey".to_string(),
            );
        }
    }

    let validation_error = if validation_errors.is_empty() {
        None
    } else {
        Some(validation_errors.join("; "))
    };

    if status == SearchRequestStatus::Active {
        if let Some(validation_error) = &validation_error {
            return Err(format!(
                "active/executable search requests cannot have validationError: {validation_error}"
            ));
        }
    }

    Ok(NormalizedSearchRequestInput {
        status,
        include_rules,
        exclude_rules,
        locations: normalize_locations(locations),
        radius_km,
        source_keys,
        validation_error,
    })
}

fn normalize_rules(
    rules: Vec<SearchRuleInput>,
    field: &str,
) -> Result<(Vec<SearchRule>, Vec<String>), String> {
    let mut normalized_rules = Vec::with_capacity(rules.len());
    let mut validation_errors = Vec::new();

    for (index, rule) in rules.into_iter().enumerate() {
        let path = format!("{field}[{index}]");
        let target = SearchRuleTarget::try_from(rule.target.as_str())
            .map_err(|error| format!("{path}.target {error}"))?;
        let kind = SearchRuleKind::try_from(rule.kind.as_str())
            .map_err(|error| format!("{path}.kind {error}"))?;
        let value = rule.value.trim().to_string();
        if value.is_empty() {
            return Err(format!("{path}.value must not be empty"));
        }

        if kind == SearchRuleKind::Regex {
            if let Err(error) = Regex::new(&value) {
                validation_errors.push(format!("{path}.value is invalid regex: {error}"));
            }
        }

        normalized_rules.push(SearchRule {
            target,
            kind,
            value,
        });
    }

    Ok((normalized_rules, validation_errors))
}

fn normalize_locations(locations: Vec<String>) -> Vec<String> {
    locations
        .into_iter()
        .map(|location| location.trim().to_string())
        .filter(|location| !location.is_empty())
        .collect()
}

fn normalize_source_keys(source_keys: Vec<String>) -> Result<Vec<String>, String> {
    source_keys
        .into_iter()
        .enumerate()
        .map(|(index, source_key)| {
            let source_key = source_key.trim().to_string();
            validate_source_key_value(&source_key, &format!("sourceKeys[{index}]"))?;
            Ok(source_key)
        })
        .collect()
}

fn validate_source_key_value(source_key: &str, path: &str) -> Result<(), String> {
    if source_key.is_empty() {
        return Err(format!("{path} must be a non-empty source key"));
    }

    if source_key.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(format!("{path} must match ^[a-z0-9_]+$"))
    }
}
