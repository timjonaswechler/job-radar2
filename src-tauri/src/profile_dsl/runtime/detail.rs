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

pub async fn execute_detail<F, B>(
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
        return cancelled_detail_result(TypedCancellation::phase(RuntimePhase::Detail));
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

    let execution = execute_first_accepted(
        &detail.strategies,
        |strategy| strategy.key.as_str(),
        |strategy_index, strategy| {
            context.is_cancelled().then(|| {
                TypedCancellation::strategy(
                    RuntimePhase::Detail,
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
                    posting,
                    fetcher,
                    browser,
                    strategy_index,
                    strategy,
                    detail.accept_when.as_ref(),
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
                            RuntimePhase::Detail,
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
    project_detail_execution(execution)
}

fn project_detail_execution(
    execution: super::strategy_set::StrategySetExecution<String>,
) -> DetailExecutionResult {
    let accepted_attempt = match execution.terminal {
        StrategySetTerminal::Accepted { attempt_index } => Some(attempt_index),
        StrategySetTerminal::Cancelled(cancellation) => {
            let mut diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .collect::<Diagnostics>();
            diagnostics.push(runtime_execution_cancelled_diagnostic(&cancellation));
            return DetailExecutionResult {
                description_text: None,
                diagnostics,
            };
        }
        StrategySetTerminal::Exhausted => None,
    };

    let mut diagnostics = Vec::new();
    let mut description_text = None;
    for (attempt_index, attempt) in execution.attempts.into_iter().enumerate() {
        debug_assert_eq!(attempt.strategy_index, attempt_index);
        debug_assert!(!attempt.strategy_key.is_empty());
        diagnostics.extend(attempt.diagnostics);
        if Some(attempt_index) == accepted_attempt {
            let StrategyAttemptCompletion::Accepted(output) = attempt.completion else {
                unreachable!("accepted terminal must reference accepted typed output");
            };
            description_text = Some(output);
        }
    }

    if accepted_attempt.is_none() {
        diagnostics.push(runtime_error(
            "fallback_exhausted",
            "detail fallback strategies were exhausted without an accepted result",
            "/detail/strategies",
            None,
            json!({}),
        ));
    }
    DetailExecutionResult {
        description_text,
        diagnostics,
    }
}

fn cancelled_detail_result(cancellation: TypedCancellation) -> DetailExecutionResult {
    DetailExecutionResult {
        description_text: None,
        diagnostics: vec![runtime_execution_cancelled_diagnostic(&cancellation)],
    }
}
