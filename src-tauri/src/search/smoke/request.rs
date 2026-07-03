use sqlx::SqlitePool;

use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRule, SearchRuleInput, SearchRuleKind, SearchRuleTarget,
};

use super::constants::{
    EXCLUDE_RULE_VALUES, INCLUDE_RULE_VALUES, SCHOTT_SOURCE_KEY, SMOKE_LOCATION, SMOKE_RADIUS_KM,
};

pub(super) async fn get_or_create_smoke_search_request(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_keys: Vec<String>,
) -> Result<(SearchRequest, bool), String> {
    let service = SearchRequestService::new(pool, running_search_runs);
    for search_request in service.list().await? {
        if is_smoke_search_request(&search_request, &source_keys) {
            return Ok((search_request, false));
        }
    }

    let created = service
        .create(smoke_search_request_input(source_keys))
        .await?;
    Ok((created, true))
}

fn is_smoke_search_request(search_request: &SearchRequest, source_keys: &[String]) -> bool {
    search_request.status == SearchRequestStatus::Active
        && search_request.include_rules == expected_rules(INCLUDE_RULE_VALUES)
        && search_request.exclude_rules == expected_regex_rules(EXCLUDE_RULE_VALUES)
        && search_request.locations == vec![SMOKE_LOCATION.to_string()]
        && search_request.radius_km == Some(SMOKE_RADIUS_KM)
        && search_request.source_keys == source_keys
        && search_request.validation_error.is_none()
}

fn smoke_search_request_input(source_keys: Vec<String>) -> CreateSearchRequestInput {
    CreateSearchRequestInput {
        status: SearchRequestStatus::Active,
        include_rules: INCLUDE_RULE_VALUES
            .iter()
            .map(|value| text_rule_input(value))
            .collect(),
        exclude_rules: EXCLUDE_RULE_VALUES
            .iter()
            .map(|value| regex_rule_input(value))
            .collect(),
        locations: vec![SMOKE_LOCATION.to_string()],
        radius_km: Some(SMOKE_RADIUS_KM),
        source_keys,
    }
}

pub(super) fn expected_rules(values: &[&str]) -> Vec<SearchRule> {
    values
        .iter()
        .map(|value| SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Text,
            value: (*value).to_string(),
        })
        .collect()
}

fn expected_regex_rules(values: &[&str]) -> Vec<SearchRule> {
    values
        .iter()
        .map(|value| SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Regex,
            value: (*value).to_string(),
        })
        .collect()
}

fn text_rule_input(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "text".to_string(),
        value: value.to_string(),
    }
}

fn regex_rule_input(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "regex".to_string(),
        value: value.to_string(),
    }
}

pub(super) fn smoke_source_keys() -> Vec<String> {
    vec![SCHOTT_SOURCE_KEY.to_string()]
}
