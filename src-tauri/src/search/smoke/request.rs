use sqlx::SqlitePool;

use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRule, SearchRuleInput, SearchRuleKind, SearchRuleTarget,
};

use super::constants::SMOKE_INCLUDE_PATTERN;

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
        && search_request.include_rules == expected_smoke_rules()
        && search_request.exclude_rules.is_empty()
        && search_request.locations.is_empty()
        && search_request.radius_km.is_none()
        && search_request.source_keys == source_keys
        && search_request.validation_error.is_none()
}

fn smoke_search_request_input(source_keys: Vec<String>) -> CreateSearchRequestInput {
    CreateSearchRequestInput {
        status: SearchRequestStatus::Active,
        include_rules: vec![SearchRuleInput {
            target: "title".to_string(),
            kind: "regex".to_string(),
            value: SMOKE_INCLUDE_PATTERN.to_string(),
        }],
        exclude_rules: Vec::new(),
        locations: Vec::new(),
        radius_km: None,
        source_keys,
    }
}

pub(super) fn expected_smoke_rules() -> Vec<SearchRule> {
    vec![SearchRule {
        target: SearchRuleTarget::Title,
        kind: SearchRuleKind::Regex,
        value: SMOKE_INCLUDE_PATTERN.to_string(),
    }]
}
