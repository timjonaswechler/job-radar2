use std::{collections::BTreeMap, future::Future, pin::Pin};

use job_radar_lib::{
    compile_source_execution_plan, execute_discovery_with_clients,
    execute_discovery_with_clients_and_context, execute_discovery_with_fetcher, DiagnosticCategory,
    DiagnosticSeverity, DiscoveryFetchError, DiscoveryFetchRequest, DiscoveryFetchResponse,
    DiscoveryFetcher, ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, HttpMethod,
    ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, ProfileCompilerSnapshot, RequestBody,
    RuntimeCancellation, RuntimeExecutionContext, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument,
};
use serde_json::{json, Value};

#[path = "discovery_runtime/cancellation.rs"]
mod cancellation;
#[path = "discovery_runtime/core.rs"]
mod core;
#[path = "discovery_runtime/document_types_and_browser.rs"]
mod document_types_and_browser;
#[path = "discovery_runtime/failure_diagnostics.rs"]
mod failure_diagnostics;
#[path = "discovery_runtime/fallback_acceptance.rs"]
mod fallback_acceptance;
#[path = "discovery_runtime/pagination.rs"]
mod pagination;
#[path = "discovery_runtime/post_request_bodies.rs"]
mod post_request_bodies;
#[path = "discovery_runtime/template_validation.rs"]
mod template_validation;
#[path = "discovery_runtime/transforms_and_combine.rs"]
mod transforms_and_combine;

#[derive(Default)]
struct FakeFetcher {
    responses: BTreeMap<String, String>,
    requests: std::sync::Mutex<Vec<DiscoveryFetchRequest>>,
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

    fn requests(&self) -> Vec<DiscoveryFetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl DiscoveryFetcher for FakeFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.clone());
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                DiscoveryFetchError::new(format!("missing fake response for {}", request.url))
            })?;
            Ok(DiscoveryFetchResponse { body })
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

fn compiled_json_discovery_plan(fields: Value, select: Value) -> SourceExecutionPlan {
    compiled_discovery_plan(
        json!({ "type": "json" }),
        select,
        fields,
        "https://example.test/jobs.json",
    )
}

fn compiled_discovery_plan(
    parse: Value,
    select: Value,
    fields: Value,
    feed_url: &'static str,
) -> SourceExecutionPlan {
    compiled_discovery_plan_with_strategy(parse, select, fields, feed_url, serde_json::Map::new())
}

fn compiled_discovery_plan_with_strategy(
    parse: Value,
    select: Value,
    fields: Value,
    feed_url: &'static str,
    extra_strategy_fields: serde_json::Map<String, Value>,
) -> SourceExecutionPlan {
    let mut strategy = serde_json::Map::from_iter([
        ("key".to_string(), json!("json_api")),
        (
            "fetch".to_string(),
            json!({
                "mode": "http",
                "method": "GET",
                "url": "{{sourceConfig:feedUrl}}",
                "timeoutMs": 10000
            }),
        ),
        ("parse".to_string(), parse),
        ("select".to_string(), select),
        ("extract".to_string(), json!({ "fields": fields })),
    ]);
    strategy.extend(extra_strategy_fields);

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
                "strategies": [Value::Object(strategy)]
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

fn compiled_browser_discovery_plan(
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

fn source_owned_json_discovery_plan(fields: Value) -> SourceExecutionPlan {
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
