use super::*;

#[test]
fn compiled_posting_discovery_runtime_falls_back_to_first_accepted_strategy() {
    let plan = compiled_posting_discovery_plan_with_strategies(
        None,
        vec![
            json!({
                "key": "empty_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/empty.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": default_fields() },
                "acceptWhen": { "minResults": 1 }
            }),
            json!({
                "key": "fallback_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/fallback.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": default_fields() },
                "acceptWhen": { "minResults": 1 }
            }),
            json!({
                "key": "unused_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/unused.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": default_fields() }
            }),
        ],
    );
    let fetcher = FakeFetcher::new([
        ("https://example.test/empty.json", json!({ "jobs": [] }).to_string()),
        (
            "https://example.test/fallback.json",
            json!({
                "jobs": [
                    { "title": "Fallback Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Fallback Engineer");
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/empty.json".to_string(),
            "https://example.test/fallback.json".to_string(),
        ]
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].category, DiagnosticCategory::Runtime);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "acceptance_min_results_not_met");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/acceptWhen/minResults"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("empty_api")
    );
}

#[test]
fn compiled_posting_discovery_runtime_combines_step_and_strategy_acceptance() {
    let plan = compiled_posting_discovery_plan_with_strategies(
        Some(json!({ "minResults": 2 })),
        vec![json!({
            "key": "json_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/one-result.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": default_select(),
            "extract": { "fields": default_fields() },
            "acceptWhen": { "minResults": 1 }
        })],
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/one-result.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(result.diagnostics[0].code, "acceptance_min_results_not_met");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/acceptWhen/minResults"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
    assert_eq!(result.diagnostics[1].path, "/postingDiscovery/strategies");
}

#[test]
fn compiled_posting_discovery_runtime_applies_required_fields_and_description_length() {
    let mut fields_with_description = default_fields();
    fields_with_description["descriptionText"] =
        json!({ "type": "json_path", "jsonPath": "$.description", "cardinality": "optional" });
    let plan = compiled_posting_discovery_plan_with_strategies(
        None,
        vec![
            json!({
                "key": "missing_description_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/missing-description.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": default_fields() },
                "acceptWhen": { "requiredFields": ["descriptionText"] }
            }),
            json!({
                "key": "short_description_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/short-description.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": fields_with_description.clone() },
                "acceptWhen": { "minDescriptionLength": 20 }
            }),
            json!({
                "key": "accepted_description_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/accepted-description.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": { "fields": fields_with_description },
                "acceptWhen": { "requiredFields": ["descriptionText"], "minDescriptionLength": 20 }
            }),
        ],
    );
    let fetcher = FakeFetcher::new([
        (
            "https://example.test/missing-description.json",
            json!({
                "jobs": [
                    { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/short-description.json",
            json!({
                "jobs": [
                    { "title": "Frontend Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2", "description": "Too short" }
                ]
            })
            .to_string(),
        ),
        (
            "https://example.test/accepted-description.json",
            json!({
                "jobs": [
                    { "title": "Platform Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/3", "description": "Long enough discovery description." }
                ]
            })
            .to_string(),
        ),
    ]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Platform Engineer");
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(
        result.diagnostics[0].code,
        "acceptance_required_field_missing"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/acceptWhen/requiredFields"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("missing_description_api")
    );
    assert_eq!(
        result.diagnostics[1].code,
        "acceptance_min_description_length_not_met"
    );
    assert_eq!(
        result.diagnostics[1].path,
        "/postingDiscovery/strategies/1/acceptWhen/minDescriptionLength"
    );
}

#[test]
fn compiled_posting_discovery_runtime_reports_unsupported_max_error_ratio() {
    let plan = compiled_posting_discovery_plan_with_strategies(
        None,
        vec![json!({
            "key": "json_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": default_select(),
            "extract": { "fields": default_fields() },
            "acceptWhen": { "maxErrorRatio": 0.25 }
        })],
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(
        result.diagnostics[0].code,
        "acceptance_max_error_ratio_unsupported"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/acceptWhen/maxErrorRatio"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
}

fn compiled_posting_discovery_plan_with_strategies(
    step_accept_when: Option<Value>,
    strategies: Vec<Value>,
) -> SourceExecutionPlan {
    let mut posting_discovery = json!({ "strategies": strategies });
    if let Some(accept_when) = step_accept_when {
        posting_discovery["acceptWhen"] = accept_when;
    }

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "fallback_jobs",
        "name": "Fallback Jobs",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Fallback runtime fixture profile."
        },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "json_feed",
            "name": "JSON feed",
            "postingDiscovery": posting_discovery
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "fallback_source",
        "name": "Fallback Source",
        "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/jobs.json" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "fallback_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "fallback_source",
    );
    assert_eq!(result.diagnostics, Vec::new());
    result.execution_plan.expect("fixture plan should compile")
}
