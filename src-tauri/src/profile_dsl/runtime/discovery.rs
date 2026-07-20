use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    future::Future,
    pin::Pin,
    time::Duration,
};

use dom_query::{Document as HtmlDocument, Matcher, NodeRef, Selection as HtmlSelection};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::strategy::Acceptance,
        documents::{
            extract::{Cardinality, CombinePart, FieldExpression, ListFieldExpression},
            transform::Transform,
            HttpMethod, PaginationParameterLocation, ParseType, RequestBody, Select,
        },
        execution_plan::{
            capabilities::{ExecutionPlanFetch, ExecutionPlanPagination},
            discovery::{ExecutionPlanDiscoveryFields, ExecutionPlanDiscoveryStrategy},
            SourceExecutionPlan,
        },
        policy::{PolicySourceExecutionPlan, StrategyPolicy},
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
mod pagination;
mod strategy;
mod support;
mod values;

use acceptance::accept_discovery_result;
use diagnostics::{runtime_error, runtime_info, runtime_warning};
use document::{parse_response_document, select_items, select_sitemap_url_items};
use extract::extract_candidate;
use fetch::{fetch_strategy_document_at_url, fetch_strategy_document_with_query_params};
use pagination::execute_paginated_strategy;
use strategy::{execute_single_strategy_fetch, execute_strategy, extract_candidates_from_items};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryExecutionResult {
    pub candidates: Vec<DiscoveryCandidate>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCandidate {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub posting_meta: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<RequestBody>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryFetchError {
    pub message: String,
}

impl DiscoveryFetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait DiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
    >;
}

#[derive(Clone, Debug)]
pub struct ReqwestDiscoveryFetcher {
    client: reqwest::Client,
}

impl ReqwestDiscoveryFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestDiscoveryFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryFetcher for ReqwestDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
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
                                .map_err(|error| DiscoveryFetchError::new(error.to_string()))?,
                        )
                    }
                    RequestBody::Text { value } => builder.body(value.clone()),
                    RequestBody::Form { fields } => builder.form(fields),
                };
            }
            let response = builder
                .send()
                .await
                .map_err(|error| DiscoveryFetchError::new(error.to_string()))?
                .error_for_status()
                .map_err(|error| DiscoveryFetchError::new(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| DiscoveryFetchError::new(error.to_string()))?;
            Ok(DiscoveryFetchResponse { body })
        })
    }
}

pub async fn execute_discovery(plan: &SourceExecutionPlan) -> DiscoveryExecutionResult {
    execute_discovery_with_fetcher(plan, &ReqwestDiscoveryFetcher::new()).await
}

pub async fn execute_discovery_with_fetcher<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> DiscoveryExecutionResult
where
    F: DiscoveryFetcher + Sync + ?Sized,
{
    execute_discovery_with_clients(plan, fetcher, &UnavailableProfileBrowserClient).await
}

pub async fn execute_discovery_with_clients<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
) -> DiscoveryExecutionResult
where
    F: DiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    execute_discovery_with_clients_and_context(
        plan,
        fetcher,
        browser,
        RuntimeExecutionContext::uncancellable(),
    )
    .await
}

pub async fn execute_discovery_with_clients_and_context<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DiscoveryExecutionResult
where
    F: DiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if context.is_cancelled() {
        return cancelled_discovery_result("/postingDiscovery", None);
    }

    if plan.discovery.strategies.is_empty() {
        return DiscoveryExecutionResult {
            candidates: Vec::new(),
            diagnostics: vec![runtime_error(
                "posting_discovery_strategy_missing",
                "postingDiscovery does not contain an executable strategy",
                "/postingDiscovery/strategies",
                None,
                json!({}),
            )],
        };
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in plan.discovery.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            plan,
            fetcher,
            browser,
            strategy_index,
            strategy,
            plan.discovery.accept_when.as_ref(),
            context,
        )
        .await;
        if contains_runtime_execution_cancelled(&attempt.result.diagnostics)
            || context.is_cancelled()
        {
            diagnostics.extend(attempt.result.diagnostics);
            push_runtime_execution_cancelled(
                &mut diagnostics,
                format!("/postingDiscovery/strategies/{strategy_index}"),
                Some(&strategy.key),
            );
            return DiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            };
        }
        if attempt.accepted {
            diagnostics.extend(attempt.result.diagnostics);
            return DiscoveryExecutionResult {
                candidates: attempt.result.candidates,
                diagnostics,
            };
        }
        diagnostics.extend(attempt.result.diagnostics);
    }

    diagnostics.push(runtime_error(
        "fallback_exhausted",
        "postingDiscovery fallback strategies were exhausted without an accepted result",
        "/postingDiscovery/strategies",
        None,
        json!({}),
    ));
    DiscoveryExecutionResult {
        candidates: Vec::new(),
        diagnostics,
    }
}

pub async fn execute_policy_discovery_with_clients_and_context<F, B>(
    plan: &PolicySourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DiscoveryExecutionResult
where
    F: DiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    match plan.discovery.policy {
        StrategyPolicy::FirstAccepted => {
            execute_policy_first_accepted(plan, fetcher, browser, context).await
        }
    }
}

async fn execute_policy_first_accepted<F, B>(
    policy_plan: &PolicySourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DiscoveryExecutionResult
where
    F: DiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if context.is_cancelled() {
        return cancelled_discovery_result("/postingDiscovery", None);
    }
    let plan = policy_plan.execution_plan();
    if plan.discovery.strategies.is_empty() {
        return DiscoveryExecutionResult {
            candidates: Vec::new(),
            diagnostics: vec![runtime_error(
                "posting_discovery_strategy_missing",
                "postingDiscovery does not contain an executable strategy",
                "/postingDiscovery/strategies",
                None,
                json!({}),
            )],
        };
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in plan.discovery.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            &plan,
            fetcher,
            browser,
            strategy_index,
            strategy,
            plan.discovery.accept_when.as_ref(),
            context,
        )
        .await;
        if contains_runtime_execution_cancelled(&attempt.result.diagnostics)
            || context.is_cancelled()
        {
            diagnostics.extend(attempt.result.diagnostics);
            push_runtime_execution_cancelled(
                &mut diagnostics,
                format!("/postingDiscovery/strategies/{strategy_index}"),
                Some(&strategy.key),
            );
            return DiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            };
        }
        diagnostics.extend(attempt.result.diagnostics);
        if attempt.accepted {
            return DiscoveryExecutionResult {
                candidates: attempt.result.candidates,
                diagnostics,
            };
        }
    }

    diagnostics.push(runtime_error(
        "fallback_exhausted",
        "postingDiscovery fallback strategies were exhausted without an accepted result",
        "/postingDiscovery/strategies",
        None,
        json!({}),
    ));
    DiscoveryExecutionResult {
        candidates: Vec::new(),
        diagnostics,
    }
}

fn cancelled_discovery_result(path: &str, strategy_key: Option<&str>) -> DiscoveryExecutionResult {
    let mut diagnostics = Vec::new();
    push_runtime_execution_cancelled(&mut diagnostics, path, strategy_key);
    DiscoveryExecutionResult {
        candidates: Vec::new(),
        diagnostics,
    }
}
