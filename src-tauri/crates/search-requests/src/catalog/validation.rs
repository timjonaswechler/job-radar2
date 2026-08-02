use std::collections::HashMap;

use regex::Regex;
use search_resolution::{SearchRule, SearchRuleKind};

use super::{Error, Input, Status, Validation, ValidationIssue, ValidationIssueCode};

const MAX_RADIUS_KM: i64 = 9_007_199_254_740_991;
const MAX_VALIDATION_ISSUES: usize = 64;
const MAX_REPORTED_VALIDATION_ISSUES: usize = MAX_VALIDATION_ISSUES - 1;

pub(super) fn normalize(mut input: Input) -> Result<(Input, Validation), Error> {
    normalize_rules(&mut input.include_rules, "includeRules")?;
    normalize_rules(&mut input.exclude_rules, "excludeRules")?;
    if input.radius_km.is_some_and(|radius| radius < 0) {
        return Err(Error::InvalidInput {
            message: "radiusKm must be greater than or equal to 0".into(),
        });
    }
    if input.radius_km.is_some_and(|radius| radius > MAX_RADIUS_KM) {
        return Err(Error::InvalidInput {
            message: format!("radiusKm must be at most {MAX_RADIUS_KM}"),
        });
    }
    input.locations = input
        .locations
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    input.source_keys = input
        .source_keys
        .into_iter()
        .enumerate()
        .map(|(index, key)| {
            let key = key.trim().to_string();
            if key.is_empty() {
                return Err(Error::InvalidInput {
                    message: format!("sourceKeys[{index}] must be a non-empty source key"),
                });
            }
            if !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(Error::InvalidInput {
                    message: format!("sourceKeys[{index}] must match ^[a-z0-9_]+$"),
                });
            }
            Ok(key)
        })
        .collect::<Result<_, _>>()?;
    let validation = derive(
        &input.include_rules,
        &input.exclude_rules,
        &input.source_keys,
    );
    if input.status == Status::Active && !validation.is_valid() {
        let summary = validation
            .issues
            .iter()
            .map(|issue| format!("{} at {}", issue.code, issue.path))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::InvalidInput {
            message: format!("active Search Requests require valid authored criteria: {summary}"),
        });
    }
    Ok((input, validation))
}

pub(super) fn derive(
    include: &[SearchRule],
    exclude: &[SearchRule],
    source_keys: &[String],
) -> Validation {
    let mut issues = Collector::default();
    collect_invalid_regex(include, "includeRules", &mut issues);
    collect_invalid_regex(exclude, "excludeRules", &mut issues);
    if include.is_empty() {
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
    let mut first = HashMap::new();
    for (index, key) in source_keys.iter().enumerate() {
        if let Some(first_index) = first.get(key) {
            issues.push(ValidationIssue::new(
                ValidationIssueCode::DuplicateSourceKey,
                format!("/sourceKeys/{index}"),
                format!(
                    "Source key duplicates /sourceKeys/{first_index}; remove the duplicate entry."
                ),
            ));
        } else {
            first.insert(key, index);
        }
    }
    Validation {
        issues: issues.finish(),
    }
}

fn normalize_rules(rules: &mut [SearchRule], field: &str) -> Result<(), Error> {
    for (index, rule) in rules.iter_mut().enumerate() {
        rule.value = rule.value.trim().to_string();
        if rule.value.is_empty() {
            return Err(Error::InvalidInput {
                message: format!("{field}[{index}].value must not be empty"),
            });
        }
    }
    Ok(())
}

fn collect_invalid_regex(rules: &[SearchRule], field: &str, issues: &mut Collector) {
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

#[derive(Default)]
struct Collector {
    issues: Vec<ValidationIssue>,
    truncated: bool,
}
impl Collector {
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
