use crate::support::{
    compile_test_source, execute_discovery_test, execute_discovery_test_with_config, unwrap_plan,
};

use std::{collections::BTreeMap, future::Future, pin::Pin};

use job_radar_lib::{
    execute_discovery, CompileSourceOutcome, DiagnosticCategory, DiagnosticSeverity,
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait, ExecutionPlanFetch, HttpMethod,
    PhaseCompletion, ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, ProfileHttpFailureKind,
    RuntimeCancellation, RuntimeExecutionContext, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
};
use serde_json::{json, Value};

#[path = "discovery/cancellation.rs"]
mod cancellation;
#[path = "discovery/core.rs"]
mod core;
#[path = "discovery/document_types_and_browser.rs"]
mod document_types_and_browser;
#[path = "discovery/failure_diagnostics.rs"]
mod failure_diagnostics;
#[path = "discovery/fallback_acceptance.rs"]
mod fallback_acceptance;
#[path = "discovery/occurrence.rs"]
mod occurrence;
#[path = "discovery/pagination.rs"]
mod pagination;
#[path = "discovery/post_request_bodies.rs"]
mod post_request_bodies;
#[path = "discovery/template_validation.rs"]
mod template_validation;
#[path = "discovery/transforms_and_combine.rs"]
mod transforms_and_combine;

fn fake_fetcher(
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
    unwrap_plan(compile_discovery_outcome_with_strategy(
        parse,
        select,
        fields,
        feed_url,
        extra_strategy_fields,
    ))
}

fn compile_discovery_outcome(
    parse: Value,
    select: Value,
    fields: Value,
    feed_url: &'static str,
) -> CompileSourceOutcome {
    compile_discovery_outcome_with_strategy(parse, select, fields, feed_url, serde_json::Map::new())
}

fn compile_discovery_outcome_with_strategy(
    parse: Value,
    select: Value,
    fields: Value,
    feed_url: &'static str,
    extra_strategy_fields: serde_json::Map<String, Value>,
) -> CompileSourceOutcome {
    let mut strategy = serde_json::Map::from_iter([
        ("key".to_string(), json!("json_api")),
        (
            "fetch".to_string(),
            json!({
                "mode": "http",
                "method": "GET",
                "url": feed_url,
                "timeoutMs": 10000
            }),
        ),
        ("parse".to_string(), parse),
        ("select".to_string(), select),
        ("extract".to_string(), discovery_extract(fields)),
    ]);
    strategy.extend(extra_strategy_fields);

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
            "required": ["feedUrl"],
            "properties": { "feedUrl": { "type": "string" } },
            "additionalProperties": false
        },
        "accessPaths": [{
            "key": "json_feed",
            "name": "JSON feed",
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [Value::Object(strategy)]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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

    compile_test_source(&source, Some(profile))
}

fn compiled_browser_discovery_plan(
    parse: Value,
    select: Value,
    fields: Value,
    page_url: &'static str,
) -> SourceExecutionPlan {
    let profile: SourceProfileDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "browser_html",
                    "fetch": {
                        "mode": "browser",
                        "url": page_url,
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
                    "extract": discovery_extract(fields)
                }]
            }
        }]
    }))
    .unwrap();
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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

    unwrap_plan(compile_test_source(&source, Some(profile)))
}

fn source_owned_json_discovery_plan(fields: Value) -> SourceExecutionPlan {
    let source: SourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
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
            "discovery": {
                "policy": { "type": "first_accepted" },
                "strategies": [{
                    "key": "owned_json_api",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "https://example.test/source-owned.json",
                        "timeoutMs": 10000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": discovery_extract(fields)
                }]
            }
        }
    }))
    .unwrap();

    unwrap_plan(compile_test_source(&source, None))
}

fn default_select() -> Value {
    json!({ "type": "json_path", "jsonPath": "$.jobs" })
}

fn discovery_extract(fields: Value) -> Value {
    let mut fields = fields
        .as_object()
        .cloned()
        .expect("Discovery fields object");
    let url = fields.remove("url").expect("Discovery URL expression");
    let mut provider_values = serde_json::Map::new();
    for key in ["title", "company", "locations", "descriptionText"] {
        if let Some(value) = fields.remove(key) {
            provider_values.insert(key.to_string(), value);
        }
    }
    let mut reference = serde_json::Map::from_iter([("url".to_string(), url)]);
    if let Some(provider_posting_id) = fields.remove("providerPostingId") {
        reference.insert("providerPostingId".to_string(), provider_posting_id);
    }
    let mut extract =
        serde_json::Map::from_iter([("reference".to_string(), Value::Object(reference))]);
    if !provider_values.is_empty() {
        extract.insert("providerValues".to_string(), Value::Object(provider_values));
    }
    if let Some(hints) = fields.remove("hints") {
        extract.insert("hints".to_string(), hints);
    }
    if let Some(posting_meta) = fields.remove("postingMeta") {
        extract.insert("postingMeta".to_string(), posting_meta);
    }
    Value::Object(extract)
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
