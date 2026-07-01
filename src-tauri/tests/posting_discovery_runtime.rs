use std::{collections::BTreeMap, future::Future, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, execute_posting_discovery_with_fetcher, DiagnosticCategory,
    DiagnosticSeverity, PostingDiscoveryFetchError, PostingDiscoveryFetchRequest,
    PostingDiscoveryFetchResponse, PostingDiscoveryFetcher, ProfileCompilerSnapshot,
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

fn compiled_json_posting_discovery_plan(fields: Value, select: Value) -> SourceExecutionPlan {
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
                    "parse": { "type": "json" },
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
    assert_eq!(result.diagnostics, Vec::new());
    result.execution_plan.expect("fixture plan should compile")
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
