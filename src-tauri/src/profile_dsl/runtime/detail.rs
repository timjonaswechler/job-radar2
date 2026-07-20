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
            select::{CaptureRule, Filter},
            transform::Transform,
            HttpMethod, ParseType, RequestBody, Select,
        },
        execution_plan::{
            capabilities::ExecutionPlanFetch, detail::ExecutionPlanDetailStrategy,
            SourceExecutionPlan,
        },
        policy::StrategyPolicy,
    },
    simple_json_path::resolve_simple_json_path,
    source::documents::SourceConfig,
};

use super::{
    browser::{
        ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
        ProfileBrowserFetchRequest, ProfileBrowserFetchResponse, UnavailableProfileBrowserClient,
    },
    cancellation::{
        contains_runtime_execution_cancelled, push_runtime_execution_cancelled,
        RuntimeExecutionContext,
    },
    transform::{apply_transform_pipeline, normalize_whitespace},
};

mod acceptance;
mod diagnostics;
mod document;
mod extract;
mod fetch;
mod strategy;
mod support;
mod values;

use acceptance::accept_detail_result;
use diagnostics::runtime_error;
use document::{parse_response_document, select_detail_document, RuntimeItem};
use extract::{evaluate_strategy_captures, evaluate_string_field};
use fetch::fetch_strategy_document;
use strategy::execute_strategy;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailExecutionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailPostingOccurrence {
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
pub struct DetailFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<RequestBody>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailFetchError {
    pub message: String,
}

impl DetailFetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait DetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>;
}

#[derive(Clone, Debug)]
pub struct ReqwestDetailFetcher {
    client: reqwest::Client,
}

impl ReqwestDetailFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestDetailFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DetailFetcher for ReqwestDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
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
                                .map_err(|error| DetailFetchError::new(error.to_string()))?,
                        )
                    }
                    RequestBody::Text { value } => builder.body(value.clone()),
                    RequestBody::Form { fields } => builder.form(fields),
                };
            }
            let response = builder
                .send()
                .await
                .map_err(|error| DetailFetchError::new(error.to_string()))?
                .error_for_status()
                .map_err(|error| DetailFetchError::new(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| DetailFetchError::new(error.to_string()))?;
            Ok(DetailFetchResponse { body })
        })
    }
}

pub async fn execute_detail(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
) -> DetailExecutionResult {
    execute_detail_with_fetcher(plan, posting, &ReqwestDetailFetcher::new()).await
}

pub async fn execute_detail_with_fetcher<F>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
) -> DetailExecutionResult
where
    F: DetailFetcher + Sync + ?Sized,
{
    execute_detail_with_clients(plan, posting, fetcher, &UnavailableProfileBrowserClient).await
}

pub async fn execute_detail_with_clients<F, B>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
) -> DetailExecutionResult
where
    F: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    execute_detail_with_clients_and_context(
        plan,
        posting,
        fetcher,
        browser,
        RuntimeExecutionContext::uncancellable(),
    )
    .await
}

pub async fn execute_detail_with_clients_and_context<F, B>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DetailExecutionResult
where
    F: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if context.is_cancelled() {
        return cancelled_detail_result("/detail", None);
    }

    let Some(detail) = &plan.detail else {
        return DetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "detail_missing",
                "Execution Plan does not contain compiled detail",
                "/detail",
                None,
                json!({}),
            )],
        };
    };

    if detail.strategies.is_empty() {
        return DetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "detail_strategy_missing",
                "detail does not contain an executable strategy",
                "/detail/strategies",
                None,
                json!({}),
            )],
        };
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in detail.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            plan,
            posting,
            fetcher,
            browser,
            strategy_index,
            strategy,
            detail.accept_when.as_ref(),
            context,
        )
        .await;
        if contains_runtime_execution_cancelled(&attempt.result.diagnostics)
            || context.is_cancelled()
        {
            diagnostics.extend(attempt.result.diagnostics);
            push_runtime_execution_cancelled(
                &mut diagnostics,
                format!("/detail/strategies/{strategy_index}"),
                Some(&strategy.key),
            );
            return DetailExecutionResult {
                description_text: None,
                diagnostics,
            };
        }
        if attempt.accepted {
            diagnostics.extend(attempt.result.diagnostics);
            return DetailExecutionResult {
                description_text: attempt.result.description_text,
                diagnostics,
            };
        }
        diagnostics.extend(attempt.result.diagnostics);
    }

    diagnostics.push(runtime_error(
        "fallback_exhausted",
        "detail fallback strategies were exhausted without an accepted result",
        "/detail/strategies",
        None,
        json!({}),
    ));
    DetailExecutionResult {
        description_text: None,
        diagnostics,
    }
}

pub async fn execute_policy_detail_with_clients_and_context<F, B>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DetailExecutionResult
where
    F: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let Some(detail) = &plan.detail else {
        return DetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "detail_missing",
                "Execution Plan does not contain compiled detail",
                "/detail",
                None,
                json!({}),
            )],
        };
    };
    match detail.policy {
        StrategyPolicy::FirstAccepted => {
            execute_policy_first_accepted(plan, posting, fetcher, browser, context).await
        }
    }
}

async fn execute_policy_first_accepted<F, B>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DetailExecutionResult
where
    F: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if context.is_cancelled() {
        return cancelled_detail_result("/detail", None);
    }
    let detail = plan
        .detail
        .as_ref()
        .expect("policy-bearing detail dispatch requires compiled detail");
    if detail.strategies.is_empty() {
        return DetailExecutionResult {
            description_text: None,
            diagnostics: vec![runtime_error(
                "detail_strategy_missing",
                "detail does not contain an executable strategy",
                "/detail/strategies",
                None,
                json!({}),
            )],
        };
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in detail.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            &plan,
            posting,
            fetcher,
            browser,
            strategy_index,
            strategy,
            detail.accept_when.as_ref(),
            context,
        )
        .await;
        if contains_runtime_execution_cancelled(&attempt.result.diagnostics)
            || context.is_cancelled()
        {
            diagnostics.extend(attempt.result.diagnostics);
            push_runtime_execution_cancelled(
                &mut diagnostics,
                format!("/detail/strategies/{strategy_index}"),
                Some(&strategy.key),
            );
            return DetailExecutionResult {
                description_text: None,
                diagnostics,
            };
        }
        diagnostics.extend(attempt.result.diagnostics);
        if attempt.accepted {
            return DetailExecutionResult {
                description_text: attempt.result.description_text,
                diagnostics,
            };
        }
    }

    diagnostics.push(runtime_error(
        "fallback_exhausted",
        "detail fallback strategies were exhausted without an accepted result",
        "/detail/strategies",
        None,
        json!({}),
    ));
    DetailExecutionResult {
        description_text: None,
        diagnostics,
    }
}

fn cancelled_detail_result(path: &str, strategy_key: Option<&str>) -> DetailExecutionResult {
    let mut diagnostics = Vec::new();
    push_runtime_execution_cancelled(&mut diagnostics, path, strategy_key);
    DetailExecutionResult {
        description_text: None,
        diagnostics,
    }
}
