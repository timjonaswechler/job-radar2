use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use dom_query::{Document as HtmlDocument, Matcher, NodeRef, Selection as HtmlSelection};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::{
            extract::{Cardinality, CombinePart, FieldExpression, ListFieldExpression},
            transform::Transform,
            HttpMethod, ParseType, Select,
        },
        execution_plan::{
            capabilities::ExecutionPlanFetch,
            posting_discovery::{
                ExecutionPlanPostingDiscoveryFields, ExecutionPlanPostingDiscoveryStrategy,
            },
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

use document::{parse_response_document, select_items};
use extract::extract_candidate;
use fetch::fetch_strategy_document;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDiscoveryExecutionResult {
    pub candidates: Vec<PostingDiscoveryCandidate>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingDiscoveryCandidate {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchResponse {
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryFetchError {
    pub message: String,
}

impl PostingDiscoveryFetchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait PostingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    >;
}

#[derive(Clone, Debug)]
pub struct ReqwestPostingDiscoveryFetcher {
    client: reqwest::Client,
}

impl ReqwestPostingDiscoveryFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestPostingDiscoveryFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PostingDiscoveryFetcher for ReqwestPostingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
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
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?
                .error_for_status()
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| PostingDiscoveryFetchError::new(error.to_string()))?;
            Ok(PostingDiscoveryFetchResponse { body })
        })
    }
}

pub async fn execute_posting_discovery(
    plan: &SourceExecutionPlan,
) -> PostingDiscoveryExecutionResult {
    execute_posting_discovery_with_fetcher(plan, &ReqwestPostingDiscoveryFetcher::new()).await
}

pub async fn execute_posting_discovery_with_fetcher<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    execute_posting_discovery_with_clients(plan, fetcher, &UnavailableProfileBrowserClient).await
}

pub async fn execute_posting_discovery_with_clients<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let Some((strategy_index, strategy)) =
        plan.posting_discovery.strategies.iter().enumerate().next()
    else {
        return PostingDiscoveryExecutionResult {
            candidates: Vec::new(),
            diagnostics: vec![runtime_error(
                "posting_discovery_strategy_missing",
                "postingDiscovery does not contain an executable strategy",
                "/postingDiscovery/strategies",
                None,
                json!({}),
            )],
        };
    };

    execute_strategy(plan, fetcher, browser, strategy_index, strategy).await
}

async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/postingDiscovery/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let response = match fetch_strategy_document(
        fetcher,
        browser,
        &strategy.fetch,
        &plan.source_config,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    )
    .await
    {
        Some(response) => response,
        None => {
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
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
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            }
        }
    };

    let items = match select_items(
        &document,
        &strategy.select,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(items) => items,
        None => {
            return PostingDiscoveryExecutionResult {
                candidates: Vec::new(),
                diagnostics,
            }
        }
    };

    let mut candidates = Vec::new();
    for (item_index, item) in items.into_iter().enumerate() {
        if let Some(candidate) = extract_candidate(
            &item,
            &strategy.extract.fields,
            &plan.source_config,
            &base_path,
            strategy_key.as_deref(),
            item_index,
            &mut diagnostics,
        ) {
            candidates.push(candidate);
        }
    }

    PostingDiscoveryExecutionResult {
        candidates,
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
