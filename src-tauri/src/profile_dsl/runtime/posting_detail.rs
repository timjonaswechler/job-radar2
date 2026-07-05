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

mod acceptance;
mod diagnostics;
mod document;
mod extract;
mod fetch;
mod strategy;
mod support;
mod values;

use acceptance::accept_posting_detail_result;
use diagnostics::runtime_error;
use document::{parse_response_document, select_detail_document, RuntimeItem};
use extract::{evaluate_strategy_captures, evaluate_string_field};
use fetch::fetch_strategy_document;
use strategy::execute_strategy;

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
