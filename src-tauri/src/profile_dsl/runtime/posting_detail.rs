use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use dom_query::{Document as HtmlDocument, Matcher, NodeRef, Selection as HtmlSelection};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::strategy::Acceptance,
        documents::{
            extract::{Cardinality, CombinePart, FieldExpression},
            select::CaptureRule,
            transform::Transform,
            HttpMethod, ParseType, RequestBody, Select,
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

mod document;
mod extract;
mod fetch;
mod support;
mod values;

use document::{parse_response_document, select_detail_document, RuntimeItem};
use extract::{evaluate_strategy_captures, evaluate_string_field};
use fetch::fetch_strategy_document;

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

#[derive(Clone, Debug, PartialEq)]
pub struct PostingDetailFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<RequestBody>,
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
            if let Some(body) = &request.body {
                builder = match body {
                    RequestBody::Json { value } => {
                        if !request
                            .headers
                            .keys()
                            .any(|name| name.eq_ignore_ascii_case("content-type"))
                        {
                            builder = builder.header("content-type", "application/json");
                        }
                        builder.body(
                            serde_json::to_string(value)
                                .map_err(|error| PostingDetailFetchError::new(error.to_string()))?,
                        )
                    }
                    RequestBody::Text { value } => builder.body(value.clone()),
                    RequestBody::Form { fields } => builder.form(fields),
                };
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

    if posting_detail.strategies.is_empty() {
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
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in posting_detail.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            plan,
            posting,
            fetcher,
            browser,
            strategy_index,
            strategy,
            posting_detail.accept_when.as_ref(),
        )
        .await;
        if attempt.accepted {
            diagnostics.extend(attempt.result.diagnostics);
            return PostingDetailExecutionResult {
                description_text: attempt.result.description_text,
                diagnostics,
            };
        }
        diagnostics.extend(attempt.result.diagnostics);
    }

    diagnostics.push(runtime_error(
        "fallback_exhausted",
        "postingDetail fallback strategies were exhausted without an accepted result",
        "/postingDetail/strategies",
        None,
        json!({}),
    ));
    PostingDetailExecutionResult {
        description_text: None,
        diagnostics,
    }
}

struct PostingDetailStrategyAttempt {
    result: PostingDetailExecutionResult,
    accepted: bool,
}

async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDetailStrategy,
    step_acceptance: Option<&Acceptance>,
) -> PostingDetailStrategyAttempt
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
        &plan.source.name,
        posting,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(captures) => captures,
        None => return rejected_detail_attempt(diagnostics),
    };

    let response = match fetch_strategy_document(
        fetcher,
        browser,
        &strategy.fetch,
        &plan.source_config,
        &plan.source.name,
        posting,
        &captures,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    )
    .await
    {
        Some(response) => response,
        None => return rejected_detail_attempt(diagnostics),
    };

    let document = match parse_response_document(
        &response.body,
        strategy,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };

    let selected_document = match select_detail_document(
        &document,
        &strategy.select,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };
    let selected_document = match match_detail_document(
        selected_document,
        plan,
        posting,
        &captures,
        strategy,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };

    let description_path = format!("{base_path}/extract/fields/descriptionText");
    let description = evaluate_string_field(
        &selected_document,
        &plan.source_config,
        &plan.source.name,
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
        return rejected_detail_attempt(diagnostics);
    };

    let description = normalize_whitespace(description.trim());
    let accepted = accept_posting_detail_result(
        &description,
        step_acceptance,
        strategy.accept_when.as_ref(),
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );
    PostingDetailStrategyAttempt {
        result: PostingDetailExecutionResult {
            description_text: accepted.then_some(description),
            diagnostics,
        },
        accepted,
    }
}

fn match_detail_document<'doc, 'body>(
    selected_document: RuntimeItem<'doc, 'body>,
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanPostingDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    let Some(field_match) = &strategy.field_match else {
        return Some(selected_document);
    };

    if let Some(key) = missing_posting_meta_key(&field_match.right, posting) {
        diagnostics.push(runtime_error(
            "posting_meta_missing",
            format!("postingDetail match requires missing postingMeta `{key}`"),
            format!("{base_path}/match/right"),
            strategy_key,
            json!({ "postingMetaKey": key }),
        ));
        return None;
    }

    let RuntimeItem::Json(Value::Array(items)) = selected_document else {
        diagnostics.push(runtime_error(
            "detail_match_unsupported_selection",
            "postingDetail match currently requires a JSON array selected by the strategy",
            format!("{base_path}/match"),
            strategy_key,
            json!({}),
        ));
        return None;
    };

    let mut matches = Vec::new();
    let left_path = format!("{base_path}/match/left");
    let right_path = format!("{base_path}/match/right");
    for item in items {
        let item_document = RuntimeItem::Json(item);
        let left = evaluate_string_field(
            &item_document,
            &plan.source_config,
            &plan.source.name,
            posting,
            captures,
            &field_match.left,
            &left_path,
            strategy_key,
            diagnostics,
        );
        let right = evaluate_string_field(
            &item_document,
            &plan.source_config,
            &plan.source.name,
            posting,
            captures,
            &field_match.right,
            &right_path,
            strategy_key,
            diagnostics,
        );
        if left.failed || right.failed {
            return None;
        }
        if left.value.is_some() && left.value == right.value {
            matches.push(item);
        }
    }

    match matches.len() {
        0 => {
            diagnostics.push(runtime_error(
                "detail_match_missing",
                "postingDetail match found no detail item for the selected posting",
                format!("{base_path}/match"),
                strategy_key,
                json!({}),
            ));
            None
        }
        1 => Some(RuntimeItem::Json(matches.remove(0))),
        count => {
            diagnostics.push(runtime_error(
                "detail_match_multiple",
                format!(
                    "postingDetail match found {count} detail items for the selected posting; expected exactly one"
                ),
                format!("{base_path}/match"),
                strategy_key,
                json!({ "actualCount": count }),
            ));
            None
        }
    }
}

fn missing_posting_meta_key<'a>(
    expression: &'a FieldExpression,
    posting: &PostingDetailPostingOccurrence,
) -> Option<&'a str> {
    match expression {
        FieldExpression::PostingMeta { key, .. } if !posting.posting_meta.contains_key(key) => {
            Some(key.as_str())
        }
        FieldExpression::Combine { parts, .. } => parts
            .iter()
            .find_map(|part| missing_posting_meta_key(&part.value, posting)),
        _ => None,
    }
}

fn rejected_detail_attempt(diagnostics: Diagnostics) -> PostingDetailStrategyAttempt {
    PostingDetailStrategyAttempt {
        result: PostingDetailExecutionResult {
            description_text: None,
            diagnostics,
        },
        accepted: false,
    }
}

fn accept_posting_detail_result(
    description: &str,
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some((ratio, owner_path)) =
        detail_first_max_error_ratio(step_acceptance, strategy_acceptance, base_path)
    {
        diagnostics.push(runtime_error(
            "acceptance_max_error_ratio_unsupported",
            "acceptWhen.maxErrorRatio is not supported by the postingDetail runtime result model yet",
            format!("{owner_path}/acceptWhen/maxErrorRatio"),
            strategy_key,
            json!({ "maxErrorRatio": ratio }),
        ));
        return false;
    }

    for (field, owner_path) in
        detail_required_field_rules(step_acceptance, strategy_acceptance, base_path)
    {
        if field != "descriptionText" || description.trim().is_empty() {
            diagnostics.push(runtime_error(
                "acceptance_required_field_missing",
                format!("postingDetail result is missing required normalized field `{field}`"),
                format!("{owner_path}/acceptWhen/requiredFields"),
                strategy_key,
                json!({ "field": field }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = detail_stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_description_length),
        strategy_acceptance.and_then(|acceptance| acceptance.min_description_length),
        base_path,
    ) {
        if description.chars().count() < minimum as usize {
            diagnostics.push(runtime_error(
                "description_too_short",
                format!(
                    "postingDetail descriptionText is shorter than the configured minimum of {minimum} characters"
                ),
                format!("{owner_path}/acceptWhen/minDescriptionLength"),
                strategy_key,
                json!({
                    "minDescriptionLength": minimum,
                    "actualLength": description.chars().count(),
                }),
            ));
            return false;
        }
    }

    true
}

fn detail_first_max_error_ratio(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Option<(f64, String)> {
    step_acceptance
        .and_then(|acceptance| acceptance.max_error_ratio)
        .map(|ratio| (ratio, "/postingDetail".to_string()))
        .or_else(|| {
            strategy_acceptance
                .and_then(|acceptance| acceptance.max_error_ratio)
                .map(|ratio| (ratio, base_path.to_string()))
        })
}

fn detail_required_field_rules(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    if let Some(fields) = step_acceptance.and_then(|acceptance| acceptance.required_fields.as_ref())
    {
        rules.extend(
            fields
                .iter()
                .map(|field| (field.clone(), "/postingDetail".to_string())),
        );
    }
    if let Some(fields) =
        strategy_acceptance.and_then(|acceptance| acceptance.required_fields.as_ref())
    {
        for field in fields {
            if !rules.iter().any(|(existing, _)| existing == field) {
                rules.push((field.clone(), base_path.to_string()));
            }
        }
    }
    rules
}

fn detail_stricter_u64_acceptance(
    step_value: Option<u64>,
    strategy_value: Option<u64>,
    base_path: &str,
) -> Option<(u64, String)> {
    match (step_value, strategy_value) {
        (Some(step), Some(strategy)) if strategy >= step => Some((strategy, base_path.to_string())),
        (Some(step), Some(_)) | (Some(step), None) => Some((step, "/postingDetail".to_string())),
        (None, Some(strategy)) => Some((strategy, base_path.to_string())),
        (None, None) => None,
    }
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
