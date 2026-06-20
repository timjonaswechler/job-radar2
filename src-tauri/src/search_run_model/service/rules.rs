use regex::Regex;

use crate::search_request_model::{SearchRule, SearchRuleKind, SearchRuleTarget};

use super::super::SourceCandidate;

pub(super) struct CompiledRule {
    target: SearchRuleTarget,
    matcher: CompiledRuleMatcher,
}

pub(super) enum CompiledRuleMatcher {
    Text(String),
    Regex(Regex),
}

pub(super) fn compile_rules(
    rules: &[SearchRule],
    field: &str,
) -> Result<Vec<CompiledRule>, String> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let matcher = match rule.kind {
                SearchRuleKind::Text => CompiledRuleMatcher::Text(rule.value.to_lowercase()),
                SearchRuleKind::Regex => {
                    CompiledRuleMatcher::Regex(Regex::new(&rule.value).map_err(|error| {
                        format!("{field}[{index}].value saved regex is invalid: {error}")
                    })?)
                }
            };
            Ok(CompiledRule {
                target: rule.target,
                matcher,
            })
        })
        .collect()
}

pub(super) fn matches_any_rule(rules: &[CompiledRule], candidate: &SourceCandidate) -> bool {
    rules.iter().any(|rule| matches_rule(rule, candidate))
}

fn matches_rule(rule: &CompiledRule, candidate: &SourceCandidate) -> bool {
    let value = match rule.target {
        SearchRuleTarget::Title => candidate.title.as_str(),
    };

    match &rule.matcher {
        CompiledRuleMatcher::Text(needle) => value.to_lowercase().contains(needle),
        CompiledRuleMatcher::Regex(regex) => regex.is_match(value),
    }
}
