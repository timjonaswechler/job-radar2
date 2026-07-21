mod support;

use support::{
    compile_test_source, execute_detail_test, execute_detail_test_with_config, unwrap_plan,
};

fn empty_source_config() -> &'static serde_json::Map<String, serde_json::Value> {
    static EMPTY: std::sync::OnceLock<serde_json::Map<String, serde_json::Value>> =
        std::sync::OnceLock::new();
    EMPTY.get_or_init(serde_json::Map::new)
}
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
    execute_detail, DetailField, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, HttpMethod, PhaseCompletion,
    PostingOccurrence, ProfileBrowserClient, ProfileBrowserFetchError,
    ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    RequestedDetailFields, RuntimeCancellation, RuntimeExecutionContext, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument, UnavailableProfileBrowserClient,
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.json",
        json!({ "description": "First paragraph.\n\nSecond paragraph." }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
    assert_eq!(
        fetcher.requests()[0].url,
        "https://example.test/jobs/42.json"
    );
    assert_eq!(fetcher.requests()[0].timeout_ms, 10_000);
}

#[test]
fn compiler_materializes_omitted_cardinality_as_typed_one_plan() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description"
        }),
        None,
        None,
    );

    let serialized = serde_json::to_value(plan).unwrap();
    assert_eq!(
        serialized.pointer("/detail/strategies/0/extract/fields/descriptionText/cardinality"),
        Some(&json!("one"))
    );
}

#[test]
fn compiled_detail_runtime_projects_canonical_cardinality_failures() {
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.json",
        json!({ "description": ["first", "second"] }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.patch.description_text, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "field_cardinality_mismatch")
        .expect("canonical cardinality failure must be projected by Detail");
    assert_eq!(diagnostic.category, DiagnosticCategory::Runtime);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.strategy_key.as_deref(), Some("detail_api"));
    assert_eq!(
        diagnostic.details,
        Some(json!({
            "expectedCardinality": "one",
            "actualCount": 2,
        }))
    );
}

#[test]
fn compiled_detail_runtime_extracts_only_requested_available_fields() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "detail_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs/fields.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "title": { "type": "json_path", "jsonPath": "$.title" },
                    "company": { "type": "json_path", "jsonPath": "$.missingCompany" },
                    "locations": { "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" },
                    "descriptionText": { "type": "json_path", "jsonPath": "$.missingDescription" }
                }
            }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42", []);
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/fields.json",
        json!({
            "title": "Engineer",
            "locations": ["Berlin", "Remote"]
        })
        .to_string(),
    )]);

    let result = block_on(execute_detail(
        &plan,
        &Default::default(),
        &posting,
        RequestedDetailFields::new([DetailField::Title, DetailField::Locations]).unwrap(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.patch.title.as_deref(), Some("Engineer"));
    assert_eq!(
        result.patch.locations,
        Some(vec!["Berlin".to_string(), "Remote".to_string()])
    );
    assert_eq!(result.patch.company, None);
    assert_eq!(result.patch.description_text, None);
    assert!(result.rejections.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn detail_acceptance_rejects_unrequested_fields_before_io() {
    let plan = compiled_detail_plan_with_strategies(
        None,
        vec![json!({
            "key": "detail_api",
            "fetch": {
                "mode": "http",
                "method": "GET",
                "url": "https://example.test/jobs/fields.json",
                "timeoutMs": 10000
            },
            "parse": { "type": "json" },
            "select": { "type": "document" },
            "extract": {
                "fields": {
                    "title": { "type": "json_path", "jsonPath": "$.title" },
                    "locations": { "type": "json_path", "jsonPath": "$.locations", "cardinality": "all" }
                }
            },
            "acceptWhen": { "requiredFields": ["title"] }
        })],
    );
    let posting = posting_occurrence("https://example.test/jobs/42", []);
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/fields.json",
        json!({ "title": "Engineer", "locations": ["Berlin"] }).to_string(),
    )]);

    let result = block_on(execute_detail(
        &plan,
        &Default::default(),
        &posting,
        RequestedDetailFields::new([DetailField::Locations]).unwrap(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert!(result.patch.is_empty());
    assert_eq!(result.diagnostics[0].code, "acceptance_field_not_requested");
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/strategies/0/acceptWhen/requiredFields"
    );
    assert_eq!(
        result.diagnostics[0].strategy_key.as_deref(),
        Some("detail_api")
    );
    assert!(fetcher.requests().is_empty());
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
    let fetcher = fake_profile_http_client([
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
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([
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
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/short.json",
        json!({ "description": "Too short" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.patch.description_text, None);
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
fn compiled_detail_runtime_renders_fetch_templates_from_pre_fetch_contexts() {
    let plan = compiled_json_detail_plan(
        "{{sourceConfig:apiBase}}/{{postingMeta:tenant}}/{{postingMeta:jobId}}?u={{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        Some(json!({
            "tenant": {
                "from": { "type": "posting_meta", "key": "tenant", "cardinality": "one" },
                "pattern": "^(?<tenant>[a-z0-9_]+)$"
            }
        })),
        None,
    );
    let posting = posting_occurrence(
        "https://example.test/jobs/job-42",
        [("jobId", "REQ-42"), ("tenant", "acme_jobs")],
    );
    let expected_url =
        "https://api.example.test/acme_jobs/REQ-42?u=https://example.test/jobs/job-42";
    let fetcher = fake_profile_http_client([(
        expected_url,
        json!({ "description": "Rendered from all template contexts." }).to_string(),
    )]);

    let source_config =
        serde_json::from_value(json!({ "apiBase": "https://api.example.test" })).unwrap();
    let result = block_on(execute_detail_test_with_config(
        &plan,
        &source_config,
        &posting,
        &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([(
        "https://api.example.test/detail",
        json!({ "description": "Detail POST response." }).to_string(),
    )]);

    let source_config =
        serde_json::from_value(json!({ "apiBase": "https://api.example.test" })).unwrap();
    let result = block_on(execute_detail_test_with_config(
        &plan,
        &source_config,
        &posting,
        &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
        Some("Detail POST response.".to_string())
    );
    let request = &fetcher.requests()[0];
    assert_eq!(request.method, HttpMethod::Post);
    assert_eq!(request.url, "https://api.example.test/detail");
    assert_eq!(request.timeout_ms, 15_000);
    assert_eq!(
        request.headers,
        vec![("content-type".to_string(), b"application/json".to_vec())]
    );
    let body = request.body.as_ref().expect("rendered JSON body");
    assert_eq!(
        body.bytes(),
        br#"{"jobId":"REQ-42","postingUrl":"https://example.test/jobs/42","source":"Example Source","tenant":"acme_jobs"}"#
    );
    assert_eq!(body.default_content_type(), Some("application/json"));
}

#[test]
fn compiled_detail_runtime_normalizes_html_in_json_description_text() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.descriptionHtml",
            "cardinality": "one",
            "transforms": [{ "type": "to_string" }, { "type": "html_to_text" }, { "type": "normalize_whitespace" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.json",
        json!({ "descriptionHtml": "<p>First paragraph.</p><p>Second <strong>paragraph</strong>.</p>" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
            "transforms": [{ "type": "to_string" }, { "type": "url_decode" }, { "type": "slug_to_title" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.json",
        json!({ "descriptionSlug": "senior%20rust-engineer" }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
        Some("Senior Rust Engineer".to_string())
    );
}

#[test]
fn compiled_detail_runtime_to_string_rejects_json_null_without_partial_output() {
    let plan = compiled_json_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one",
            "transforms": [{ "type": "to_string" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.json",
        json!({ "description": null }).to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.patch.description_text, None);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "transform_type_mismatch")
        .expect("typed transform failure");
    assert_eq!(
        diagnostic.details,
        Some(json!({ "transformIndex": 0, "valueIndex": 0 }))
    );
    assert!(!diagnostic.message.contains("null"));
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
            "transforms": [{ "type": "to_string" }, { "type": "normalize_whitespace" }]
        }),
        None,
        None,
    );
    let posting = posting_occurrence("https://example.test/jobs/42.json", []);
    let fetcher = fake_profile_http_client([(
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
        result.patch.description_text,
        Some("About the role. Build reliable DSL runtimes.".to_string())
    );
}

#[test]
fn compiled_detail_runtime_accepts_empty_requested_contributions_and_rejects_too_short_descriptions(
) {
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
        &fake_profile_http_client([(
            "https://example.test/jobs/missing-description.json",
            json!({ "title": "Engineer" }).to_string(),
        )]),
    ));
    assert_eq!(missing_result.patch.description_text, None);
    assert!(missing_result.diagnostics.is_empty());
    assert_eq!(
        missing_result.report.unwrap().completion,
        PhaseCompletion::Accepted
    );

    let empty_result = block_on(execute_detail_test(
        &empty_plan,
        &posting_occurrence("https://example.test/jobs/empty.json", []),
        &fake_profile_http_client([(
            "https://example.test/jobs/empty.json",
            json!({ "description": " \n \t " }).to_string(),
        )]),
    ));
    assert_eq!(empty_result.patch.description_text, None);
    assert!(empty_result.diagnostics.is_empty());
    assert_eq!(
        empty_result.report.unwrap().completion,
        PhaseCompletion::Accepted
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
        &fake_profile_http_client([(
            "https://example.test/jobs/short.json",
            json!({ "description": "Too short" }).to_string(),
        )]),
    ));
    assert_eq!(too_short_result.patch.description_text, None);
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.xml",
        r#"<jobs><job><description>First paragraph.

Second paragraph.</description></job></jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
                "type": "equal",
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
    let fetcher = fake_profile_http_client([(
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
        result.patch.description_text,
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
                "type": "equal",
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/one-job.xml",
        r#"<jobs><job><id>42</id><description>Single XML detail description.</description></job></jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([(
        "https://example.test/jobs/42.html",
        r#"<main class="job"><section class="description"><p>First paragraph.</p><p>Second paragraph.</p></section></main>"#.to_string(),
    )]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([]);
    let browser = FakeBrowser::new([(
        "https://example.test/jobs/42.html?tenant=acme",
        r#"<main class="job"><section class="description">Rendered browser detail.</section></main>"#
            .to_string(),
    )]);

    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting,
        RequestedDetailFields::description_text(),
        &fetcher,
        &browser,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.patch.description_text,
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
    let fetcher = fake_profile_http_client([]);
    let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::InteractionFailed {
            interaction_index: Some(0),
        },
        "click_until_gone reached maxCount",
    ));

    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting_occurrence("https://example.test/jobs/42.html", []),
        RequestedDetailFields::description_text(),
        &fetcher,
        &browser,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.patch.description_text, None);
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
        &fake_profile_http_client([]),
    ));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_detail_test(
        &plan,
        &posting_occurrence("https://example.test/jobs/bad-json.json", []),
        &fake_profile_http_client([(
            "https://example.test/jobs/bad-json.json",
            "{not-json".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    const AUTHORED_URL_SECRET: &str = "raw-authored-detail-secret";
    let missing_context_plan = compiled_json_detail_plan(
        "https://raw-authored-detail-secret.example.test/{{postingMeta:jobId}}.json",
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
        &fake_profile_http_client([]),
    ));
    assert_runtime_diagnostic(&missing_context.diagnostics[0], "fetch_url_template_failed");
    let diagnostic = &missing_context.diagnostics[0];
    let serialized = serde_json::to_string(diagnostic).unwrap();
    assert!(!serialized.contains(AUTHORED_URL_SECRET));
    assert_eq!(diagnostic.details, Some(json!({})));
}

#[test]
fn non_success_http_status_exposes_no_detail_patch_or_parse_result() {
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
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 500,
        final_url: "https://example.test/jobs/failed.json".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(
            json!({ "description": "Must not escape" })
                .to_string()
                .into_bytes(),
        )],
        content_length: None,
    }]);

    let result = block_on(execute_detail_test(
        &plan,
        &posting_occurrence("https://example.test/jobs/failed.json", []),
        &fetcher,
    ));

    assert_eq!(result.patch.description_text, None);
    assert_eq!(fetcher.request_count(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "http_fetch_non_success_status");
    assert_eq!(
        result.diagnostics[0].details,
        Some(json!({ "method": "GET", "status": 500 }))
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.code.ends_with("_parse_failed")));
    let report = result.report.expect("work-started terminal has a report");
    assert_eq!(report.completion, PhaseCompletion::PolicyUnsatisfied);
    assert_eq!(report.usage.requests, 1);
}

#[test]
fn detail_strict_decode_terminal_exposes_no_document_or_parse_diagnostic() {
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
    let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
        status: 200,
        final_url: "https://example.test/jobs/bad-text.json".to_string(),
        headers: Vec::new(),
        body: vec![ScriptedHttpBodyEvent::Chunk(vec![0xff])],
        content_length: None,
    }]);

    let result = block_on(execute_detail_test(
        &plan,
        &posting_occurrence("https://example.test/jobs/bad-text.json", []),
        &fetcher,
    ));

    assert_eq!(result.patch.description_text, None);
    assert_eq!(fetcher.requests().len(), 1);
    assert_runtime_diagnostic(&result.diagnostics[0], "fetch_failed");
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.code.ends_with("_parse_failed")));
}

#[test]
fn detail_capture_failure_is_atomic_and_prevents_fetch() {
    let plan = compiled_json_detail_plan(
        "https://example.test/detail",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        Some(json!({
            "tenant": {
                "from": { "type": "posting_meta", "key": "tenant" },
                "pattern": "^(?<tenant>.+)$"
            },
            "optional": {
                "from": { "type": "posting_meta", "key": "jobId" },
                "pattern": "^REQ-[0-9]+(?<optional>X)?$"
            }
        })),
        None,
    );
    let posting = posting_occurrence(
        "https://example.test/jobs/job-42",
        [("tenant", "acme"), ("jobId", "REQ-42")],
    );
    let fetcher = fake_profile_http_client([]);

    let result = block_on(execute_detail_test(&plan, &posting, &fetcher));

    assert_eq!(result.patch.description_text, None);
    assert!(fetcher.requests().is_empty());
    assert_eq!(result.diagnostics[0].code, "capture_named_group_unmatched");
    assert_eq!(
        result.diagnostics[0].path,
        "/detail/strategies/0/captures/optional"
    );
    assert_eq!(result.diagnostics[1].code, "fallback_exhausted");
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
    let fetcher = fake_profile_http_client([]);
    let browser = FakeBrowser::new([]);
    let cancellation = AlwaysCancelled;

    let result = block_on(execute_detail(
        &plan,
        empty_source_config(),
        &posting,
        RequestedDetailFields::description_text(),
        &fetcher,
        &browser,
        RuntimeExecutionContext::with_cancellation(&cancellation),
    ));

    assert_eq!(result.patch.description_text, None);
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
        let fetcher = ScriptedProfileHttpClient::new([ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs/42.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Gate("active-fetch".to_string())],
            content_length: None,
        }]);
        let browser = FakeBrowser::new([]);
        let cancellation = TestCancellation::default();

        let cancel = async {
            while !fetcher.gate_is_waiting("active-fetch") {
                tokio::task::yield_now().await;
            }
            cancellation.cancel();
        };
        let execute = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            execute_detail(
                &plan,
                empty_source_config(),
                &posting,
                RequestedDetailFields::description_text(),
                &fetcher,
                &browser,
                RuntimeExecutionContext::with_cancellation(&cancellation),
            ),
        );
        let (_, result) = tokio::join!(cancel, execute);
        let result = result.expect("cancellation should interrupt the active Detail fetch");

        assert_eq!(result.patch.description_text, None);
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
        let fetcher = fake_profile_http_client([]);
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
                empty_source_config(),
                &posting,
                RequestedDetailFields::description_text(),
                &fetcher,
                &browser,
                RuntimeExecutionContext::with_cancellation(&cancellation),
            ),
        );
        let (_, result) = tokio::join!(cancel, execute);
        let result = result.expect("cancellation should interrupt the active Detail browser");

        assert_eq!(result.patch.description_text, None);
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

fn fake_profile_http_client(
    responses: impl IntoIterator<Item = (&'static str, String)>,
) -> ScriptedProfileHttpClient {
    ScriptedProfileHttpClient::new(responses.into_iter().map(|(url, body)| {
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: url.to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(body.into_bytes())],
            content_length: None,
        }
    }))
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
                        "reference": {
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" }
                        },
                        "providerValues": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" }
                        },
                        "postingMeta": {
                            "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "one" },
                            "tenant": { "type": "json_path", "jsonPath": "$.tenant", "cardinality": "one" }
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
                        "reference": {
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" }
                        },
                        "providerValues": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" }
                        },
                        "postingMeta": {
                            "jobId": { "type": "json_path", "jsonPath": "$.id", "cardinality": "one" }
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
) -> PostingOccurrence {
    let (reference, identity) =
        job_radar_lib::validate_posting_reference("fixture", url, None).unwrap();
    PostingOccurrence {
        identity,
        reference,
        provider_values: job_radar_lib::ProviderValues {
            title: Some("Fixture title".to_string()),
            company: Some("Fixture GmbH".to_string()),
            locations: Vec::new(),
            description_text: None,
        },
        hints: Default::default(),
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
