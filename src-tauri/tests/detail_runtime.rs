mod support;

use support::{compile_test_source, execute_detail_test, unwrap_plan};

use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use job_radar_lib::{
    execute_detail, DetailFetchError, DetailFetchRequest, DetailFetchResponse, DetailFetcher,
    DetailPostingOccurrence, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, HttpMethod, ProfileBrowserClient,
    ProfileBrowserFetchError, ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest,
    ProfileBrowserFetchResponse, RequestBody, RuntimeCancellation, RuntimeExecutionContext,
    SourceDocument, SourceExecutionPlan, SourceProfileDocument,
};
use serde_json::{json, Value};
use tokio::sync::Notify;

#[test]
fn compiled_detail_runtime_extracts_direct_json_description_text() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.json",
        json!({ "description": "First paragraph.\n\nSecond paragraph." }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
    assert_eq!(
        fetcher.requests()[0].url,
        "https://example.test/jobs/42.json"
    );
    assert_eq!(fetcher.requests()[0].timeout_ms, 10_000);
}

#[test]
fn compiled_detail_runtime_falls_back_to_first_accepted_strategy() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![
            json!({
                "key": "short_detail_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs/short.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "extract": {
                    "fields": {
                        "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                    }
                },
                "acceptWhen": { "minDescriptionLength": 20 }
            }),
            json!({
                "key": "fallback_detail_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs/fallback.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "extract": {
                    "fields": {
                        "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                    }
                }
            }),
        ],
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([
        (
            "https://example.test/jobs/short.json",
            json!({ "description": "Too short" }).to_string(),
        ),
        (
            "https://example.test/jobs/fallback.json",
            json!({ "description": "Fallback detail description." }).to_string(),
        ),
    ]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(
        result.description_text,
        Some("Fallback detail description.".to_string())
    );
    assert_eq!(
        fetcher
            .requests()
            .into_iter()
            .map(|request| request.url)
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/jobs/short.json".to_string(),
            "https://example.test/jobs/fallback.json".to_string(),
        ]
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "description_too_short");
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("short_detail_api")
    );
}

#[test]
fn compiled_detail_runtime_applies_where_filters_before_extraction() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![
            json!({
                "key": "filtered_detail_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs/filtered.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "where": [{
                    "type": "regex",
                    "field": { "type": "json_path", "jsonPath": "$.status", "cardinality": "one" },
                    "pattern": "^published$"
                }],
                "extract": {
                    "fields": {
                        "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                    }
                }
            }),
            json!({
                "key": "fallback_detail_api",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs/fallback.json",
                    "timeoutMs": 10000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "extract": {
                    "fields": {
                        "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                    }
                }
            }),
        ],
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([
        (
            "https://example.test/jobs/filtered.json",
            json!({ "status": "draft", "description": "This draft description must be filtered before extraction." }).to_string(),
        ),
        (
            "https://example.test/jobs/fallback.json",
            json!({ "description": "Fallback detail description." }).to_string(),
        ),
    ]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(
        result.description_text,
        Some("Fallback detail description.".to_string())
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "where_condition_not_matched");
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("filtered_detail_api")
    );
}

#[test]
fn compiled_detail_runtime_reports_invalid_where_regex_diagnostic() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "invalid_where_detail_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs/invalid-where.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "document" },
            "where": [{
                "type": "regex",
                "field": { "type": "json_path", "jsonPath": "$.status", "cardinality": "one" },
                "pattern": "["
            }],
            "extract": {
                "fields": {
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                }
            }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/invalid-where.json",
        json!({ "status": "published", "description": "This description is not extracted." })
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.description_text, None);
    assert_runtime_diagnostic(&result.diagnostics[0], "where_pattern_invalid");
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/strategies/0/where/0/pattern"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("invalid_where_detail_api")
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
}

#[test]
fn compiled_detail_runtime_combines_step_and_strategy_acceptance() {
    let plan = compiled_detail_plan_with_strategies(
        Some(json!({ "minDescriptionLength": 20 })),
        vec![json!({
            "key": "detail_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs/short.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                }
            },
            "acceptWhen": { "minDescriptionLength": 5 }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/short.json",
        json!({ "description": "Too short" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.description_text, None);
    assert_eq!(result.diagnostics.len(), 2);
    assert_runtime_diagnostic(&result.diagnostics[0], "description_too_short");
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/acceptWhen/minDescriptionLength"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
    assert_eq!(result.diagnostics[1].path, "/detail/strategies");
}

#[test]
fn compiled_detail_runtime_reports_unsupported_max_error_ratio() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "detail_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs/detail.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "descriptionText": { "type": "json_path", "jsonPath": "$.description", "cardinality": "one" }
                }
            },
            "acceptWhen": { "maxErrorRatio": 0.25 }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/detail.json",
        json!({ "description": "Detailed role description." }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.description_text, None);
    assert_eq!(
        result.diagnostics[0].code,
        "acceptance_max_error_ratio_unsupported"
    );
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/strategies/0/acceptWhen/maxErrorRatio"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
}

#[test]
fn compiled_detail_runtime_renders_fetch_templates_from_all_runtime_contexts() {
    let plan = compiled_json_detail_plan(
        "{{sourceConfig:apiBase}}/{{captures:tenant}}/{{postingMeta:jobId}}?u={{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        Some(json!({
            "tenant": {
                "from": { "type": "posting_meta", "key": "tenant", "cardinality": "one" },
                "pattern": "^(?<value>[a-z0-9_]+)$"
            }
        })),
        None,
    );
    let posting = posting_occurrence("job-42", [("jobId", "REQ-42"), ("tenant", "acme_jobs")]);
    let expected_url = "https://api.example.test/acme_jobs/REQ-42?u=job-42";
    let fetcher = FakeDetailFetcher::new([(
        expected_url,
        json!({ "description": "Rendered from all template contexts." }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Rendered from all template contexts.".to_string())
    );
    assert_eq!(fetcher.requests()[0].url, expected_url);
}

#[test]
fn compiled_detail_runtime_posts_rendered_json_body() {
    let plan = compiled_detail_plan_with_fetch(
        json!({
            "mode": "http",
            "method": "POST",
            "url": "{{sourceConfig:apiBase}}/detail",
            "headers": { "content-type": "application/json" },
            "body": {
                "type": "json",
                "value": {
                    "jobId": "{{postingMeta:jobId}}",
                    "tenant": "{{postingMeta:tenant}}",
                    "postingUrl": "{{posting:url}}",
                    "source": "{{source:name}}"
                }
            },
            "timeoutMs": 15000
        }),
        json!({ "type": "json" }),
        json!({ "type": "document" }),
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let posting = posting_occurrence(
        "https://example.test/jobs/42",
        [("jobId", "REQ-42"), ("tenant", "acme_jobs")],
    );
    let fetcher = FakeDetailFetcher::new([(
        "https://api.example.test/detail",
        json!({ "description": "Detail POST response." }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Detail POST response.".to_string())
    );
    let request = &fetcher.requests()[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert_eq!(request.url, "https://api.example.test/detail");
    assert_eq!(request.timeout_ms, 15_000);
    assert_eq!(
        request.headers,
        BTreeMap::from_iter([("content-type".to_string(), "application/json".to_string())])
    );
    assert_eq!(
        request.body,
        Some(RequestBody::Json {
            value: serde_json::Map::from_iter([
                ("jobId".to_string(), json!("REQ-42")),
                (
                    "postingUrl".to_string(),
                    json!("https://example.test/jobs/42")
                ),
                ("source".to_string(), json!("Example Source")),
                ("tenant".to_string(), json!("acme_jobs")),
            ])
        })
    );
}

#[test]
fn compiled_detail_runtime_normalizes_html_in_json_description_text() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.descriptionHtml",
            "cardinality": "one",
            "transforms": [{ "type": "html_to_text" }, { "type": "normalize_whitespace" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.json",
        json!({ "descriptionHtml": "<p>First paragraph.</p><p>Second <strong>paragraph</strong>.</p>" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_applies_explicit_text_transforms() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.descriptionSlug",
            "cardinality": "one",
            "transforms": [{ "type": "url_decode" }, { "type": "slug_to_title" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.json",
        json!({ "descriptionSlug": "senior%20rust-engineer" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Senior Rust Engineer".to_string())
    );
}

#[test]
fn compiled_detail_runtime_combines_description_text_parts() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "combine",
            "join": "\n\n",
            "parts": [
                { "value": { "type": "json_path", "jsonPath": "$.intro", "cardinality": "one" } },
                { "value": { "type": "json_path", "jsonPath": "$.body", "cardinality": "one" } },
                { "value": { "type": "json_path", "jsonPath": "$.footer", "cardinality": "one" }, "optional": true }
            ],
            "transforms": [{ "type": "normalize_whitespace" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.json",
        json!({
            "intro": "About the role.",
            "body": "Build reliable DSL runtimes."
        })
        .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("About the role. Build reliable DSL runtimes.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_reports_missing_empty_and_too_short_description_diagnostics() {
    let empty_plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let missing_result = block_on(execute_detail_test(
        &empty_plan,
        &posting_occurrence("https://example.test/jobs/missing-description.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/missing-description.json",
            json!({ "title": "Engineer" }).to_string(),
        )]),
    ));
    assert_eq!(missing_result.description_text, None);
    assert_runtime_diagnostic(&missing_result.diagnostics[0], "description_empty");

    let empty_result = block_on(execute_detail_test(
        &empty_plan,
        &posting_occurrence("https://example.test/jobs/empty.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/empty.json",
            json!({ "description": " \n \t " }).to_string(),
        )]),
    ));
    assert_eq!(empty_result.description_text, None);
    assert_runtime_diagnostic(&empty_result.diagnostics[0], "description_empty");
    assert_eq!(
        empty_result.diagnostics[0].path,
        "/detail/strategies/0/extract/fields/descriptionText"
    );

    let too_short_plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        Some(20),
    );
    let too_short_result = block_on(execute_detail_test(
        &too_short_plan,
        &posting_occurrence("https://example.test/jobs/short.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/short.json",
            json!({ "description": "Too short" }).to_string(),
        )]),
    ));
    assert_eq!(too_short_result.description_text, None);
    assert_runtime_diagnostic(&too_short_result.diagnostics[0], "description_too_short");
    assert_eq!(
        too_short_result.diagnostics[0].details.as_ref().unwrap()["minDescriptionLength"],
        20
    );
}

#[test]
fn compiled_detail_runtime_extracts_xml_description_text() {
    let plan = compiled_detail_plan(
        "{{posting:url}}",
        json!({ "type": "xml" }),
        json!({ "type": "xml_element", "element": "job" }),
        json!({ "type": "xml_text", "textPath": "description", "cardinality": "one" }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.xml", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.xml",
        r#"<jobs><job><description>First paragraph.

Second paragraph.</description></job></jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_matches_xml_detail_collection() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "xml_feed_detail",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs.xml",
                "timeoutMs": 10000
            },
            "parse": { "type": "xml" },
            "select": { "type": "xml_element", "element": "job" },
            "match": {
                "left": { "type": "xml_text", "textPath": "id", "cardinality": "one" },
                "right": { "type": "posting_meta", "key": "jobId", "cardinality": "one" }
            },
            "extract": {
                "fields": {
                    "descriptionText": { "type": "xml_text", "textPath": "description", "cardinality": "one" }
                }
            }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42", [("jobId", "42")]);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs.xml",
        r#"<jobs>
            <job><id>41</id><description>Wrong job.</description></job>
            <job><id>42</id><description>Matched XML detail description.</description></job>
        </jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Matched XML detail description.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_matches_one_item_xml_detail_collection() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "xml_feed_detail",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/one-job.xml",
                "timeoutMs": 10000
            },
            "parse": { "type": "xml" },
            "select": { "type": "xml_element", "element": "job" },
            "match": {
                "left": { "type": "xml_text", "textPath": "id", "cardinality": "one" },
                "right": { "type": "posting_meta", "key": "jobId", "cardinality": "one" }
            },
            "extract": {
                "fields": {
                    "descriptionText": { "type": "xml_text", "textPath": "description", "cardinality": "one" }
                }
            }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42", [("jobId", "42")]);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/one-job.xml",
        r#"<jobs><job><id>42</id><description>Single XML detail description.</description></job></jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Single XML detail description.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_extracts_html_description_text_with_css() {
    let plan = compiled_detail_plan(
        "{{posting:url}}",
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "main.job" }),
        json!({ "type": "css_text", "selector": ".description", "cardinality": "one" }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.html", []);
    let fetcher = FakeDetailFetcher::new([(
        "https://example.test/jobs/42.html",
        r#"<main class="job"><section class="description"><p>First paragraph.</p><p>Second paragraph.</p></section></main>"#.to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_uses_browser_fetch_rendered_html() {
    let plan = compiled_browser_detail_plan(
        "{{posting:url}}?tenant={{postingMeta:tenant}}",
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "main.job" }),
        json!({ "type": "css_text", "selector": ".description", "cardinality": "one" }),
    );
    let posting = posting_occurrence(
        "https://example.test/jobs/42.html",
        [("tenant", "acme"), ("jobId", "42")],
    );
    let fetcher = FakeDetailFetcher::new([]);
    let browser = FakeBrowser::new([(
        "https://example.test/jobs/42.html?tenant=acme",
        r#"<main class="job"><section class="description">Rendered browser detail.</section></main>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail(
        &plan,
        &posting,
        &fetcher,
        &browser,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Rendered browser detail.".to_string())
    );
    assert!(fetcher.requests().is_empty());
    let browser_requests = browser.requests();
    assert_eq!(browser_requests.len(), 1);
    assert_eq!(
        browser_requests[0].url,
        "https://example.test/jobs/42.html?tenant=acme"
    );
    assert_eq!(
        browser_requests[0].waits,
        vec![ExecutionPlanBrowserWait::Selector {
            selector: Some("main.job".to_string()),
            timeout_ms: 5000,
        }]
    );
    assert_eq!(
        browser_requests[0].interactions,
        vec![ExecutionPlanBrowserInteraction::ClickUntilGone {
            selector: "button.cookie-banner".to_string(),
            max_count: 2,
            wait_after_ms: Some(100),
        }]
    );
}

#[test]
fn compiled_detail_runtime_reports_browser_interaction_diagnostics() {
    let plan = compiled_browser_detail_plan(
        "{{posting:url}}",
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "main.job" }),
        json!({ "type": "css_text", "selector": ".description", "cardinality": "one" }),
    );
    let fetcher = FakeDetailFetcher::new([]);
    let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::InteractionFailed {
            interaction_index: Some(0),
        },
        "click_until_gone reached maxCount",
    ));

    let result = block_on(execute_detail(
        &plan,
        &posting_occurrence("https://example.test/jobs/42.html", []),
        &fetcher,
        &browser,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.description_text, None);
    assert_runtime_diagnostic(&result.diagnostics[0], "browser_interaction_failed");
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/strategies/0/fetch/interactions/0"
    );
}

#[test]
fn compiled_detail_runtime_reports_fetch_parse_extract_and_missing_context_failures() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );

    let fetch_failure = block_on(execute_detail_test(
        &plan,
        &posting_occurrence("https://example.test/jobs/missing.json", []),
        &FakeDetailFetcher::new([]),
    ));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_detail_test(
        &plan,
        &posting_occurrence("https://example.test/jobs/bad-json.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/bad-json.json",
            "{not-json".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let mut extract_plan = plan.clone();
    extract_plan.detail.as_mut().unwrap().strategies[0]
        .extract
        .fields
        .description_text = serde_json::from_value(json!({
        "type": "json_path",
        "jsonPath": "$.description[*]",
        "cardinality": "one"
    }))
    .unwrap();
    let extract_failure = block_on(execute_detail_test(
        &extract_plan,
        &posting_occurrence("https://example.test/jobs/42.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/42.json",
            json!({ "description": "Text" }).to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&extract_failure.diagnostics[0], "field_json_path_failed");

    let missing_context_plan = compiled_json_detail_plan(
        "https://example.test/{{postingMeta:jobId}}.json",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let missing_context = block_on(execute_detail_test(
        &missing_context_plan,
        &posting_occurrence("https://example.test/jobs/42", []),
        &FakeDetailFetcher::new([]),
    ));
    assert_runtime_diagnostic(
        &missing_context.diagnostics[0],
        "runtime_template_context_missing",
    );
}

#[test]
fn detail_accepts_the_shared_runtime_cancellation_context() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = FakeDetailFetcher::default();
    let browser = FakeBrowser::new([]);
    let cancellation = AlwaysCancelled;

    let result = block_on(execute_detail(
        &plan,
        &posting,
        &fetcher,
        &browser,
        RuntimeExecutionContext::with_cancellation(&cancellation),
    ));

    assert_eq!(result.description_text, None);
    assert!(fetcher.requests().is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "runtime_execution_cancelled");
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

#[test]
fn detail_cancellation_interrupts_an_active_http_fetch() {
    block_on(async {
        let plan = compiled_json_detail_plan(
            "{{posting:url}}",
            json!({
                "type": "json_path",
                "jsonPath": "$.description",
                "cardinality": "one"
            }),
            None,
            None,
        );
        let posting = posting_occurrence("https://example.test/jobs/42.json", []);
        let fetcher = HangingDetailFetcher::default();
        let browser = FakeBrowser::new([]);
        let cancellation = TestCancellation::default();

        let cancel = async {
            fetcher.started.notified().await;
            cancellation.cancel();
        };
        let execute = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            execute_detail(
                &plan,
                &posting,
                &fetcher,
                &browser,
                RuntimeExecutionContext::with_cancellation(&cancellation),
            ),
        );
        let (_, result) = tokio::join!(cancel, execute);
        let result = result.expect("cancellation should interrupt the active Detail fetch");

        assert_eq!(result.description_text, None);
        assert_eq!(fetcher.request_count(), 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "runtime_execution_cancelled");
    });
}

#[test]
fn detail_browser_cancellation_is_typed_control_flow() {
    block_on(async {
        let plan = compiled_browser_detail_plan(
            "{{posting:url}}",
            json!({ "type": "html" }),
            json!({ "type": "css", "selector": "main.job" }),
            json!({ "type": "css_text", "selector": ".description", "cardinality": "one" }),
        );
        let posting = posting_occurrence("https://example.test/jobs/42.html", []);
        let fetcher = FakeDetailFetcher::default();
        let browser = CancellationAwareDetailBrowser::default();
        let cancellation = TestCancellation::default();

        let cancel = async {
            browser.started.notified().await;
            cancellation.cancel();
        };
        let execute = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            execute_detail(
                &plan,
                &posting,
                &fetcher,
                &browser,
                RuntimeExecutionContext::with_cancellation(&cancellation),
            ),
        );
        let (_, result) = tokio::join!(cancel, execute);
        let result = result.expect("cancellation should interrupt the active Detail browser");

        assert_eq!(result.description_text, None);
        assert_eq!(browser.render_count(), 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "runtime_execution_cancelled");
    });
}

#[derive(Default)]
struct TestCancellation {
    cancelled: AtomicBool,
}

impl TestCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl RuntimeCancellation for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct HangingDetailFetcher {
    started: Arc<Notify>,
    request_count: std::sync::Mutex<usize>,
}

impl HangingDetailFetcher {
    fn request_count(&self) -> usize {
        *self.request_count.lock().unwrap()
    }
}

impl DetailFetcher for HangingDetailFetcher {
    fn fetch<'a>(
        &'a self,
        _request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        Box::pin(async move {
            *self.request_count.lock().unwrap() += 1;
            self.started.notify_one();
            std::future::pending().await
        })
    }
}

#[derive(Default)]
struct CancellationAwareDetailBrowser {
    started: Arc<Notify>,
    render_count: std::sync::Mutex<usize>,
}

impl CancellationAwareDetailBrowser {
    fn render_count(&self) -> usize {
        *self.render_count.lock().unwrap()
    }
}

impl ProfileBrowserClient for CancellationAwareDetailBrowser {
    fn render<'a>(
        &'a self,
        _request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move { panic!("Detail should use cancellation-aware browser rendering") })
    }

    fn render_with_context<'a>(
        &'a self,
        _request: ProfileBrowserFetchRequest,
        context: RuntimeExecutionContext<'a>,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            *self.render_count.lock().unwrap() += 1;
            self.started.notify_one();
            context.cancelled().await;
            Err(ProfileBrowserFetchError::new(
                ProfileBrowserFetchErrorKind::Cancelled,
                "detail cancelled",
            ))
        })
    }
}

struct AlwaysCancelled;

impl RuntimeCancellation for AlwaysCancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[derive(Default)]
struct FakeDetailFetcher {
    responses: BTreeMap<String, String>,
    requests: std::sync::Mutex<Vec<DetailFetchRequest>>,
}

impl FakeDetailFetcher {
    fn new(responses: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body))
                .collect(),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<DetailFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl DetailFetcher for FakeDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                DetailFetchError::new(format!("missing fake response for {}", request.url))
            })?;
            Ok(DetailFetchResponse { body })
        })
    }
}

struct FakeBrowser {
    responses: BTreeMap<String, String>,
    failure: Option<ProfileBrowserFetchError>,
    requests: std::sync::Mutex<Vec<ProfileBrowserFetchRequest>>,
}

impl FakeBrowser {
    fn new(responses: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body))
                .collect(),
            failure: None,
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn failing(error: ProfileBrowserFetchError) -> Self {
        Self {
            responses: BTreeMap::new(),
            failure: Some(error),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<ProfileBrowserFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ProfileBrowserClient for FakeBrowser {
    fn render<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            if let Some(error) = &self.failure {
                return Err(error.clone());
            }
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                ProfileBrowserFetchError::new(
                    ProfileBrowserFetchErrorKind::NavigationFailed,
                    format!("missing fake browser response for {}", request.url),
                )
            })?;
            Ok(ProfileBrowserFetchResponse { body })
        })
    }
}

fn compiled_json_detail_plan(
    fetch_url: &str,
    description_text: Value,
    captures: Option<Value>,
    min_description_length: Option<u64>,
) -> SourceExecutionPlan {
    compiled_detail_plan(
        fetch_url,
        json!({ "type": "json" }),
        json!({ "type": "document" }),
        description_text,
        captures,
        min_description_length,
    )
}

fn compiled_browser_detail_plan(
    fetch_url: &str,
    parse: Value,
    select: Value,
    description_text: Value,
) -> SourceExecutionPlan {
    compiled_detail_plan_with_fetch(
        json!({
            "mode": "browser",
            "url": fetch_url,
            "timeoutMs": 30000,
            "waits": [{
                "type": "selector",
                "selector": "main.job",
                "timeoutMs": 5000
            }],
            "interactions": [{
                "type": "click_until_gone",
                "selector": "button.cookie-banner",
                "maxCount": 2,
                "waitAfterMs": 100
            }]
        }),
        parse,
        select,
        description_text,
        None,
        None,
    )
}

fn compiled_detail_plan(
    fetch_url: &str,
    parse: Value,
    select: Value,
    description_text: Value,
    captures: Option<Value>,
    min_description_length: Option<u64>,
) -> SourceExecutionPlan {
    compiled_detail_plan_with_fetch(
        json!({
            "mode": "http",
            "method": "GET",
            "url": fetch_url,
            "timeoutMs": 10000
        }),
        parse,
        select,
        description_text,
        captures,
        min_description_length,
    )
}

fn compiled_detail_plan_with_fetch(
    fetch: Value,
    parse: Value,
    select: Value,
    description_text: Value,
    captures: Option<Value>,
    min_description_length: Option<u64>,
) -> SourceExecutionPlan {
    let mut strategy = json!({
        "key": "detail_api",
        "fetch": fetch,
        "parse": parse,
        "select": select,
        "extract": { "fields": { "descriptionText": description_text } }
    });
    if let Some(captures) = captures {
        strategy["captures"] = captures;
    }
    if let Some(min_length) = min_description_length {
        strategy["acceptWhen"] = json!({ "minDescriptionLength": min_length });
    }

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "example_jobs",
        "name": "Example Jobs",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Runtime fixture profile."
        },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl", "apiBase"],
            "properties": {
                "feedUrl": { "type": "string" },
                "apiBase": { "type": "string" }
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
                        "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "fields": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" },
                            "postingMeta": {
                                "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "one" },
                                "tenant": { "type": "json_path", "jsonPath": "$.tenant", "cardinality": "one" }
                            }
                        }
                    }
                }]
            },
            "detail": {
                "policy": { "type": "first_accepted" },
                "strategies": [strategy]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "example_source",
        "name": "Example Source",
        "status": "active",
        "sourceConfig": {
            "feedUrl": "https://example.test/jobs.json",
            "apiBase": "https://api.example.test"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();

    let result = compile_test_source(&source, Some(profile));
    unwrap_plan(result)
}

fn compiled_detail_plan_with_strategies(
    step_accept_when: Option<Value>,
    strategies: Vec<Value>,
) -> SourceExecutionPlan {
    let mut detail = json!({
        "policy": { "type": "first_accepted" },
        "strategies": strategies
    });
    if let Some(accept_when) = step_accept_when {
        detail["acceptWhen"] = accept_when;
    }

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "fallback_detail_jobs",
        "name": "Fallback Detail Jobs",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Posting detail fallback runtime fixture."
        },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl", "apiBase"],
            "properties": {
                "feedUrl": { "type": "string" },
                "apiBase": { "type": "string" }
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
                        "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "fields": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" },
                            "postingMeta": {
                                "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "one" }
                            }
                        }
                    }
                }]
            },
            "detail": detail
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "fallback_detail_source",
        "name": "Fallback Detail Source",
        "status": "active",
        "sourceConfig": {
            "feedUrl": "https://example.test/jobs.json",
            "apiBase": "https://api.example.test"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "fallback_detail_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();

    let result = compile_test_source(&source, Some(profile));
    unwrap_plan(result)
}

fn posting_occurrence(
    url: &str,
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> DetailPostingOccurrence {
    DetailPostingOccurrence {
        url: url.to_string(),
        title: Some("Fixture title".to_string()),
        company: Some("Fixture GmbH".to_string()),
        locations: Vec::new(),
        description_text: None,
        posting_meta: posting_meta
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}

fn assert_runtime_diagnostic(diagnostic: &Diagnostic, expected_code: &str) {
    assert_eq!(diagnostic.category, DiagnosticCategory::Runtime);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, expected_code);
    assert!(
        diagnostic.strategy_key.is_some(),
        "runtime diagnostic should include the executing strategy key"
    );
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
