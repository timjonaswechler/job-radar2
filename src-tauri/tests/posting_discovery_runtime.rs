use std::{collections::BTreeMap, future::Future, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, execute_posting_discovery_with_clients,
    execute_posting_discovery_with_fetcher, DiagnosticCategory, DiagnosticSeverity,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, PostingDiscoveryFetchError,
    PostingDiscoveryFetchRequest, PostingDiscoveryFetchResponse, PostingDiscoveryFetcher,
    ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, ProfileCompilerSnapshot,
    SourceDocument, SourceExecutionPlan, SourceProfileDocument,
};
use serde_json::{json, Value};

#[test]
fn compiled_posting_discovery_runtime_returns_one_normalized_candidate() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
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

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(result.candidates[0].url, "https://example.test/jobs/1");
    assert_eq!(fetcher.requests()[0].url, "https://example.test/jobs.json");
    assert_eq!(fetcher.requests()[0].timeout_ms, 10_000);
}

#[test]
fn compiled_posting_discovery_runtime_selects_multiple_json_items() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
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

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_eq!(result.candidates[1].title, "Frontend Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_reports_required_field_and_cardinality_diagnostics() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
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

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "required_field_missing");
    assert_runtime_diagnostic(&result.diagnostics[1], "field_cardinality_mismatch");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/extract/fields/title"
    );
    assert_eq!(
        result.diagnostics[1].path,
        "/postingDiscovery/strategies/0/extract/fields/company"
    );
}

#[test]
fn compiled_posting_discovery_runtime_preserves_successful_items_with_partial_diagnostics() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
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

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Rust Engineer");
    assert_runtime_diagnostic(&result.diagnostics[0], "required_field_missing");
    assert_eq!(
        result.diagnostics[0].details.as_ref().unwrap()["itemIndex"],
        1
    );
}

#[test]
fn compiled_posting_discovery_runtime_applies_explicit_whitespace_transforms() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.title",
        "cardinality": "one",
        "transforms": [{ "type": "trim" }, { "type": "normalize_whitespace" }]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "\n\tStaff    Platform\nEngineer\t",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Staff Platform Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_applies_url_decode_and_slug_to_title_transforms_in_order() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.titleSlug",
        "cardinality": "one",
        "transforms": [{ "type": "url_decode" }, { "type": "slug_to_title" }]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "titleSlug": "senior%20rust-engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_dedupes_string_arrays_before_cardinality() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "json_path",
        "jsonPath": "$.titles",
        "cardinality": "one",
        "transforms": [{ "type": "dedupe" }]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "titles": ["Rust Engineer", "Rust Engineer"],
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Rust Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_combines_parts_in_declared_order() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " / ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "Senior",
                "role": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Senior / Rust Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_fails_combine_when_required_part_is_missing() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "Senior",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "required_combine_part_missing");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/extract/fields/title/parts/1/value"
    );
}

#[test]
fn compiled_posting_discovery_runtime_allows_missing_optional_combine_part() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" }, "optional": true },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "role": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Rust Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_preserves_empty_combine_join() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": "",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.prefix", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.suffix", "cardinality": "one" } }
        ]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "prefix": "Data",
                "suffix": "Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "DataEngineer");
}

#[test]
fn compiled_posting_discovery_runtime_applies_final_transforms_after_combine() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": "-",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ],
        "transforms": [{ "type": "slug_to_title" }]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "level": "senior",
                "role": "rust-engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
}

#[test]
fn source_owned_posting_discovery_runtime_uses_same_combine_behavior() {
    let mut fields = default_fields();
    fields["title"] = json!({
        "type": "combine",
        "join": " ",
        "parts": [
            { "value": { "type": "json_path", "jsonPath": "$.level", "cardinality": "one" } },
            { "value": { "type": "json_path", "jsonPath": "$.role", "cardinality": "one" } }
        ]
    });
    let plan = source_owned_json_posting_discovery_plan(fields);
    let fetcher = FakeFetcher::new([(
        "https://example.test/source-owned.json",
        json!({
            "jobs": [{
                "level": "Staff",
                "role": "Platform Engineer",
                "company": "Owned Example GmbH",
                "url": "https://example.test/jobs/owned-1"
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates[0].title, "Staff Platform Engineer");
}

#[test]
fn compiled_posting_discovery_runtime_normalizes_single_location_expression_without_implicit_splitting(
) {
    let mut fields = default_fields();
    fields["locations"] = json!({
        "type": "json_path",
        "jsonPath": "$.locations",
        "cardinality": "all"
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "locations": ["  Berlin  ", "", "Berlin", "Remote, München", " Remote, München "]
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].locations,
        vec!["Berlin", "Remote, München"]
    );
}

#[test]
fn compiled_posting_discovery_runtime_normalizes_list_style_locations_in_order() {
    let mut fields = default_fields();
    fields["locations"] = json!([
        { "type": "json_path", "jsonPath": "$.primaryLocation", "cardinality": "one" },
        { "type": "json_path", "jsonPath": "$.otherLocations", "cardinality": "all" }
    ]);
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "primaryLocation": " Remote ",
                "otherLocations": ["Berlin", "Remote", " München ", ""]
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].locations,
        vec!["Remote", "Berlin", "München"]
    );
}

#[test]
fn compiled_posting_discovery_runtime_splits_and_dedupes_location_arrays_in_order() {
    let mut fields = default_fields();
    fields["locations"] = json!({
        "type": "json_path",
        "jsonPath": "$.locationsText",
        "cardinality": "one",
        "transforms": [
            { "type": "split", "separator": ";" },
            { "type": "trim" },
            { "type": "dedupe" }
        ]
    });
    let plan = compiled_json_posting_discovery_plan(fields, default_select());
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [{
                "title": "Rust Engineer",
                "company": "Example GmbH",
                "url": "https://example.test/jobs/1",
                "locationsText": " Berlin ;Remote; Berlin; München "
            }]
        })
        .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.candidates[0].locations,
        vec!["Berlin", "Remote", "München"]
    );
}

#[test]
fn compiled_posting_discovery_rejects_template_transform_pipes() {
    let mut fields = default_fields();
    fields["company"] = json!({
        "type": "template",
        "template": "{{sourceConfig:feedUrl|slugToTitle}}",
        "cardinality": "one"
    });

    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "example_jobs",
        "name": "Example Jobs",
        "kind": "generic",
        "support": { "level": "experimental" },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "json_feed",
            "name": "JSON feed",
            "postingDiscovery": {
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
                    "extract": { "fields": fields }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
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

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "example_source",
    );

    assert!(result.execution_plan.is_none());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "template_transform_pipes_unsupported"
            && diagnostic.category == DiagnosticCategory::Compiler
            && diagnostic.path
                == "/accessPaths/0/postingDiscovery/strategies/0/extract/fields/company/template"
    }));
}

#[test]
fn compiled_posting_discovery_runtime_extracts_xml_posting_fields() {
    let fields = json!({
        "title": { "type": "xml_text", "textPath": "title", "cardinality": "one" },
        "company": { "type": "xml_text", "textPath": "company", "cardinality": "one" },
        "url": { "type": "xml_text", "textPath": "url", "cardinality": "one" },
        "locations": { "type": "xml_element", "element": "location", "cardinality": "all" },
        "postingMeta": {
            "jobId": { "type": "xml_text", "textPath": "id", "cardinality": "one" }
        }
    });
    let plan = compiled_posting_discovery_plan(
        json!({ "type": "xml" }),
        json!({ "type": "xml_element", "element": "job" }),
        fields,
        "https://example.test/jobs.xml",
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.xml",
        r#"<jobs>
            <job>
              <id> 42 </id>
              <title> Senior   Rust
Engineer </title>
              <company> Example GmbH </company>
              <url> https://example.test/jobs/42 </url>
              <locations><location> Berlin </location><location>Berlin</location><location> Remote </location></locations>
            </job>
        </jobs>"#
            .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Senior Rust Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(result.candidates[0].url, "https://example.test/jobs/42");
    assert_eq!(result.candidates[0].locations, vec!["Berlin", "Remote"]);
    assert_eq!(result.candidates[0].posting_meta["jobId"], "42");
}

#[test]
fn compiled_posting_discovery_runtime_extracts_html_posting_fields_with_css() {
    let fields = json!({
        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
        "company": { "type": "css_text", "selector": ".company", "cardinality": "one" },
        "url": { "type": "css_attribute", "selector": "a.apply", "attribute": "href", "cardinality": "one" },
        "locations": { "type": "css_text", "selector": ".location", "cardinality": "all" }
    });
    let plan = compiled_posting_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        fields,
        "https://example.test/jobs.html",
    );
    let fetcher = FakeFetcher::new([(
        "https://example.test/jobs.html",
        r#"<html><body>
            <article class="posting">
              <h2 class="title"> Staff   Frontend
Engineer </h2>
              <span class="company"> Example GmbH </span>
              <a class="apply" href="https://example.test/jobs/frontend">Apply</a>
              <span class="location"> Berlin </span><span class="location">Remote</span>
            </article>
        </body></html>"#
            .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_fetcher(&plan, &fetcher));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Staff Frontend Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(
        result.candidates[0].url,
        "https://example.test/jobs/frontend"
    );
    assert_eq!(result.candidates[0].locations, vec!["Berlin", "Remote"]);
}

#[test]
fn compiled_posting_discovery_runtime_uses_browser_fetch_rendered_html() {
    let fields = json!({
        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
        "company": { "type": "css_text", "selector": ".company", "cardinality": "one" },
        "url": { "type": "css_attribute", "selector": "a.apply", "attribute": "href", "cardinality": "one" }
    });
    let plan = compiled_browser_posting_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        fields,
        "https://example.test/rendered?tenant=acme",
    );
    let fetcher = FakeFetcher::new(std::iter::empty());
    let browser = FakeBrowser::new([(
        "https://example.test/rendered?tenant=acme",
        r#"<html><body>
            <article class="posting">
              <h2 class="title"> Browser Rendered Engineer </h2>
              <span class="company"> Example GmbH </span>
              <a class="apply" href="https://example.test/jobs/browser">Apply</a>
            </article>
        </body></html>"#
            .to_string(),
    )]);

    let result = block_on(execute_posting_discovery_with_clients(
        &plan, &fetcher, &browser,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].title, "Browser Rendered Engineer");
    assert_eq!(result.candidates[0].company, "Example GmbH");
    assert_eq!(
        result.candidates[0].url,
        "https://example.test/jobs/browser"
    );
    assert!(fetcher.requests().is_empty());
    let browser_requests = browser.requests();
    assert_eq!(browser_requests.len(), 1);
    assert_eq!(
        browser_requests[0].url,
        "https://example.test/rendered?tenant=acme"
    );
    assert_eq!(browser_requests[0].timeout_ms, 30_000);
    assert_eq!(
        browser_requests[0].waits,
        vec![
            ExecutionPlanBrowserWait::Selector {
                selector: Some("article.posting".to_string()),
                timeout_ms: 5000,
            },
            ExecutionPlanBrowserWait::NetworkIdle {
                selector: None,
                timeout_ms: 250,
            },
        ]
    );
    assert_eq!(
        browser_requests[0].interactions,
        vec![ExecutionPlanBrowserInteraction::ClickIfVisible {
            selector: "button.load-more".to_string(),
            max_count: 2,
            wait_after_ms: Some(250),
        }]
    );
}

#[test]
fn compiled_posting_discovery_runtime_reports_browser_fetch_diagnostics() {
    let plan = compiled_browser_posting_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article.posting" }),
        default_html_fields(),
        "https://example.test/rendered",
    );
    let fetcher = FakeFetcher::new(std::iter::empty());
    let browser = FakeBrowser::failing(ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::WaitTimeout {
            wait_index: Some(0),
        },
        "selector .posting did not appear",
    ));

    let result = block_on(execute_posting_discovery_with_clients(
        &plan, &fetcher, &browser,
    ));

    assert!(result.candidates.is_empty());
    assert_runtime_diagnostic(&result.diagnostics[0], "browser_wait_timeout");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDiscovery/strategies/0/fetch/waits/0"
    );
}

#[test]
fn compiled_posting_discovery_runtime_reports_xml_and_html_diagnostics() {
    let xml_plan = compiled_posting_discovery_plan(
        json!({ "type": "xml" }),
        json!({ "type": "xml_element", "element": "job" }),
        default_xml_fields(),
        "https://example.test/jobs.xml",
    );
    let xml_parse_failure = block_on(execute_posting_discovery_with_fetcher(
        &xml_plan,
        &FakeFetcher::new([("https://example.test/jobs.xml", "<jobs><job>".to_string())]),
    ));
    assert_runtime_diagnostic(&xml_parse_failure.diagnostics[0], "xml_parse_failed");
    assert_eq!(
        xml_parse_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/parse"
    );

    let html_select_plan = compiled_posting_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "[" }),
        default_html_fields(),
        "https://example.test/jobs.html",
    );
    let html_select_failure = block_on(execute_posting_discovery_with_fetcher(
        &html_select_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.html",
            "<article></article>".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&html_select_failure.diagnostics[0], "css_select_failed");
    assert_eq!(
        html_select_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/select/selector"
    );

    let mut html_fields = default_html_fields();
    html_fields["title"] = json!({ "type": "css_text", "selector": "[", "cardinality": "one" });
    let html_extract_plan = compiled_posting_discovery_plan(
        json!({ "type": "html" }),
        json!({ "type": "css", "selector": "article" }),
        html_fields,
        "https://example.test/jobs.html",
    );
    let html_extract_failure = block_on(execute_posting_discovery_with_fetcher(
        &html_extract_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.html",
            "<article><a class='apply' href='https://example.test/jobs/1'></a><span class='company'>Example GmbH</span></article>".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(
        &html_extract_failure.diagnostics[0],
        "field_css_selector_failed",
    );
    assert_eq!(
        html_extract_failure.diagnostics[0].path,
        "/postingDiscovery/strategies/0/extract/fields/title"
    );
}

#[test]
fn compiled_posting_discovery_runtime_reports_fetch_parse_select_and_extract_failures() {
    let plan = compiled_json_posting_discovery_plan(default_fields(), default_select());
    let fetch_failure = block_on(execute_posting_discovery_with_fetcher(
        &plan,
        &FakeFetcher::new([]),
    ));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_posting_discovery_with_fetcher(
        &plan,
        &FakeFetcher::new([("https://example.test/jobs.json", "{not-json".to_string())]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let select_plan = compiled_json_posting_discovery_plan(
        default_fields(),
        json!({ "type": "json_path", "jsonPath": "$.jobs[*]" }),
    );
    let select_failure = block_on(execute_posting_discovery_with_fetcher(
        &select_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.json",
            json!({ "jobs": [] }).to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&select_failure.diagnostics[0], "json_path_select_failed");

    let mut fields = default_fields();
    fields["title"] =
        json!({ "type": "json_path", "jsonPath": "$.title[*]", "cardinality": "one" });
    let extract_plan = compiled_json_posting_discovery_plan(fields, default_select());
    let extract_failure = block_on(execute_posting_discovery_with_fetcher(
        &extract_plan,
        &FakeFetcher::new([(
            "https://example.test/jobs.json",
            json!({
                "jobs": [{
                    "title": "Rust Engineer",
                    "company": "Example GmbH",
                    "url": "https://example.test/jobs/1"
                }]
            })
            .to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&extract_failure.diagnostics[0], "field_json_path_failed");
    assert!(extract_failure.candidates.is_empty());
}

#[derive(Default)]
struct FakeFetcher {
    responses: BTreeMap<String, String>,
    requests: std::sync::Mutex<Vec<PostingDiscoveryFetchRequest>>,
}

impl FakeFetcher {
    fn new(responses: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body))
                .collect(),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<PostingDiscoveryFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl PostingDiscoveryFetcher for FakeFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                PostingDiscoveryFetchError::new(format!(
                    "missing fake response for {}",
                    request.url
                ))
            })?;
            Ok(PostingDiscoveryFetchResponse { body })
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

fn compiled_json_posting_discovery_plan(fields: Value, select: Value) -> SourceExecutionPlan {
    compiled_posting_discovery_plan(
        json!({ "type": "json" }),
        select,
        fields,
        "https://example.test/jobs.json",
    )
}

fn compiled_posting_discovery_plan(
    parse: Value,
    select: Value,
    fields: Value,
    feed_url: &'static str,
) -> SourceExecutionPlan {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "example_jobs",
        "name": "Example Jobs",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Runtime fixture profile."
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
            "postingDiscovery": {
                "strategies": [{
                    "key": "json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": parse,
                    "select": select,
                    "extract": { "fields": fields }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "example_source",
        "name": "Example Source",
        "status": "active",
        "sourceConfig": { "feedUrl": feed_url },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "example_jobs",
            "pathKey": "json_feed"
        }
    }))
    .unwrap();

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "example_source",
    );
    assert_eq!(result.diagnostics, Vec::new());
    result.execution_plan.expect("fixture plan should compile")
}

fn compiled_browser_posting_discovery_plan(
    parse: Value,
    select: Value,
    fields: Value,
    page_url: &'static str,
) -> SourceExecutionPlan {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "browser_jobs",
        "name": "Browser Jobs",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Browser runtime fixture profile."
        },
        "sourceConfigSchema": {
            "type": "object",
            "required": ["pageUrl"],
            "properties": { "pageUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "browser_page",
            "name": "Browser page",
            "postingDiscovery": {
                "strategies": [{
                    "key": "browser_html",
                    "fetch": {
                        "mode": "browser",
                        "url": "{{sourceConfig:pageUrl}}",
                        "timeoutMs": 30000,
                        "waits": [
                            {
                                "type": "selector",
                                "selector": "article.posting",
                                "timeoutMs": 5000
                            },
                            {
                                "type": "network_idle",
                                "timeoutMs": 250
                            }
                        ],
                        "interactions": [{
                            "type": "click_if_visible",
                            "selector": "button.load-more",
                            "maxCount": 2,
                            "waitAfterMs": 250
                        }]
                    },
                    "parse": parse,
                    "select": select,
                    "extract": { "fields": fields }
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "browser_source",
        "name": "Browser Source",
        "status": "active",
        "sourceConfig": { "pageUrl": page_url },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "browser_jobs",
            "pathKey": "browser_page"
        }
    }))
    .unwrap();

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: vec![profile],
            sources: vec![source],
        },
        "browser_source",
    );
    assert_eq!(result.diagnostics, Vec::new());
    result
        .execution_plan
        .expect("browser fixture plan should compile")
}

fn source_owned_json_posting_discovery_plan(fields: Value) -> SourceExecutionPlan {
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
        "key": "owned_source",
        "name": "Owned Source",
        "status": "active",
        "sourceConfig": { "feedUrl": "https://example.test/source-owned.json" },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Source-owned runtime fixture."
        },
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "owned_json_feed",
            "name": "Owned JSON feed",
            "sourceConfigSchema": {
                "type": "object",
                "required": ["feedUrl"],
                "properties": { "feedUrl": { "type": "string" } },
                "additionalProperties": false
            },
            "postingDiscovery": {
                "strategies": [{
                    "key": "owned_json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "{{sourceConfig:feedUrl}}",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": { "fields": fields }
                }]
            }
        }
    }))
    .unwrap();

    let result = compile_source_execution_plan(
        &ProfileCompilerSnapshot {
            profiles: Vec::new(),
            sources: vec![source],
        },
        "owned_source",
    );
    assert_eq!(result.diagnostics, Vec::new());
    result
        .execution_plan
        .expect("source-owned fixture plan should compile")
}

fn default_select() -> Value {
    json!({ "type": "json_path", "jsonPath": "$.jobs" })
}

fn default_fields() -> Value {
    json!({
        "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
        "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
        "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" }
    })
}

fn default_xml_fields() -> Value {
    json!({
        "title": { "type": "xml_text", "textPath": "title", "cardinality": "one" },
        "company": { "type": "xml_text", "textPath": "company", "cardinality": "one" },
        "url": { "type": "xml_text", "textPath": "url", "cardinality": "one" }
    })
}

fn default_html_fields() -> Value {
    json!({
        "title": { "type": "css_text", "selector": ".title", "cardinality": "one" },
        "company": { "type": "css_text", "selector": ".company", "cardinality": "one" },
        "url": { "type": "css_attribute", "selector": "a.apply", "attribute": "href", "cardinality": "one" }
    })
}

fn assert_runtime_diagnostic(diagnostic: &job_radar_lib::Diagnostic, expected_code: &str) {
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
