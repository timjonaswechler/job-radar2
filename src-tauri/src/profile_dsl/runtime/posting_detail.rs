use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use dom_query::{Document as HtmlDocument, Matcher, NodeRef, Selection as HtmlSelection};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::{
            extract::{Cardinality, CombinePart, FieldExpression},
            select::CaptureRule,
            transform::Transform,
            HttpMethod, ParseType, Select,
        },
        execution_plan::{
            capabilities::ExecutionPlanFetch, posting_detail::ExecutionPlanPostingDetailStrategy,
            SourceExecutionPlan,
        },
    },
    simple_json_path::resolve_simple_json_path,
    source::documents::SourceConfig,
};

use super::{
    browser::{
        ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
        ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, UnavailableProfileBrowserClient,
    },
    transform::{apply_transform_pipeline, normalize_whitespace},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDetailExecutionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDetailPostingOccurrence {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub posting_meta: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDetailFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDetailFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDetailFetchError {
    pub message: String,
}

impl PostingDetailFetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait PostingDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDetailFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDetailFetchResponse, PostingDetailFetchError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone, Debug)]
pub struct ReqwestPostingDetailFetcher {
    client: reqwest::Client,
}

impl ReqwestPostingDetailFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestPostingDetailFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PostingDetailFetcher for ReqwestPostingDetailFetcher {
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
            let method = match request.method {
                HttpMethod::Get => reqwest::Method::GET,
                HttpMethod::Post => reqwest::Method::POST,
            };
            let mut builder = self
                .client
                .request(method, &request.url)
                .timeout(Duration::from_millis(request.timeout_ms));
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            let response = builder
                .send()
                .await
                .map_err(|error| PostingDetailFetchError::new(error.to_string()))?
                .error_for_status()
                .map_err(|error| PostingDetailFetchError::new(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| PostingDetailFetchError::new(error.to_string()))?;
            Ok(PostingDetailFetchResponse { body })
        })
    }
}

pub async fn execute_posting_detail(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
) -> PostingDetailExecutionResult {
    execute_posting_detail_with_fetcher(plan, posting, &ReqwestPostingDetailFetcher::new()).await
}

pub async fn execute_posting_detail_with_fetcher<F>(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    fetcher: &F,
) -> PostingDetailExecutionResult
where
    F: PostingDetailFetcher + Sync + ?Sized,
{
    execute_posting_detail_with_clients(plan, posting, fetcher, &UnavailableProfileBrowserClient)
        .await
}

pub async fn execute_posting_detail_with_clients<F, B>(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
) -> PostingDetailExecutionResult
where
    F: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let Some(posting_detail) = &plan.posting_detail else {
        return PostingDetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "posting_detail_missing",
                "Execution Plan does not contain compiled postingDetail",
                "/postingDetail",
                None,
                json!({}),
            )],
        };
    };

    let Some((strategy_index, strategy)) = posting_detail.strategies.iter().enumerate().next()
    else {
        return PostingDetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "posting_detail_strategy_missing",
                "postingDetail does not contain an executable strategy",
                "/postingDetail/strategies",
                None,
                json!({}),
            )],
        };
    };

    execute_strategy(plan, posting, fetcher, browser, strategy_index, strategy).await
}

async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDetailStrategy,
) -> PostingDetailExecutionResult
where
    F: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/postingDetail/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let captures = match evaluate_strategy_captures(
        strategy,
        &plan.source_config,
        posting,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(captures) => captures,
        None => {
            return PostingDetailExecutionResult {
                description_text: None,
                diagnostics,
            }
        }
    };

    let response = match fetch_strategy_document(
        fetcher,
        browser,
        &strategy.fetch,
        &plan.source_config,
        posting,
        &captures,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    )
    .await
    {
        Some(response) => response,
        None => {
            return PostingDetailExecutionResult {
                description_text: None,
                diagnostics,
            }
        }
    };

    let document = match parse_response_document(
        &response.body,
        strategy,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => {
            return PostingDetailExecutionResult {
                description_text: None,
                diagnostics,
            }
        }
    };

    let selected_document = match select_detail_document(
        &document,
        &strategy.select,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => {
            return PostingDetailExecutionResult {
                description_text: None,
                diagnostics,
            }
        }
    };

    let description_path = format!("{base_path}/extract/fields/descriptionText");
    let description = evaluate_string_field(
        &selected_document,
        &plan.source_config,
        posting,
        &captures,
        &strategy.extract.fields.description_text,
        &description_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );

    let Some(description) = description.value.filter(|value| !value.trim().is_empty()) else {
        if !description.failed {
            diagnostics.push(runtime_error(
                "description_empty",
                "postingDetail descriptionText did not resolve to non-empty text",
                &description_path,
                strategy_key.as_deref(),
                json!({}),
            ));
        }
        return PostingDetailExecutionResult {
            description_text: None,
            diagnostics,
        };
    };

    let description = normalize_whitespace(description.trim());
    if let Some(minimum) = strategy
        .accept_when
        .as_ref()
        .and_then(|acceptance| acceptance.min_description_length)
        .or_else(|| {
            plan.posting_detail
                .as_ref()
                .and_then(|step| step.accept_when.as_ref())
                .and_then(|acceptance| acceptance.min_description_length)
        })
    {
        if description.chars().count() < minimum as usize {
            diagnostics.push(runtime_error(
                "description_too_short",
                format!(
                    "postingDetail descriptionText is shorter than the configured minimum of {minimum} characters"
                ),
                &description_path,
                strategy_key.as_deref(),
                json!({
                    "minDescriptionLength": minimum,
                    "actualLength": description.chars().count(),
                }),
            ));
            return PostingDetailExecutionResult {
                description_text: None,
                diagnostics,
            };
        }
    }

    PostingDetailExecutionResult {
        description_text: Some(description),
        diagnostics,
    }
}

async fn fetch_strategy_document<F, B>(
    fetcher: &F,
    browser: &B,
    fetch: &ExecutionPlanFetch,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
where
    F: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let context = TemplateRuntimeContext {
        source_config,
        posting,
        posting_meta: &posting.posting_meta,
        captures,
    };

    match fetch {
        ExecutionPlanFetch::Http {
            method,
            url,
            headers,
            timeout_ms,
            ..
        } => {
            fetch_http_strategy_document(
                fetcher,
                *method,
                url,
                headers.as_ref(),
                *timeout_ms,
                &context,
                base_path,
                strategy_key,
                diagnostics,
            )
            .await
        }
        ExecutionPlanFetch::Browser {
            url,
            timeout_ms,
            waits,
            interactions,
        } => {
            fetch_browser_strategy_document(
                browser,
                url,
                *timeout_ms,
                waits,
                interactions,
                &context,
                base_path,
                strategy_key,
                diagnostics,
            )
            .await
        }
    }
}

async fn fetch_http_strategy_document<F>(
    fetcher: &F,
    method: Option<HttpMethod>,
    url: &str,
    headers: Option<&BTreeMap<String, String>>,
    timeout_ms: u64,
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
where
    F: PostingDetailFetcher + Sync + ?Sized,
{
    let method = method.unwrap_or(HttpMethod::Get);
    if method != HttpMethod::Get {
        diagnostics.push(runtime_error(
            "unsupported_http_method",
            "postingDetail runtime slice supports only HTTP GET",
            format!("{base_path}/fetch/method"),
            strategy_key,
            json!({ "supportedMethod": "GET" }),
        ));
        return None;
    }

    let rendered_url = match render_template(url, context) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let rendered_headers = match render_headers(headers, context) {
        Ok(headers) => headers,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch header template could not be rendered: {message}"),
                format!("{base_path}/fetch/headers"),
                strategy_key,
                json!({}),
            ));
            return None;
        }
    };

    let request = PostingDetailFetchRequest {
        method,
        url: rendered_url.clone(),
        headers: rendered_headers,
        timeout_ms,
    };

    match fetcher.fetch(request).await {
        Ok(response) => Some(response),
        Err(error) => {
            diagnostics.push(runtime_error(
                "fetch_failed",
                format!(
                    "HTTP GET fetch failed for {rendered_url}: {}",
                    error.message
                ),
                format!("{base_path}/fetch"),
                strategy_key,
                json!({ "url": rendered_url, "error": error.message }),
            ));
            None
        }
    }
}

async fn fetch_browser_strategy_document<B>(
    browser: &B,
    url: &str,
    timeout_ms: u64,
    waits: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserWait],
    interactions: &[crate::profile_dsl::execution_plan::capabilities::ExecutionPlanBrowserInteraction],
    context: &TemplateRuntimeContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<PostingDetailFetchResponse>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let rendered_url = match render_template(url, context) {
        Ok(url) => url,
        Err(message) => {
            diagnostics.push(runtime_error(
                "runtime_template_context_missing",
                format!("Fetch URL template could not be rendered: {message}"),
                format!("{base_path}/fetch/url"),
                strategy_key,
                json!({ "template": url }),
            ));
            return None;
        }
    };

    let request = ProfileBrowserFetchRequest {
        url: rendered_url.clone(),
        timeout_ms,
        waits: waits.to_vec(),
        interactions: interactions.to_vec(),
    };

    match browser.render(request).await {
        Ok(ProfileBrowserFetchResponse { body }) => Some(PostingDetailFetchResponse { body }),
        Err(error) => {
            push_browser_fetch_diagnostic(
                error,
                &rendered_url,
                base_path,
                strategy_key,
                diagnostics,
            );
            None
        }
    }
}

fn render_headers(
    headers: Option<&BTreeMap<String, String>>,
    context: &TemplateRuntimeContext<'_>,
) -> Result<BTreeMap<String, String>, String> {
    let mut rendered = BTreeMap::new();
    for (name, value) in headers.into_iter().flatten() {
        rendered.insert(name.clone(), render_template(value, context)?);
    }
    Ok(rendered)
}

enum ParsedDocument<'body> {
    Json(Value),
    Xml(roxmltree::Document<'body>),
    Html(HtmlDocument),
}

#[derive(Clone)]
enum RuntimeItem<'doc, 'body> {
    Json(&'doc Value),
    Xml(roxmltree::Node<'doc, 'body>),
    Html(NodeRef<'doc>),
    Text(String),
}

fn parse_response_document<'body>(
    body: &'body str,
    strategy: &ExecutionPlanPostingDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<ParsedDocument<'body>> {
    match strategy.parse.parse_type {
        ParseType::Json => match serde_json::from_str(body) {
            Ok(document) => Some(ParsedDocument::Json(document)),
            Err(error) => {
                diagnostics.push(runtime_error(
                    "json_parse_failed",
                    format!("Fetched response could not be parsed as JSON: {error}"),
                    format!("{base_path}/parse"),
                    strategy_key,
                    json!({ "error": error.to_string() }),
                ));
                None
            }
        },
        ParseType::Xml => match roxmltree::Document::parse(body) {
            Ok(document) => Some(ParsedDocument::Xml(document)),
            Err(error) => {
                diagnostics.push(runtime_error(
                    "xml_parse_failed",
                    format!("Fetched response could not be parsed as XML: {error}"),
                    format!("{base_path}/parse"),
                    strategy_key,
                    json!({ "error": error.to_string() }),
                ));
                None
            }
        },
        ParseType::Html => Some(ParsedDocument::Html(HtmlDocument::from(body))),
        ParseType::Text => {
            diagnostics.push(runtime_error(
                "unsupported_parse_type",
                "postingDetail runtime supports JSON, XML, and HTML parse types",
                format!("{base_path}/parse/type"),
                strategy_key,
                json!({ "supportedTypes": ["json", "xml", "html"] }),
            ));
            None
        }
    }
}

fn select_detail_document<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &Select,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    match (document, select) {
        (ParsedDocument::Json(document), Select::Document) => Some(RuntimeItem::Json(document)),
        (ParsedDocument::Json(document), Select::JsonPath { json_path }) => {
            match resolve_simple_json_path(document, json_path) {
                Ok(Some(value)) => Some(RuntimeItem::Json(value)),
                Ok(None) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_missing",
                        "JSONPath selector did not match a posting detail document",
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path }),
                    ));
                    None
                }
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "json_path_select_failed",
                        format!("JSONPath selector is invalid: {error}"),
                        format!("{base_path}/select/jsonPath"),
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Xml(document), Select::Document) => {
            Some(RuntimeItem::Xml(document.root_element()))
        }
        (ParsedDocument::Xml(document), Select::XmlElement { element }) => {
            let mut items = xml_descendant_elements(document.root_element(), element);
            match items.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "xml_select_missing",
                        "XML element selector did not match a posting detail document",
                        format!("{base_path}/select/element"),
                        strategy_key,
                        json!({ "element": element }),
                    ));
                    None
                }
                1 => Some(RuntimeItem::Xml(items.remove(0))),
                count => {
                    diagnostics.push(runtime_error(
                        "xml_select_multiple",
                        "XML element selector matched multiple posting detail documents",
                        format!("{base_path}/select/element"),
                        strategy_key,
                        json!({ "element": element, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Xml(document), Select::XmlText { text_path }) => {
            let texts = xml_path_texts(document.root_element(), text_path);
            match texts.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "xml_text_select_missing",
                        "XML text selector did not match posting detail text",
                        format!("{base_path}/select/textPath"),
                        strategy_key,
                        json!({ "textPath": text_path }),
                    ));
                    None
                }
                1 => Some(RuntimeItem::Text(texts.into_iter().next().unwrap())),
                count => {
                    diagnostics.push(runtime_error(
                        "xml_text_select_multiple",
                        "XML text selector matched multiple posting detail text values",
                        format!("{base_path}/select/textPath"),
                        strategy_key,
                        json!({ "textPath": text_path, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        (ParsedDocument::Html(document), Select::Document) => {
            Some(RuntimeItem::Html(document.tree.root()))
        }
        (ParsedDocument::Html(document), Select::Css { selector }) => {
            let matcher = match Matcher::new(selector) {
                Ok(matcher) => matcher,
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "css_select_failed",
                        format!("CSS selector is invalid: {error:?}"),
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector, "error": format!("{error:?}") }),
                    ));
                    return None;
                }
            };
            let mut nodes = document
                .select_matcher(&matcher)
                .nodes()
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            match nodes.len() {
                0 => {
                    diagnostics.push(runtime_error(
                        "css_select_missing",
                        "CSS selector did not match a posting detail document",
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector }),
                    ));
                    None
                }
                1 => Some(RuntimeItem::Html(nodes.remove(0))),
                count => {
                    diagnostics.push(runtime_error(
                        "css_select_multiple",
                        "CSS selector matched multiple posting detail documents",
                        format!("{base_path}/select/selector"),
                        strategy_key,
                        json!({ "selector": selector, "actualCount": count }),
                    ));
                    None
                }
            }
        }
        _ => {
            diagnostics.push(runtime_error(
                "unsupported_select_type",
                "Select type is not compatible with the parsed response document",
                format!("{base_path}/select"),
                strategy_key,
                json!({}),
            ));
            None
        }
    }
}

fn evaluate_strategy_captures(
    strategy: &ExecutionPlanPostingDetailStrategy,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let mut captures = BTreeMap::new();
    let empty_document = Value::Null;
    let empty_item = RuntimeItem::Json(&empty_document);
    let Some(capture_rules) = &strategy.captures else {
        return Some(captures);
    };

    for (key, rule) in capture_rules {
        let path = format!("{base_path}/captures/{key}");
        let context_captures = captures.clone();
        let evaluation = evaluate_string_field(
            &empty_item,
            source_config,
            posting,
            &context_captures,
            &rule.from,
            &format!("{path}/from"),
            strategy_key,
            diagnostics,
        );
        if evaluation.failed {
            return None;
        }
        let Some(value) = evaluation.value else {
            diagnostics.push(runtime_error(
                "capture_source_missing",
                format!("Capture `{key}` source did not resolve to text"),
                &path,
                strategy_key,
                json!({ "captureKey": key }),
            ));
            return None;
        };
        let Some(captured) =
            apply_capture_rule(key, &value, rule, &path, strategy_key, diagnostics)
        else {
            return None;
        };
        captures.insert(key.clone(), captured);
    }

    Some(captures)
}

fn apply_capture_rule(
    key: &str,
    value: &str,
    rule: &CaptureRule,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    let regex = match Regex::new(&rule.pattern) {
        Ok(regex) => regex,
        Err(error) => {
            diagnostics.push(runtime_error(
                "capture_pattern_invalid",
                format!("Capture `{key}` pattern is invalid: {error}"),
                format!("{path}/pattern"),
                strategy_key,
                json!({ "captureKey": key, "error": error.to_string() }),
            ));
            return None;
        }
    };
    let Some(captures) = regex.captures(value) else {
        diagnostics.push(runtime_error(
            "capture_not_matched",
            format!("Capture `{key}` pattern did not match runtime text"),
            path,
            strategy_key,
            json!({ "captureKey": key }),
        ));
        return None;
    };

    let captured = captures
        .name("value")
        .or_else(|| {
            regex
                .capture_names()
                .flatten()
                .find_map(|name| captures.name(name))
        })
        .or_else(|| captures.get(1))
        .or_else(|| captures.get(0))
        .map(|matched| matched.as_str().trim().to_string())
        .filter(|value| !value.is_empty());

    match captured {
        Some(value) => Some(value),
        None => {
            diagnostics.push(runtime_error(
                "capture_empty",
                format!("Capture `{key}` resolved to empty text"),
                path,
                strategy_key,
                json!({ "captureKey": key }),
            ));
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldEvaluation {
    value: Option<String>,
    failed: bool,
}

fn evaluate_string_field(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let RawFieldValues {
        values,
        failed,
        cardinality,
        transforms,
    } = raw_field_values(
        document,
        source_config,
        posting,
        captures,
        expression,
        path,
        strategy_key,
        diagnostics,
    );
    if failed {
        return FieldEvaluation {
            value: None,
            failed: true,
        };
    }

    let values = match apply_transforms(values, transforms, path, strategy_key, diagnostics) {
        Some(values) => values,
        None => {
            return FieldEvaluation {
                value: None,
                failed: true,
            };
        }
    };

    let mut normalized_values = Vec::new();
    for value in values {
        let value = normalize_whitespace(value.trim());
        if !value.is_empty() {
            normalized_values.push(value);
        }
    }

    match cardinality.unwrap_or(Cardinality::One) {
        Cardinality::One => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => cardinality_mismatch(path, strategy_key, count, "one", diagnostics),
        },
        Cardinality::First => FieldEvaluation {
            value: normalized_values.into_iter().next(),
            failed: false,
        },
        Cardinality::Optional => match normalized_values.len() {
            0 => FieldEvaluation {
                value: None,
                failed: false,
            },
            1 => FieldEvaluation {
                value: normalized_values.into_iter().next(),
                failed: false,
            },
            count => cardinality_mismatch(path, strategy_key, count, "optional", diagnostics),
        },
        Cardinality::All => FieldEvaluation {
            value: normalized_values.into_iter().next(),
            failed: false,
        },
    }
}

fn cardinality_mismatch(
    path: &str,
    strategy_key: Option<&str>,
    actual_count: usize,
    expected: &str,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    diagnostics.push(runtime_error(
        "field_cardinality_mismatch",
        format!("Field cardinality `{expected}` did not match {actual_count} resolved values"),
        path,
        strategy_key,
        json!({
            "expectedCardinality": expected,
            "actualCount": actual_count,
        }),
    ));
    FieldEvaluation {
        value: None,
        failed: true,
    }
}

struct RawFieldValues<'a> {
    values: Vec<String>,
    failed: bool,
    cardinality: Option<Cardinality>,
    transforms: Option<&'a Vec<Transform>>,
}

fn raw_field_values<'a>(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &'a FieldExpression,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    match expression {
        FieldExpression::Const {
            value,
            cardinality,
            transforms,
        } => json_value_to_strings(value, path, strategy_key, diagnostics)
            .into_raw(*cardinality, transforms.as_ref()),
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match resolve_simple_json_path(value, json_path) {
                Ok(Some(value)) => json_value_to_strings(value, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref()),
                Ok(None) => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
                Err(error) => {
                    diagnostics.push(runtime_error(
                        "field_json_path_failed",
                        format!("Field JSONPath is invalid: {error}"),
                        path,
                        strategy_key,
                        json!({ "jsonPath": json_path, "error": error.to_string() }),
                    ));
                    RawFieldValues {
                        values: Vec::new(),
                        failed: true,
                        cardinality: *cardinality,
                        transforms: transforms.as_ref(),
                    }
                }
            },
            _ => incompatible_field_expression(
                "field_json_path_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => match source_config.get(key) {
            Some(value) => json_value_to_strings(value, path, strategy_key, diagnostics)
                .into_raw(*cardinality, transforms.as_ref()),
            None => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::PostingMeta {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: posting.posting_meta.get(key).cloned().into_iter().collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms.as_ref(),
        },
        FieldExpression::Capture {
            key,
            cardinality,
            transforms,
        } => RawFieldValues {
            values: captures.get(key).cloned().into_iter().collect(),
            failed: false,
            cardinality: *cardinality,
            transforms: transforms.as_ref(),
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Json(value) => match value.get(key) {
                Some(value) => json_value_to_strings(value, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref()),
                None => RawFieldValues {
                    values: Vec::new(),
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
            },
            RuntimeItem::Text(value) if key == "value" || key == "." => RawFieldValues {
                values: vec![value.clone()],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => RawFieldValues {
                values: Vec::new(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
        },
        FieldExpression::Template {
            template,
            cardinality,
            transforms,
        } => {
            let context = TemplateRuntimeContext {
                source_config,
                posting,
                posting_meta: &posting.posting_meta,
                captures,
            };
            match render_template(template, &context) {
                Ok(value) => RawFieldValues {
                    values: vec![value],
                    failed: false,
                    cardinality: *cardinality,
                    transforms: transforms.as_ref(),
                },
                Err(message) => {
                    diagnostics.push(runtime_error(
                        "runtime_template_context_missing",
                        format!("Field template could not be rendered: {message}"),
                        path,
                        strategy_key,
                        json!({ "template": template }),
                    ));
                    RawFieldValues {
                        values: Vec::new(),
                        failed: true,
                        cardinality: *cardinality,
                        transforms: transforms.as_ref(),
                    }
                }
            }
        }
        FieldExpression::XmlText {
            text_path,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Xml(node) => RawFieldValues {
                values: xml_path_texts(*node, text_path),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            RuntimeItem::Text(value) if text_path == "." => RawFieldValues {
                values: vec![value.clone()],
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => incompatible_field_expression(
                "field_xml_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::XmlElement {
            element,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Xml(node) => RawFieldValues {
                values: xml_descendant_elements(*node, element)
                    .into_iter()
                    .map(xml_node_text)
                    .collect(),
                failed: false,
                cardinality: *cardinality,
                transforms: transforms.as_ref(),
            },
            _ => incompatible_field_expression(
                "field_xml_element_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::CssText {
            selector,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Html(node) => {
                css_text_values(node, selector, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            _ => incompatible_field_expression(
                "field_css_text_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::CssAttribute {
            selector,
            attribute,
            cardinality,
            transforms,
        } => match document {
            RuntimeItem::Html(node) => {
                css_attribute_values(node, selector, attribute, path, strategy_key, diagnostics)
                    .into_raw(*cardinality, transforms.as_ref())
            }
            _ => incompatible_field_expression(
                "field_css_attribute_incompatible",
                path,
                strategy_key,
                *cardinality,
                transforms.as_ref(),
                diagnostics,
            ),
        },
        FieldExpression::Combine {
            parts,
            join,
            cardinality,
            transforms,
        } => combine_field_values(
            document,
            source_config,
            posting,
            captures,
            parts,
            join.as_deref().unwrap_or_default(),
            path,
            strategy_key,
            diagnostics,
        )
        .into_raw(*cardinality, transforms.as_ref()),
    }
}

fn combine_field_values(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    parts: &[CombinePart],
    join: &str,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part_path = format!("{path}/parts/{index}/value");
        match evaluate_string_field(
            document,
            source_config,
            posting,
            captures,
            &part.value,
            &part_path,
            strategy_key,
            diagnostics,
        ) {
            FieldEvaluation {
                value: Some(value),
                failed: false,
            } => values.push(value),
            FieldEvaluation {
                value: None,
                failed: false,
            } if part.optional.unwrap_or(false) => {}
            FieldEvaluation {
                value: None,
                failed: false,
            } => {
                diagnostics.push(runtime_error(
                    "required_combine_part_missing",
                    "Required combine part did not resolve to a non-empty string",
                    &part_path,
                    strategy_key,
                    json!({ "partIndex": index }),
                ));
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
            FieldEvaluation { failed: true, .. } => {
                return JsonStringsResult {
                    values: Vec::new(),
                    failed: true,
                };
            }
        }
    }

    JsonStringsResult {
        values: vec![values.join(join)],
        failed: false,
    }
}

fn incompatible_field_expression<'a>(
    code: &'static str,
    path: &str,
    strategy_key: Option<&str>,
    cardinality: Option<Cardinality>,
    transforms: Option<&'a Vec<Transform>>,
    diagnostics: &mut Diagnostics,
) -> RawFieldValues<'a> {
    diagnostics.push(runtime_error(
        code,
        "Field expression is not compatible with the selected detail document type",
        path,
        strategy_key,
        json!({}),
    ));
    RawFieldValues {
        values: Vec::new(),
        failed: true,
        cardinality,
        transforms,
    }
}

fn css_text_values(
    node: &NodeRef<'_>,
    selector: &str,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) = select_relative_html(node, selector, path, strategy_key, diagnostics)
    else {
        return JsonStringsResult {
            values: Vec::new(),
            failed: true,
        };
    };
    JsonStringsResult {
        values: selection
            .iter()
            .map(|selected| selected.formatted_text().to_string())
            .collect(),
        failed: false,
    }
}

fn css_attribute_values(
    node: &NodeRef<'_>,
    selector: &str,
    attribute: &str,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    let Some(selection) = select_relative_html(node, selector, path, strategy_key, diagnostics)
    else {
        return JsonStringsResult {
            values: Vec::new(),
            failed: true,
        };
    };
    JsonStringsResult {
        values: selection
            .iter()
            .filter_map(|selected| selected.attr(attribute).map(|value| value.to_string()))
            .collect(),
        failed: false,
    }
}

fn select_relative_html<'a>(
    node: &NodeRef<'a>,
    selector: &str,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<HtmlSelection<'a>> {
    let matcher = match Matcher::new(selector) {
        Ok(matcher) => matcher,
        Err(error) => {
            diagnostics.push(runtime_error(
                "field_css_selector_failed",
                format!("Field CSS selector is invalid: {error:?}"),
                path,
                strategy_key,
                json!({ "selector": selector, "error": format!("{error:?}") }),
            ));
            return None;
        }
    };
    Some(HtmlSelection::from(node.clone()).select_matcher(&matcher))
}

fn xml_descendant_elements<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    element: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    node.descendants()
        .filter(|candidate| candidate.is_element() && candidate.tag_name().name() == element)
        .collect()
}

fn xml_path_texts(node: roxmltree::Node<'_, '_>, text_path: &str) -> Vec<String> {
    xml_path_nodes(node, text_path)
        .into_iter()
        .map(xml_node_text)
        .collect()
}

fn xml_path_nodes<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    path: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    let trimmed = path.trim();
    if trimmed == "." || trimmed.is_empty() {
        return vec![node];
    }

    let parts = trimmed
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return vec![node];
    }

    if parts.len() == 1 {
        return xml_descendant_elements(node, parts[0]);
    }

    let mut current = vec![node];
    for (index, part) in parts.into_iter().enumerate() {
        let mut next = Vec::new();
        for candidate in current {
            if index == 0 && candidate.is_element() && candidate.tag_name().name() == part {
                next.push(candidate);
            }
            next.extend(
                candidate
                    .children()
                    .filter(|child| child.is_element() && child.tag_name().name() == part),
            );
        }
        current = next;
        if current.is_empty() {
            break;
        }
    }
    current
}

fn xml_node_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect::<Vec<_>>()
        .join(" ")
}

struct JsonStringsResult {
    values: Vec<String>,
    failed: bool,
}

impl JsonStringsResult {
    fn into_raw(
        self,
        cardinality: Option<Cardinality>,
        transforms: Option<&Vec<Transform>>,
    ) -> RawFieldValues<'_> {
        RawFieldValues {
            values: self.values,
            failed: self.failed,
            cardinality,
            transforms,
        }
    }
}

fn json_value_to_strings(
    value: &Value,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> JsonStringsResult {
    match value {
        Value::Null => JsonStringsResult {
            values: Vec::new(),
            failed: false,
        },
        Value::String(value) => JsonStringsResult {
            values: vec![value.clone()],
            failed: false,
        },
        Value::Number(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Bool(value) => JsonStringsResult {
            values: vec![value.to_string()],
            failed: false,
        },
        Value::Array(values) => {
            let mut strings = Vec::new();
            for (value_index, value) in values.iter().enumerate() {
                match value {
                    Value::Null => {}
                    Value::String(value) => strings.push(value.clone()),
                    Value::Number(value) => strings.push(value.to_string()),
                    Value::Bool(value) => strings.push(value.to_string()),
                    Value::Array(_) | Value::Object(_) => {
                        diagnostics.push(runtime_error(
                            "field_type_mismatch",
                            "Field array values must resolve to strings, numbers, booleans, or null",
                            path,
                            strategy_key,
                            json!({ "valueIndex": value_index }),
                        ));
                        return JsonStringsResult {
                            values: Vec::new(),
                            failed: true,
                        };
                    }
                }
            }
            JsonStringsResult {
                values: strings,
                failed: false,
            }
        }
        Value::Object(_) => {
            diagnostics.push(runtime_error(
                "field_type_mismatch",
                "Field value must resolve to a string, number, boolean, null, or an array of scalar values",
                path,
                strategy_key,
                json!({}),
            ));
            JsonStringsResult {
                values: Vec::new(),
                failed: true,
            }
        }
    }
}

fn apply_transforms(
    values: Vec<String>,
    transforms: Option<&Vec<Transform>>,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    match apply_transform_pipeline(values, transforms) {
        Ok(values) => Some(values),
        Err(error) => {
            diagnostics.push(runtime_error(
                error.code,
                error.message,
                path,
                strategy_key,
                json!({ "transform": error.transform }),
            ));
            None
        }
    }
}

struct TemplateRuntimeContext<'a> {
    source_config: &'a SourceConfig,
    posting: &'a PostingDetailPostingOccurrence,
    posting_meta: &'a BTreeMap<String, String>,
    captures: &'a BTreeMap<String, String>,
}

fn render_template(template: &str, context: &TemplateRuntimeContext<'_>) -> Result<String, String> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered = placeholder_regex
        .replace_all(template, |captures: &regex::Captures<'_>| {
            let variable = captures[1].trim();
            match render_template_variable(variable, context) {
                Ok(value) => value,
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    String::new()
                }
            }
        })
        .to_string();

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(rendered)
    }
}

fn render_template_variable(
    variable: &str,
    context: &TemplateRuntimeContext<'_>,
) -> Result<String, String> {
    let Some((namespace, key)) = split_template_variable(variable) else {
        return Err(format!(
            "template variable `{variable}` must use namespace:key syntax"
        ));
    };

    match namespace {
        "sourceConfig" => source_config_value_as_string(context.source_config, key)
            .ok_or_else(|| format!("sourceConfig `{key}` is missing or not scalar")),
        "captures" => context
            .captures
            .get(key)
            .cloned()
            .ok_or_else(|| format!("capture `{key}` is missing")),
        "postingMeta" => context
            .posting_meta
            .get(key)
            .cloned()
            .ok_or_else(|| format!("postingMeta `{key}` is missing")),
        "posting" => posting_value_as_string(context.posting, key)
            .ok_or_else(|| format!("posting `{key}` is missing or not scalar")),
        _ => Err(format!("unsupported template namespace `{namespace}`")),
    }
}

fn split_template_variable(variable: &str) -> Option<(&str, &str)> {
    variable
        .split_once(':')
        .or_else(|| variable.split_once('.'))
        .filter(|(namespace, key)| !namespace.is_empty() && !key.is_empty())
}

fn source_config_value_as_string(source_config: &SourceConfig, key: &str) -> Option<String> {
    match source_config.get(key)? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn posting_value_as_string(posting: &PostingDetailPostingOccurrence, key: &str) -> Option<String> {
    match key {
        "url" => Some(posting.url.clone()),
        "title" => posting.title.clone(),
        "company" => posting.company.clone(),
        "descriptionText" => posting.description_text.clone(),
        "locations" if !posting.locations.is_empty() => Some(posting.locations.join(", ")),
        _ => None,
    }
}

fn push_browser_fetch_diagnostic(
    error: ProfileBrowserFetchError,
    rendered_url: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) {
    let (code, path) = match error.kind {
        ProfileBrowserFetchErrorKind::RuntimeUnavailable => {
            ("browser_runtime_unavailable", format!("{base_path}/fetch"))
        }
        ProfileBrowserFetchErrorKind::NavigationFailed => (
            "browser_navigation_failed",
            format!("{base_path}/fetch/url"),
        ),
        ProfileBrowserFetchErrorKind::WaitTimeout { wait_index } => (
            "browser_wait_timeout",
            wait_index
                .map(|index| format!("{base_path}/fetch/waits/{index}"))
                .unwrap_or_else(|| format!("{base_path}/fetch/waits")),
        ),
        ProfileBrowserFetchErrorKind::InteractionFailed { interaction_index } => (
            "browser_interaction_failed",
            interaction_index
                .map(|index| format!("{base_path}/fetch/interactions/{index}"))
                .unwrap_or_else(|| format!("{base_path}/fetch/interactions")),
        ),
        ProfileBrowserFetchErrorKind::RenderTimeout => (
            "browser_render_timeout",
            format!("{base_path}/fetch/timeoutMs"),
        ),
        ProfileBrowserFetchErrorKind::ContentReadFailed => {
            ("browser_content_read_failed", format!("{base_path}/fetch"))
        }
    };

    diagnostics.push(runtime_error(
        code,
        format!("Browser fetch failed for {rendered_url}: {}", error.message),
        path,
        strategy_key,
        json!({ "url": rendered_url, "error": error.message }),
    ));
}

fn runtime_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(details),
    }
}
