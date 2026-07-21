use super::*;

#[test]
fn compiled_discovery_runtime_falls_back_to_first_accepted_strategy() {
    let plan = compiled_discovery_plan_with_strategies(
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
                "extract": discovery_extract(default_fields()),
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
                "extract": discovery_extract(default_fields()),
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
                "extract": discovery_extract(default_fields())
            }),
        ],
    );
    let fetcher = fake_fetcher([
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

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Fallback Engineer"
    );
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
        "/discovery/strategies/0/acceptWhen/minResults"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("empty_api")
    );
}

#[test]
fn compiled_discovery_runtime_falls_back_after_paginated_strategy_level_error() {
    let plan = compiled_discovery_plan_with_strategies(
        None,
        vec![
            json!({
                "key": "paginated_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/paginated.json",
                    "timeoutMs": 10000
                },
                "pagination": {
                    "type": "page",
                    "pageParam": "page",
                    "limits": { "maxRequests": 2 }
                },
                "parse": { "type": "json" },
                "select": default_select(),
                "extract": discovery_extract(default_fields())
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
                "extract": discovery_extract(default_fields())
            }),
        ],
    );
    let fetcher = ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/paginated.json?page=1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({
                    "jobs": [
                        { "title": "Partial Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
                    ]
                })
                .to_string()
                .into_bytes(),
            )],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/paginated.json?page=2".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Failure(
                ProfileHttpFailureKind::BodyStream,
            )],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/fallback.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({
                    "jobs": [
                        { "title": "Fallback Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/2" }
                    ]
                })
                .to_string()
                .into_bytes(),
            )],
            content_length: None,
        },
    ]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Fallback Engineer"
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "fetch_failed");
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("paginated_api")
    );
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/paginated.json?page=1".to_string(),
            "https://example.test/paginated.json?page=2".to_string(),
            "https://example.test/fallback.json".to_string(),
        ]
    );
}

#[test]
fn compiled_discovery_runtime_combines_step_and_strategy_acceptance() {
    let plan = compiled_discovery_plan_with_strategies(
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
            "extract": discovery_extract(default_fields()),
            "acceptWhen": { "minResults": 1 }
        })],
    );
    let fetcher = fake_fetcher([(
        "https://example.test/one-result.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(result.diagnostics[0].code, "acceptance_min_results_not_met");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/acceptWhen/minResults"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
    assert_eq!(result.diagnostics[1].path, "/discovery/strategies");
}

#[test]
fn compiled_discovery_runtime_applies_required_fields_and_description_length() {
    let mut fields_with_description = default_fields();
    fields_with_description["descriptionText"] =
        json!({ "type": "json_path", "jsonPath": "$.description", "cardinality": "optional" });
    let plan = compiled_discovery_plan_with_strategies(
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
                "extract": discovery_extract(default_fields()),
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
                "extract": discovery_extract(fields_with_description.clone()),
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
                "extract": discovery_extract(fields_with_description),
                "acceptWhen": { "requiredFields": ["descriptionText"], "minDescriptionLength": 20 }
            }),
        ],
    );
    let fetcher = fake_fetcher([
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

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0]
            .provider_values
            .title
            .as_deref()
            .unwrap(),
        "Platform Engineer"
    );
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(
        result.diagnostics[0].code,
        "acceptance_required_field_missing"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/acceptWhen/requiredFields"
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
        "/discovery/strategies/1/acceptWhen/minDescriptionLength"
    );
}

#[test]
fn compiled_discovery_runtime_reports_unsupported_max_error_ratio() {
    let plan = compiled_discovery_plan_with_strategies(
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
            "extract": discovery_extract(default_fields()),
            "acceptWhen": { "maxErrorRatio": 0.25 }
        })],
    );
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                { "title": "Rust Engineer", "company": "Example GmbH", "url": "https://example.test/jobs/1" }
            ]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_eq!(
        result.diagnostics[0].code,
        "acceptance_max_error_ratio_unsupported"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/acceptWhen/maxErrorRatio"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
}

fn compiled_discovery_plan_with_strategies(
    step_accept_when: Option<Value>,
    strategies: Vec<Value>,
) -> SourceExecutionPlan {
    let mut discovery = json!({
        "policy": { "type": "first_accepted" },
        "strategies": strategies
    });
    if let Some(accept_when) = step_accept_when {
        discovery["acceptWhen"] = accept_when;
    }

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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
            "discovery": discovery
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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

    let result = compile_test_source(&source, Some(profile));
    unwrap_plan(result)
}
