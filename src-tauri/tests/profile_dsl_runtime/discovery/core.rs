use super::*;

#[test]
fn compiled_discovery_runtime_returns_one_normalized_candidate() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = fake_fetcher([(
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
    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Senior Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .company
            .as_deref()
            .unwrap(),
        "Example GmbH"
    );
    assert_eq!(
        result.payload.candidates[0].reference.provider_url,
        "https://example.test/jobs/1"
    );
    assert_eq!(fetcher.requests()[0].url, "https://example.test/jobs.json");
    assert_eq!(fetcher.requests()[0].timeout_ms, 10_000);
}

#[test]
fn compiled_discovery_runtime_uses_the_canonical_named_capture_output() {
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        json!({
            "title": { "type": "capture", "key": "title" },
            "company": { "type": "json_path", "jsonPath": "$.company" },
            "url": { "type": "json_path", "jsonPath": "$.url" }
        }),
        "https://example.test/jobs.json",
        serde_json::Map::from_iter([(
            "captures".to_string(),
            json!({
                "title": {
                    "from": { "type": "json_path", "jsonPath": "$.title" },
                    "pattern": "^prefix: (?<title>.+)$"
                }
            }),
        )]),
    );
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "prefix: Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_selects_multiple_json_items() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = fake_fetcher([(
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
    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
    assert_eq!(
        result.payload.candidates[1]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Frontend Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_reports_required_field_and_cardinality_diagnostics() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = fake_fetcher([(
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

    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(result.payload.candidates[0].provider_values.title, None);
    assert_eq!(result.payload.candidates[0].provider_values.company, None);
    assert_runtime_diagnostic(&result.diagnostics[0], "field_cardinality_mismatch");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/extract/providerValues/company"
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
    let fetcher = fake_fetcher([(
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
    assert_eq!(result.payload.candidates.len(), 1);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Rust Engineer"
    );
}

#[test]
fn compiled_discovery_runtime_preserves_successful_items_with_partial_diagnostics() {
    let plan = compiled_json_discovery_plan(default_fields(), default_select());
    let fetcher = fake_fetcher([(
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

    assert_eq!(result.payload.candidates.len(), 2);
    assert_eq!(
        result.payload.candidates[0]
            .provider_values
            .title
            .as_deref(),
        Some("Rust Engineer")
    );
    assert_eq!(result.payload.candidates[1].provider_values.title, None);
    assert!(result.diagnostics.is_empty());
}
