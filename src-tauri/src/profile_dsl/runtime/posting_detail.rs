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

mod document;
mod extract;
mod fetch;
mod support;
mod values;

use document::{parse_response_document, select_detail_document};
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
