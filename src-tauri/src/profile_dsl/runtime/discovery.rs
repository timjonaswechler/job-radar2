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
    },
    simple_json_path::resolve_simple_json_path,
    source::documents::SourceConfig,
};

use super::{
    browser::{
        ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
        ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    },
    cancellation::{
        runtime_execution_cancelled_diagnostic, CancellationOperation, RuntimeExecutionContext,
        RuntimePhase, TypedCancellation,
    },
    strategy_set::{
        execute_first_accepted, StrategyAttemptCompletion, StrategyExecution, StrategySetTerminal,
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

pub async fn execute_discovery<F, B>(
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
        return cancelled_discovery_result(TypedCancellation::phase(RuntimePhase::Discovery));
    }

    if plan.discovery.strategies.is_empty() {
        return DiscoveryExecutionResult {
            candidates: Vec::new(),
            diagnostics: vec![runtime_error(
                "discovery_strategy_missing",
                "discovery does not contain an executable strategy",
                "/discovery/strategies",
                None,
                json!({}),
            )],
        };
    }

    let execution = execute_first_accepted(
        &plan.discovery.strategies,
        |strategy| strategy.key.as_str(),
        |strategy_index, strategy| {
            context.is_cancelled().then(|| {
                TypedCancellation::strategy(
                    RuntimePhase::Discovery,
                    strategy_index,
                    &strategy.key,
                    CancellationOperation::Phase,
                )
            })
        },
        |strategy_index, strategy| {
            Box::pin(async move {
                let mut execution = execute_strategy(
                    plan,
                    fetcher,
                    browser,
                    strategy_index,
                    strategy,
                    plan.discovery.accept_when.as_ref(),
                    context,
                )
                .await;
                if context.is_cancelled()
                    && !matches!(
                        execution.completion,
                        StrategyAttemptCompletion::Cancelled(_)
                    )
                {
                    execution.completion =
                        StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
                            RuntimePhase::Discovery,
                            strategy_index,
                            &strategy.key,
                            CancellationOperation::Phase,
                        ));
                }
                execution
            })
        },
    )
    .await;
    project_discovery_execution(execution)
}

fn project_discovery_execution(
    execution: super::strategy_set::StrategySetExecution<Vec<DiscoveryCandidate>>,
) -> DiscoveryExecutionResult {
    let accepted_attempt = match execution.terminal {
        StrategySetTerminal::Accepted { attempt_index } => Some(attempt_index),
        StrategySetTerminal::Cancelled(cancellation) => {
            let mut diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .collect::<Diagnostics>();
            diagnostics.push(runtime_execution_cancelled_diagnostic(&cancellation));
            return DiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            };
        }
        StrategySetTerminal::Exhausted => None,
    };

    let mut diagnostics = Vec::new();
    let mut candidates = Vec::new();
    for (attempt_index, attempt) in execution.attempts.into_iter().enumerate() {
        debug_assert_eq!(attempt.strategy_index, attempt_index);
        debug_assert!(!attempt.strategy_key.is_empty());
        diagnostics.extend(attempt.diagnostics);
        if Some(attempt_index) == accepted_attempt {
            let StrategyAttemptCompletion::Accepted(output) = attempt.completion else {
                unreachable!("accepted terminal must reference accepted typed output");
            };
            candidates = output;
        }
    }

    if accepted_attempt.is_none() {
        diagnostics.push(runtime_error(
            "fallback_exhausted",
            "discovery fallback strategies were exhausted without an accepted result",
            "/discovery/strategies",
            None,
            json!({}),
        ));
    }
    DiscoveryExecutionResult {
        candidates,
        diagnostics,
    }
}

fn cancelled_discovery_result(cancellation: TypedCancellation) -> DiscoveryExecutionResult {
    DiscoveryExecutionResult {
        candidates: Vec::new(),
        diagnostics: vec![runtime_execution_cancelled_diagnostic(&cancellation)],
    }
}
