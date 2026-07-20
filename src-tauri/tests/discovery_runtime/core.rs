use super::*;

#[test]
fn compiled_discovery_runtime_returns_one_normalized_candidate() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "  Senior   Rust\nEngineer  ",
                "company": " Example\tGmbH ",
                "url": " https://example.test/jobs/1 "
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(result.candidates[0].url, "https://example.test/jobs/1");
    assert_eq!(fetcher.requests()[0].url, "https://example.test/jobs.json");
    assert_eq!(fetcher.requests()[0].timeout_ms, 10_000);
}

#[test]
fn compiled_discovery_runtime_selects_multiple_json_items() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
}

#[test]
fn compiled_discovery_runtime_reports_required_field_and_cardinality_diagnostics() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "company": ["Example GmbH", "Example AG"],
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "required_field_missing");
    assert_runtime_diagnostic(&result.diagnostics[1], "field_cardinality_mismatch");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/extract/fields/title"
    );
    assert_eq!(
        result.diagnostics[1].path,
        "/discovery/strategies/0/extract/fields/company"
    );
}

#[test]
fn compiled_discovery_runtime_applies_where_filters_before_extraction() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "where".to_string(),
            json!([
                {
                    "type": "regex",
                    "field": { "type": "json_path", "jsonPath": "$.status", "cardinality": "one" },
                    "pattern": "^open$"
                },
                {
                    "type": "non_empty",
                    "field": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" }
                }
            ]),
        )]),
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "status": "open", "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "status": "closed", "company": "Example GmbH", "url": "https://example.test/jobs/2" },
                { "status": "open", "title": "", "company": "Example GmbH", "url": "https://example.test/jobs/3" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
}

#[test]
fn compiled_discovery_runtime_preserves_successful_items_with_partial_diagnostics() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" },
                { "company": "Example GmbH", "url": "https://example.test/jobs/2" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_runtime_diagnostic(&result.diagnostics[0], "required_field_missing");
    assert_eq!(
        result.diagnostics[0].details.as_ref().unwrap()["itemIndex"],
        1
    );
}
