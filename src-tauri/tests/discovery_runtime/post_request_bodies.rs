use super::*;

#[test]
fn compiled_discovery_runtime_posts_rendered_json_body_and_public_headers() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "fetch".to_string(),
        json!({
            "mode": "http",
            "method": "POST",
            "url": "{{sourceConfig:feedUrl}}",
            "headers": {
                "accept": "application/json",
                "content-type": "application/json",
                "x-requested-with": "XMLHttpRequest"
            },
            "body": {
                "type": "json",
                "value": {
                    "limit": 25,
                    "tenant": "{{source:name}}",
                    "feed": "{{sourceConfig:feedUrl}}"
                }
            },
            "timeoutMs": 12000
        }),
    );
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    let request = &fetcher.requests()[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert_eq!(request.url, "https://example.test/jobs.json");
    assert_eq!(request.timeout_ms, 12_000);
    assert_eq!(
        request.headers,
        vec![
            ("accept".to_string(), b"application/json".to_vec()),
            ("content-type".to_string(), b"application/json".to_vec()),
            ("x-requested-with".to_string(), b"XMLHttpRequest".to_vec()),
        ]
    );
    let body = request.body.as_ref().expect("rendered JSON body");
    assert_eq!(
        body.bytes(),
        br#"{"feed":"https://example.test/jobs.json","limit":25,"tenant":"Example Source"}"#
    );
    assert_eq!(body.default_content_type(), Some("application/json"));
}

#[test]
fn compiled_discovery_runtime_posts_rendered_text_body() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "fetch".to_string(),
        json!({
            "mode": "http",
            "method": "POST",
            "url": "{{sourceConfig:feedUrl}}",
            "body": {
                "type": "text",
                "value": "source={{source:name}}&feed={{sourceConfig:feedUrl}}"
            },
            "timeoutMs": 10000
        }),
    );
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    let requests = fetcher.requests();
    let body = requests[0].body.as_ref().expect("rendered text body");
    assert_eq!(
        body.bytes(),
        b"source=Example Source&feed=https://example.test/jobs.json"
    );
    assert_eq!(body.default_content_type(), None);
}

#[test]
fn compiled_discovery_runtime_reports_body_template_rendering_failure() {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "example_jobs",
        "name": "Example Jobs",
        "kind": "generic",
        "support": { "level": "experimental" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": {
                "feedUrl": { "type": "string" },
                "tenant": { "type": "string" }
            },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "json_feed",
            "name": "JSON feed",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "POST",
                        "url": "{{sourceConfig:feedUrl}}",
                        "body": {
                            "type": "json",
                            "value": { "tenant": "{{sourceConfig:tenant}}" }
                        },
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": { "fields": default_fields() }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "example_source",
        "name": "Example Source",
        "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/jobs.json" },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();
    let compile_result = compile_test_source(&source, Some(profile));
    let plan = unwrap_plan(compile_result);
    let fetcher = fake_fetcher([]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "fetch_body_template_failed");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/fetch/body"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("json_api")
    );
    assert!(fetcher.requests().is_empty());
}

#[test]
fn compiled_discovery_runtime_reports_get_body_combination() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "fetch".to_string(),
        json!({
            "mode": "http",
            "method": "GET",
            "url": "{{sourceConfig:feedUrl}}",
            "body": { "type": "text", "value": "not allowed on GET" },
            "timeoutMs": 10000
        }),
    );
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );
    let fetcher = fake_fetcher([]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "unsupported_http_body_for_method");
    assert_eq!(
        result.diagnostics[0].path,
        "/discovery/strategies/0/fetch/body"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("json_api")
    );
    assert!(fetcher.requests().is_empty());
}

#[test]
fn compiled_discovery_runtime_posts_rendered_form_body() {
    let mut extra = serde_json::Map::new();
    extra.insert(
        "fetch".to_string(),
        json!({
            "mode": "http",
            "method": "POST",
            "url": "{{sourceConfig:feedUrl}}",
            "body": {
                "type": "form",
                "fields": {
                    "source": "{{source:name}}",
                    "feed": "{{sourceConfig:feedUrl}}"
                }
            },
            "timeoutMs": 10000
        }),
    );
    let plan = compiled_discovery_plan_with_strategy(
        json!({ "type": "json" }),
        default_select(),
        default_fields(),
        "https://example.test/jobs.json",
        extra,
    );
    let fetcher = fake_fetcher([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_discovery_test(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    let requests = fetcher.requests();
    let body = requests[0].body.as_ref().expect("rendered form body");
    assert_eq!(
        body.bytes(),
        b"feed=https%3A%2F%2Fexample.test%2Fjobs.json&source=Example+Source"
    );
    assert_eq!(
        body.default_content_type(),
        Some("application/x-www-form-urlencoded")
    );
}
