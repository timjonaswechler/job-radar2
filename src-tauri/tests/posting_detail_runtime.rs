use std::{collections::BTreeMap, future::Future, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, execute_posting_detail_with_clients,
    execute_posting_detail_with_fetcher, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, PostingDetailFetchError,
    PostingDetailFetchRequest, PostingDetailFetchResponse, PostingDetailFetcher,
    PostingDetailPostingOccurrence, ProfileBrowserClient, ProfileBrowserFetchError,
    ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    ProfileCompilerSnapshot, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
};
use serde_json::{json, Value};

#[test]
fn compiled_posting_detail_runtime_extracts_direct_json_description_text() {
    let plan = compiled_json_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

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
fn compiled_posting_detail_runtime_renders_fetch_templates_from_all_runtime_contexts() {
    let plan = compiled_json_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Rendered from all template contexts.".to_string())
    );
    assert_eq!(fetcher.requests()[0].url, expected_url);
}

#[test]
fn compiled_posting_detail_runtime_normalizes_html_in_json_description_text() {
    let plan = compiled_json_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_posting_detail_runtime_applies_explicit_text_transforms() {
    let plan = compiled_json_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("Senior Rust Engineer".to_string())
    );
}

#[test]
fn compiled_posting_detail_runtime_combines_description_text_parts() {
    let plan = compiled_json_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("About the role. Build reliable DSL runtimes.".to_string())
    );
}

#[test]
fn compiled_posting_detail_runtime_reports_missing_empty_and_too_short_description_diagnostics() {
    let empty_plan = compiled_json_posting_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let missing_result = block_on(execute_posting_detail_with_fetcher(
        &empty_plan,
        &posting_occurrence("https://example.test/jobs/missing-description.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/missing-description.json",
            json!({ "title": "Engineer" }).to_string(),
        )]),
    ));
    assert_eq!(missing_result.description_text, None);
    assert_runtime_diagnostic(&missing_result.diagnostics[0], "description_empty");

    let empty_result = block_on(execute_posting_detail_with_fetcher(
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
        "/postingDetail/strategies/0/extract/fields/descriptionText"
    );

    let too_short_plan = compiled_json_posting_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        Some(20),
    );
    let too_short_result = block_on(execute_posting_detail_with_fetcher(
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
fn compiled_posting_detail_runtime_extracts_xml_description_text() {
    let plan = compiled_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_posting_detail_runtime_extracts_html_description_text_with_css() {
    let plan = compiled_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_fetcher(
        &plan, &posting, &fetcher,
    ));

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.description_text,
        Some("First paragraph. Second paragraph.".to_string())
    );
}

#[test]
fn compiled_posting_detail_runtime_uses_browser_fetch_rendered_html() {
    let plan = compiled_browser_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_clients(
        &plan, &posting, &fetcher, &browser,
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
fn compiled_posting_detail_runtime_reports_browser_interaction_diagnostics() {
    let plan = compiled_browser_posting_detail_plan(
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

    let result = block_on(execute_posting_detail_with_clients(
        &plan,
        &posting_occurrence("https://example.test/jobs/42.html", []),
        &fetcher,
        &browser,
    ));

    assert_eq!(result.description_text, None);
    assert_runtime_diagnostic(&result.diagnostics[0], "browser_interaction_failed");
    assert_eq!(
        result.diagnostics[0].path,
        "/postingDetail/strategies/0/fetch/interactions/0"
    );
}

#[test]
fn compiled_posting_detail_runtime_reports_fetch_parse_extract_and_missing_context_failures() {
    let plan = compiled_json_posting_detail_plan(
        "{{posting:url}}",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );

    let fetch_failure = block_on(execute_posting_detail_with_fetcher(
        &plan,
        &posting_occurrence("https://example.test/jobs/missing.json", []),
        &FakeDetailFetcher::new([]),
    ));
    assert_runtime_diagnostic(&fetch_failure.diagnostics[0], "fetch_failed");

    let parse_failure = block_on(execute_posting_detail_with_fetcher(
        &plan,
        &posting_occurrence("https://example.test/jobs/bad-json.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/bad-json.json",
            "{not-json".to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&parse_failure.diagnostics[0], "json_parse_failed");

    let mut extract_plan = plan.clone();
    extract_plan.posting_detail.as_mut().unwrap().strategies[0]
        .extract
        .fields
        .description_text = serde_json::from_value(json!({
        "type": "json_path",
        "jsonPath": "$.description[*]",
        "cardinality": "one"
    }))
    .unwrap();
    let extract_failure = block_on(execute_posting_detail_with_fetcher(
        &extract_plan,
        &posting_occurrence("https://example.test/jobs/42.json", []),
        &FakeDetailFetcher::new([(
            "https://example.test/jobs/42.json",
            json!({ "description": "Text" }).to_string(),
        )]),
    ));
    assert_runtime_diagnostic(&extract_failure.diagnostics[0], "field_json_path_failed");

    let missing_context_plan = compiled_json_posting_detail_plan(
        "https://example.test/{{postingMeta:jobId}}.json",
        json!({
            "type": "json_path",
            "jsonPath": "$.description",
            "cardinality": "one"
        }),
        None,
        None,
    );
    let missing_context = block_on(execute_posting_detail_with_fetcher(
        &missing_context_plan,
        &posting_occurrence("https://example.test/jobs/42", []),
        &FakeDetailFetcher::new([]),
    ));
    assert_runtime_diagnostic(
        &missing_context.diagnostics[0],
        "runtime_template_context_missing",
    );
}

#[derive(Default)]
struct FakeDetailFetcher {
    responses: BTreeMap<String, String>,
    requests: std::sync::Mutex<Vec<PostingDetailFetchRequest>>,
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

    fn requests(&self) -> Vec<PostingDetailFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl PostingDetailFetcher for FakeDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDetailFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDetailFetchResponse, PostingDetailFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                PostingDetailFetchError::new(format!("missing fake response for {}", request.url))
            })?;
            Ok(PostingDetailFetchResponse { body })
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

fn compiled_json_posting_detail_plan(
    fetch_url: &str,
    description_text: Value,
    captures: Option<Value>,
    min_description_length: Option<u64>,
) -> SourceExecutionPlan {
    compiled_posting_detail_plan(
        fetch_url,
        json!({ "type": "json" }),
        json!({ "type": "document" }),
        description_text,
        captures,
        min_description_length,
    )
}

fn compiled_browser_posting_detail_plan(
    fetch_url: &str,
    parse: Value,
    select: Value,
    description_text: Value,
) -> SourceExecutionPlan {
    compiled_posting_detail_plan_with_fetch(
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

fn compiled_posting_detail_plan(
    fetch_url: &str,
    parse: Value,
    select: Value,
    description_text: Value,
    captures: Option<Value>,
    min_description_length: Option<u64>,
) -> SourceExecutionPlan {
    compiled_posting_detail_plan_with_fetch(
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

fn compiled_posting_detail_plan_with_fetch(
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
            "postingDetail": { "strategies": [strategy] }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 2,
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

fn posting_occurrence(
    url: &str,
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> PostingDetailPostingOccurrence {
    PostingDetailPostingOccurrence {
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
