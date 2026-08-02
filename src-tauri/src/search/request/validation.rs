use std::collections::HashMap;

use regex::Regex;

use super::{
    SearchRequestStatus, SearchRule, SearchRuleInput, SearchRuleKind, SearchRuleTarget,
    ValidationIssue, ValidationIssueCode,
};

const MAX_RADIUS_KM: i64 = 9_007_199_254_740_991;
const MAX_VALIDATION_ISSUES: usize = 64;
const MAX_REPORTED_VALIDATION_ISSUES: usize = MAX_VALIDATION_ISSUES - 1;

pub(super) struct NormalizedSearchRequestInput {
    pub(super) status: SearchRequestStatus,
    pub(super) include_rules: Vec<SearchRule>,
    pub(super) exclude_rules: Vec<SearchRule>,
    pub(super) locations: Vec<String>,
    pub(super) radius_km: Option<i64>,
    pub(super) source_keys: Vec<String>,
}

pub(super) fn normalize_search_request_input(
    status: SearchRequestStatus,
    include_rules: Vec<SearchRuleInput>,
    exclude_rules: Vec<SearchRuleInput>,
    locations: Vec<String>,
    radius_km: Option<i64>,
    source_keys: Vec<String>,
) -> Result<NormalizedSearchRequestInput, String> {
    let include_rules = normalize_rules(include_rules, "includeRules")?;
    let exclude_rules = normalize_rules(exclude_rules, "excludeRules")?;

    if radius_km.is_some_and(|radius_km| radius_km < 0) {
        return Err("radiusKm must be greater than or equal to 0".to_string());
    }
    if radius_km.is_some_and(|radius_km| radius_km > MAX_RADIUS_KM) {
        return Err(format!("radiusKm must be at most {MAX_RADIUS_KM}"));
    }

    let source_keys = normalize_source_keys(source_keys)?;
    let validation_issues = derive_issues(&include_rules, &exclude_rules, &source_keys);
    ensure_status_allows_issues(status, &validation_issues)?;

    Ok(NormalizedSearchRequestInput {
        status,
        include_rules,
        exclude_rules,
        locations: normalize_locations(locations),
        radius_km,
        source_keys,
    })
}

pub(super) fn derive_issues(
    include_rules: &[SearchRule],
    exclude_rules: &[SearchRule],
    source_keys: &[String],
) -> Vec<ValidationIssue> {
    let mut issues = IssueCollector::default();
    collect_invalid_regex_issues(include_rules, "includeRules", &mut issues);
    collect_invalid_regex_issues(exclude_rules, "excludeRules", &mut issues);

    if include_rules.is_empty() {
        issues.push(ValidationIssue::new(
            ValidationIssueCode::IncludeRuleRequired,
            "/includeRules",
            "At least one Include Rule is required.",
        ));
    }
    if source_keys.is_empty() {
        issues.push(ValidationIssue::new(
            ValidationIssueCode::SourceKeyRequired,
            "/sourceKeys",
            "At least one Source key is required.",
        ));
    }

    let mut first_index_by_key = HashMap::new();
    for (index, source_key) in source_keys.iter().enumerate() {
        if let Some(first_index) = first_index_by_key.get(source_key) {
            issues.push(ValidationIssue::new(
                ValidationIssueCode::DuplicateSourceKey,
                format!("/sourceKeys/{index}"),
                format!(
                    "Source key duplicates /sourceKeys/{first_index}; remove the duplicate entry."
                ),
            ));
        } else {
            first_index_by_key.insert(source_key, index);
        }
    }

    issues.finish()
}

fn ensure_status_allows_issues(
    status: SearchRequestStatus,
    issues: &[ValidationIssue],
) -> Result<(), String> {
    if status != SearchRequestStatus::Active || issues.is_empty() {
        return Ok(());
    }

    let summary = issues
        .iter()
        .map(|issue| format!("{} at {}", issue.code.as_str(), issue.path))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "active Search Requests require valid authored criteria: {summary}"
    ))
}

fn collect_invalid_regex_issues(rules: &[SearchRule], field: &str, issues: &mut IssueCollector) {
    for (index, rule) in rules.iter().enumerate() {
        if rule.kind == SearchRuleKind::Regex && Regex::new(&rule.value).is_err() {
            issues.push(ValidationIssue::new(
                ValidationIssueCode::InvalidRegex,
                format!("/{field}/{index}/value"),
                "Rule value must be a valid regular expression.",
            ));
        }
    }
}

fn normalize_rules(rules: Vec<SearchRuleInput>, field: &str) -> Result<Vec<SearchRule>, String> {
    rules
        .into_iter()
        .enumerate()
        .map(|(index, rule)| {
            let path = format!("{field}[{index}]");
            let target = SearchRuleTarget::try_from(rule.target.as_str())
                .map_err(|error| format!("{path}.target {error}"))?;
            let kind = SearchRuleKind::try_from(rule.kind.as_str())
                .map_err(|error| format!("{path}.kind {error}"))?;
            let value = rule.value.trim().to_string();
            if value.is_empty() {
                return Err(format!("{path}.value must not be empty"));
            }
            Ok(SearchRule {
                target,
                kind,
                value,
            })
        })
        .collect()
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

#[derive(Default)]
struct IssueCollector {
    issues: Vec<ValidationIssue>,
    truncated: bool,
}

impl IssueCollector {
    fn push(&mut self, issue: ValidationIssue) {
        if self.issues.len() < MAX_REPORTED_VALIDATION_ISSUES {
            self.issues.push(issue);
        } else {
            self.truncated = true;
        }
    }

    fn finish(mut self) -> Vec<ValidationIssue> {
        if self.truncated {
            self.issues.push(ValidationIssue::new(
                ValidationIssueCode::IssuesTruncated,
                "",
                "Additional validation issues were omitted.",
            ));
        }
        self.issues
    }
}
