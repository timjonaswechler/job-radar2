use search_requests::{Catalog, Input, Record, Status};
use search_resolution::{SearchRule, SearchRuleKind, SearchRuleTarget};

use super::constants::SMOKE_INCLUDE_PATTERN;

pub(super) async fn get_or_create_smoke_search_request(
    catalog: &Catalog,
    source_keys: Vec<String>,
) -> Result<(Record, bool), String> {
    for request in catalog.list().await.map_err(|error| error.to_string())? {
        if is_smoke_search_request(&request, &source_keys) {
            return Ok((request, false));
        }
    }
    let created = catalog
        .create(smoke_search_request_input(source_keys))
        .await
        .map_err(|error| error.to_string())?;
    Ok((created, true))
}

fn is_smoke_search_request(request: &Record, source_keys: &[String]) -> bool {
    request.status == Status::Active
        && request.include_rules == expected_smoke_rules()
        && request.exclude_rules.is_empty()
        && request.locations.is_empty()
        && request.radius_km.is_none()
        && request.source_keys == source_keys
        && request.validation.is_valid()
}

fn smoke_search_request_input(source_keys: Vec<String>) -> Input {
    Input {
        status: Status::Active,
        include_rules: expected_smoke_rules(),
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
