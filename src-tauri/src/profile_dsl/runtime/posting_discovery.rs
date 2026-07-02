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

use document::{parse_response_document, select_items, select_sitemap_url_items};
use extract::extract_candidate;
use fetch::{fetch_strategy_document_at_url, fetch_strategy_document_with_query_params};

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

#[derive(Clone, Debug, PartialEq)]
pub struct PostingDiscoveryFetchRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<RequestBody>,
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
            if let Some(body) = &request.body {
                builder =
                    match body {
                        RequestBody::Json { value } => {
                            if !request
                                .headers
                                .keys()
                                .any(|name| name.eq_ignore_ascii_case("content-type"))
                            {
                                builder = builder.header("content-type", "application/json");
                            }
                            builder.body(serde_json::to_string(value).map_err(|error| {
                                PostingDiscoveryFetchError::new(error.to_string())
                            })?)
                        }
                        RequestBody::Text { value } => builder.body(value.clone()),
                        RequestBody::Form { fields } => builder.form(fields),
                    };
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
    if plan.posting_discovery.strategies.is_empty() {
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
    }

    let mut diagnostics = Vec::new();
    for (strategy_index, strategy) in plan.posting_discovery.strategies.iter().enumerate() {
        let attempt = execute_strategy(
            plan,
            fetcher,
            browser,
            strategy_index,
            strategy,
            plan.posting_discovery.accept_when.as_ref(),
        )
        .await;
        if attempt.accepted {
            diagnostics.extend(attempt.result.diagnostics);
            return PostingDiscoveryExecutionResult {
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
    PostingDiscoveryExecutionResult {
        candidates: Vec::new(),
        diagnostics,
    }
}

struct PostingDiscoveryStrategyAttempt {
    result: PostingDiscoveryExecutionResult,
    accepted: bool,
}

async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    step_acceptance: Option<&Acceptance>,
) -> PostingDiscoveryStrategyAttempt
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/postingDiscovery/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    if let Some(pagination) = &strategy.pagination {
        let mut result = execute_paginated_strategy(
            plan,
            fetcher,
            browser,
            strategy,
            pagination,
            &base_path,
            strategy_key.as_deref(),
            diagnostics,
        )
        .await;
        let execution_failed = posting_discovery_execution_failed(&result);
        let accepted = !execution_failed
            && accept_posting_discovery_result(
                &result.candidates,
                step_acceptance,
                strategy.accept_when.as_ref(),
                &base_path,
                strategy_key.as_deref(),
                &mut result.diagnostics,
            );
        return PostingDiscoveryStrategyAttempt { result, accepted };
    }

    let output = execute_single_strategy_fetch(
        plan,
        fetcher,
        browser,
        strategy,
        &[],
        PaginationParameterLocation::Query,
        None,
        None,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    )
    .await;

    let mut result = PostingDiscoveryExecutionResult {
        candidates: output.candidates,
        diagnostics,
    };
    let execution_failed = posting_discovery_execution_failed(&result);
    let accepted = !execution_failed
        && accept_posting_discovery_result(
            &result.candidates,
            step_acceptance,
            strategy.accept_when.as_ref(),
            &base_path,
            strategy_key.as_deref(),
            &mut result.diagnostics,
        );
    PostingDiscoveryStrategyAttempt { result, accepted }
}

fn posting_discovery_execution_failed(result: &PostingDiscoveryExecutionResult) -> bool {
    result.candidates.is_empty()
        && result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

async fn execute_paginated_strategy<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    pagination: &ExecutionPlanPagination,
    base_path: &str,
    strategy_key: Option<&str>,
    mut diagnostics: Diagnostics,
) -> PostingDiscoveryExecutionResult
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    match pagination {
        ExecutionPlanPagination::Page {
            page_param,
            parameter_location,
            first_page,
            page_size_param,
            page_size,
            total_path,
            limits,
        } => {
            let max_requests = limits.max_requests.unwrap_or(1);
            let mut candidates = Vec::new();
            for request_index in 0..max_requests {
                let page = first_page.unwrap_or(1) + request_index;
                let mut pagination_params = vec![(page_param.as_str(), page.to_string())];
                if let (Some(page_size_param), Some(page_size)) = (page_size_param, page_size) {
                    pagination_params.push((page_size_param.as_str(), page_size.to_string()));
                }
                let page_output = execute_single_strategy_fetch(
                    plan,
                    fetcher,
                    browser,
                    strategy,
                    &pagination_params,
                    *parameter_location,
                    total_path.as_deref(),
                    None,
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                )
                .await;
                let page_candidates = page_output.candidates;
                if page_candidates.is_empty() {
                    break;
                }
                if append_page_candidates(
                    &mut candidates,
                    page_candidates,
                    limits.max_items,
                    "page",
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                ) {
                    break;
                }
                if page_total_exhausted(
                    page_output.total_count,
                    request_index,
                    *page_size,
                    candidates.len(),
                ) {
                    break;
                }
                if request_index + 1 == max_requests {
                    diagnostics.push(runtime_warning(
                        "pagination_max_requests_reached",
                        "Pagination stopped after reaching maxRequests",
                        format!("{base_path}/pagination/limits/maxRequests"),
                        strategy_key,
                        json!({ "maxRequests": max_requests, "paginationType": "page" }),
                    ));
                }
            }
            PostingDiscoveryExecutionResult {
                candidates,
                diagnostics,
            }
        }
        ExecutionPlanPagination::OffsetLimit {
            offset_param,
            limit_param,
            parameter_location,
            start_offset,
            limit,
            total_path,
            limits,
        } => {
            let max_requests = limits.max_requests.unwrap_or(1);
            let mut candidates = Vec::new();
            for request_index in 0..max_requests {
                let offset = start_offset.unwrap_or(0) + request_index * limit;
                let pagination_params = [
                    (offset_param.as_str(), offset.to_string()),
                    (limit_param.as_str(), limit.to_string()),
                ];
                let page_output = execute_single_strategy_fetch(
                    plan,
                    fetcher,
                    browser,
                    strategy,
                    &pagination_params,
                    *parameter_location,
                    total_path.as_deref(),
                    None,
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                )
                .await;
                let page_candidates = page_output.candidates;
                if page_candidates.is_empty() {
                    break;
                }
                if append_page_candidates(
                    &mut candidates,
                    page_candidates,
                    limits.max_items,
                    "offset_limit",
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                ) {
                    break;
                }
                if page_output
                    .total_count
                    .is_some_and(|total| offset.saturating_add(*limit) >= total)
                {
                    break;
                }
                if request_index + 1 == max_requests {
                    diagnostics.push(runtime_warning(
                        "pagination_max_requests_reached",
                        "Pagination stopped after reaching maxRequests",
                        format!("{base_path}/pagination/limits/maxRequests"),
                        strategy_key,
                        json!({ "maxRequests": max_requests, "paginationType": "offset_limit" }),
                    ));
                }
            }
            PostingDiscoveryExecutionResult {
                candidates,
                diagnostics,
            }
        }
        ExecutionPlanPagination::Cursor {
            cursor_param,
            parameter_location,
            next_cursor_path,
            limits,
        } => {
            let max_requests = limits.max_requests.unwrap_or(1);
            let mut candidates = Vec::new();
            let mut seen_cursors = HashSet::new();
            let mut cursor = None::<String>;

            for request_index in 0..max_requests {
                let pagination_params = cursor
                    .as_ref()
                    .map(|cursor| vec![(cursor_param.as_str(), cursor.clone())])
                    .unwrap_or_default();
                let page_output = execute_single_strategy_fetch(
                    plan,
                    fetcher,
                    browser,
                    strategy,
                    &pagination_params,
                    *parameter_location,
                    None,
                    Some(next_cursor_path.as_str()),
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                )
                .await;

                if append_page_candidates(
                    &mut candidates,
                    page_output.candidates,
                    limits.max_items,
                    "cursor",
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                ) {
                    break;
                }

                let Some(next_cursor) = page_output.next_cursor else {
                    break;
                };
                if !seen_cursors.insert(next_cursor.clone()) {
                    diagnostics.push(runtime_warning(
                        "pagination_duplicate_cursor",
                        "Cursor pagination stopped after detecting a duplicate cursor value",
                        format!("{base_path}/pagination/nextCursorPath"),
                        strategy_key,
                        json!({ "cursor": next_cursor, "paginationType": "cursor" }),
                    ));
                    break;
                }
                if request_index + 1 == max_requests {
                    diagnostics.push(runtime_warning(
                        "pagination_max_requests_reached",
                        "Pagination stopped after reaching maxRequests",
                        format!("{base_path}/pagination/limits/maxRequests"),
                        strategy_key,
                        json!({ "maxRequests": max_requests, "paginationType": "cursor" }),
                    ));
                    break;
                }
                cursor = Some(next_cursor);
            }

            PostingDiscoveryExecutionResult {
                candidates,
                diagnostics,
            }
        }
        ExecutionPlanPagination::Sitemap {
            child_sitemap_selector,
            posting_url_selector,
            limits,
        } => {
            let mut candidates = Vec::new();
            let mut queue = VecDeque::from([(None::<String>, 0_u64)]);
            let mut request_count = 0_u64;
            let max_requests = limits.max_requests.unwrap_or(1);
            let max_depth = limits.max_depth.unwrap_or(0);

            while let Some((url_override, depth)) = queue.pop_front() {
                if request_count >= max_requests {
                    diagnostics.push(runtime_warning(
                        "pagination_max_requests_reached",
                        "Sitemap pagination stopped after reaching maxRequests",
                        format!("{base_path}/pagination/limits/maxRequests"),
                        strategy_key,
                        json!({ "maxRequests": max_requests, "paginationType": "sitemap" }),
                    ));
                    break;
                }

                let response = match &url_override {
                    Some(url) => {
                        fetch_strategy_document_at_url(
                            fetcher,
                            browser,
                            &strategy.fetch,
                            &plan.source_config,
                            &plan.source.name,
                            url,
                            base_path,
                            strategy_key,
                            &mut diagnostics,
                        )
                        .await
                    }
                    None => {
                        fetch_strategy_document_with_query_params(
                            fetcher,
                            browser,
                            &strategy.fetch,
                            &plan.source_config,
                            &plan.source.name,
                            &[],
                            &[],
                            base_path,
                            strategy_key,
                            &mut diagnostics,
                        )
                        .await
                    }
                };
                let Some(response) = response else { break };
                request_count += 1;

                let document = match parse_response_document(
                    &response.body,
                    strategy,
                    base_path,
                    strategy_key,
                    &mut diagnostics,
                ) {
                    Some(document) => document,
                    None => break,
                };

                if let Some(items) = select_sitemap_url_items(
                    &document,
                    posting_url_selector.as_ref(),
                    &format!("{base_path}/pagination/postingUrlSelector"),
                    strategy_key,
                    &mut diagnostics,
                ) {
                    let page_candidates = extract_candidates_from_items(
                        plan,
                        strategy,
                        items,
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    );
                    if append_page_candidates(
                        &mut candidates,
                        page_candidates,
                        limits.max_items,
                        "sitemap",
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    ) {
                        break;
                    }
                }

                if child_sitemap_selector.is_some() {
                    if let Some(child_items) = select_sitemap_url_items(
                        &document,
                        child_sitemap_selector.as_ref(),
                        &format!("{base_path}/pagination/childSitemapSelector"),
                        strategy_key,
                        &mut diagnostics,
                    ) {
                        let child_urls = text_items_to_urls(child_items);
                        if depth < max_depth {
                            for child_url in child_urls {
                                queue.push_back((Some(child_url), depth + 1));
                            }
                        } else if !child_urls.is_empty() {
                            diagnostics.push(runtime_warning(
                                "pagination_max_depth_reached",
                                "Sitemap pagination did not follow child sitemap URLs because maxDepth was reached",
                                format!("{base_path}/pagination/limits/maxDepth"),
                                strategy_key,
                                json!({ "maxDepth": max_depth, "paginationType": "sitemap" }),
                            ));
                        }
                    }
                }
            }

            PostingDiscoveryExecutionResult {
                candidates,
                diagnostics,
            }
        }
    }
}

fn accept_posting_discovery_result(
    candidates: &[PostingDiscoveryCandidate],
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some((ratio, owner_path)) =
        first_max_error_ratio(step_acceptance, strategy_acceptance, base_path)
    {
        diagnostics.push(runtime_error(
            "acceptance_max_error_ratio_unsupported",
            "acceptWhen.maxErrorRatio is not supported by the postingDiscovery runtime result model yet",
            format!("{owner_path}/acceptWhen/maxErrorRatio"),
            strategy_key,
            json!({ "maxErrorRatio": ratio }),
        ));
        return false;
    }

    for (field, owner_path) in required_field_rules(step_acceptance, strategy_acceptance, base_path)
    {
        if let Some((item_index, _)) = candidates
            .iter()
            .enumerate()
            .find(|(_, candidate)| !posting_discovery_field_present(candidate, &field))
        {
            diagnostics.push(runtime_error(
                "acceptance_required_field_missing",
                format!(
                    "postingDiscovery candidate is missing required normalized field `{field}`"
                ),
                format!("{owner_path}/acceptWhen/requiredFields"),
                strategy_key,
                json!({ "field": field, "itemIndex": item_index }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_description_length),
        strategy_acceptance.and_then(|acceptance| acceptance.min_description_length),
        base_path,
    ) {
        if let Some((item_index, actual_length)) =
            candidates
                .iter()
                .enumerate()
                .find_map(|(item_index, candidate)| {
                    let description = candidate.description_text.as_ref()?;
                    let actual_length = description.chars().count() as u64;
                    (actual_length < minimum).then_some((item_index, actual_length))
                })
        {
            diagnostics.push(runtime_error(
                "acceptance_min_description_length_not_met",
                format!(
                    "postingDiscovery descriptionText is shorter than the configured minimum of {minimum} characters"
                ),
                format!("{owner_path}/acceptWhen/minDescriptionLength"),
                strategy_key,
                json!({
                    "minDescriptionLength": minimum,
                    "actualLength": actual_length,
                    "itemIndex": item_index,
                }),
            ));
            return false;
        }
    }

    if let Some((minimum, owner_path)) = stricter_u64_acceptance(
        step_acceptance.and_then(|acceptance| acceptance.min_results),
        strategy_acceptance.and_then(|acceptance| acceptance.min_results),
        base_path,
    ) {
        if candidates.len() < minimum as usize {
            diagnostics.push(runtime_error(
                "acceptance_min_results_not_met",
                format!(
                    "postingDiscovery returned fewer than the required minimum of {minimum} candidates"
                ),
                format!("{owner_path}/acceptWhen/minResults"),
                strategy_key,
                json!({
                    "minResults": minimum,
                    "actualResults": candidates.len(),
                }),
            ));
            return false;
        }
    }

    true
}

fn first_max_error_ratio(
    step_acceptance: Option<&Acceptance>,
    strategy_acceptance: Option<&Acceptance>,
    base_path: &str,
) -> Option<(f64, String)> {
    step_acceptance
        .and_then(|acceptance| acceptance.max_error_ratio)
        .map(|ratio| (ratio, "/postingDiscovery".to_string()))
        .or_else(|| {
            strategy_acceptance
                .and_then(|acceptance| acceptance.max_error_ratio)
                .map(|ratio| (ratio, base_path.to_string()))
        })
}

fn required_field_rules(
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
                .map(|field| (field.clone(), "/postingDiscovery".to_string())),
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

fn posting_discovery_field_present(candidate: &PostingDiscoveryCandidate, field: &str) -> bool {
    match field {
        "title" => !candidate.title.trim().is_empty(),
        "company" => !candidate.company.trim().is_empty(),
        "url" => !candidate.url.trim().is_empty(),
        "descriptionText" => candidate
            .description_text
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        "locations" => !candidate.locations.is_empty(),
        field => field
            .strip_prefix("postingMeta.")
            .and_then(|key| candidate.posting_meta.get(key))
            .is_some_and(|value| !value.trim().is_empty()),
    }
}

fn stricter_u64_acceptance(
    step_value: Option<u64>,
    strategy_value: Option<u64>,
    base_path: &str,
) -> Option<(u64, String)> {
    match (step_value, strategy_value) {
        (Some(step), Some(strategy)) if strategy >= step => Some((strategy, base_path.to_string())),
        (Some(step), Some(_)) | (Some(step), None) => Some((step, "/postingDiscovery".to_string())),
        (None, Some(strategy)) => Some((strategy, base_path.to_string())),
        (None, None) => None,
    }
}

fn append_page_candidates(
    candidates: &mut Vec<PostingDiscoveryCandidate>,
    page_candidates: Vec<PostingDiscoveryCandidate>,
    max_items: Option<u64>,
    pagination_type: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(max_items) = max_items else {
        candidates.extend(page_candidates);
        return false;
    };

    for candidate in page_candidates {
        if candidates.len() as u64 >= max_items {
            diagnostics.push(runtime_warning(
                "pagination_max_items_reached",
                "Pagination stopped accumulating candidates after reaching maxItems",
                format!("{base_path}/pagination/limits/maxItems"),
                strategy_key,
                json!({ "maxItems": max_items, "paginationType": pagination_type }),
            ));
            return true;
        }
        candidates.push(candidate);
    }

    false
}

fn text_items_to_urls(items: Vec<document::RuntimeItem<'_, '_>>) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|item| match item {
            document::RuntimeItem::Text(url) => Some(url),
            _ => None,
        })
        .collect()
}

struct StrategyFetchOutput {
    candidates: Vec<PostingDiscoveryCandidate>,
    total_count: Option<u64>,
    next_cursor: Option<String>,
}

fn query_params_for_location<'a>(
    params: &'a [(&'a str, String)],
    location: PaginationParameterLocation,
) -> &'a [(&'a str, String)] {
    match location {
        PaginationParameterLocation::Query => params,
        PaginationParameterLocation::JsonBody => &[],
    }
}

fn json_body_params_for_location<'a>(
    params: &'a [(&'a str, String)],
    location: PaginationParameterLocation,
) -> &'a [(&'a str, String)] {
    match location {
        PaginationParameterLocation::Query => &[],
        PaginationParameterLocation::JsonBody => params,
    }
}

async fn execute_single_strategy_fetch<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    pagination_params: &[(&str, String)],
    parameter_location: PaginationParameterLocation,
    total_path: Option<&str>,
    next_cursor_path: Option<&str>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> StrategyFetchOutput
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let response = match fetch_strategy_document_with_query_params(
        fetcher,
        browser,
        &strategy.fetch,
        &plan.source_config,
        &plan.source.name,
        query_params_for_location(pagination_params, parameter_location),
        json_body_params_for_location(pagination_params, parameter_location),
        base_path,
        strategy_key,
        diagnostics,
    )
    .await
    {
        Some(response) => response,
        None => {
            return StrategyFetchOutput {
                candidates: Vec::new(),
                total_count: None,
                next_cursor: None,
            }
        }
    };

    extract_candidates_from_response(
        plan,
        strategy,
        &response.body,
        total_path,
        next_cursor_path,
        base_path,
        strategy_key,
        diagnostics,
    )
}

fn extract_candidates_from_response(
    plan: &SourceExecutionPlan,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    body: &str,
    total_path: Option<&str>,
    next_cursor_path: Option<&str>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> StrategyFetchOutput {
    let document =
        match parse_response_document(body, strategy, base_path, strategy_key, diagnostics) {
            Some(document) => document,
            None => {
                return StrategyFetchOutput {
                    candidates: Vec::new(),
                    total_count: None,
                    next_cursor: None,
                }
            }
        };
    let total_count = total_path.and_then(|path| extract_total_count(&document, path));
    let next_cursor = next_cursor_path.and_then(|path| extract_next_cursor(&document, path));

    let items = match select_items(
        &document,
        &strategy.select,
        base_path,
        strategy_key,
        diagnostics,
    ) {
        Some(items) => items,
        None => {
            return StrategyFetchOutput {
                candidates: Vec::new(),
                total_count,
                next_cursor,
            }
        }
    };

    let candidates =
        extract_candidates_from_items(plan, strategy, items, base_path, strategy_key, diagnostics);
    StrategyFetchOutput {
        candidates,
        total_count,
        next_cursor,
    }
}

fn extract_candidates_from_items(
    plan: &SourceExecutionPlan,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    items: Vec<document::RuntimeItem<'_, '_>>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Vec<PostingDiscoveryCandidate> {
    let mut candidates = Vec::new();
    for (item_index, item) in items.into_iter().enumerate() {
        if let Some(candidate) = extract_candidate(
            &item,
            &strategy.extract.fields,
            &plan.source_config,
            &plan.source.name,
            base_path,
            strategy_key,
            item_index,
            diagnostics,
        ) {
            candidates.push(candidate);
        }
    }
    candidates
}

fn extract_total_count(document: &document::ParsedDocument<'_>, total_path: &str) -> Option<u64> {
    let document::ParsedDocument::Json(value) = document else {
        return None;
    };
    let value = resolve_simple_json_path(value, total_path).ok().flatten()?;
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => value.parse::<u64>().ok(),
        _ => None,
    }
}

fn extract_next_cursor(
    document: &document::ParsedDocument<'_>,
    next_cursor_path: &str,
) -> Option<String> {
    let document::ParsedDocument::Json(value) = document else {
        return None;
    };
    let value = resolve_simple_json_path(value, next_cursor_path)
        .ok()
        .flatten()?;
    match value {
        Value::String(value) => non_empty_cursor(value),
        Value::Number(number) => non_empty_cursor(&number.to_string()),
        _ => None,
    }
}

fn non_empty_cursor(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn page_total_exhausted(
    total_count: Option<u64>,
    request_index: u64,
    page_size: Option<u64>,
    accumulated_candidates: usize,
) -> bool {
    let Some(total_count) = total_count else {
        return false;
    };
    if let Some(page_size) = page_size {
        request_index.saturating_add(1).saturating_mul(page_size) >= total_count
    } else {
        accumulated_candidates as u64 >= total_count
    }
}

fn runtime_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    runtime_diagnostic(
        code,
        message,
        DiagnosticSeverity::Error,
        path,
        strategy_key,
        details,
    )
}

fn runtime_warning(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    runtime_diagnostic(
        code,
        message,
        DiagnosticSeverity::Warning,
        path,
        strategy_key,
        details,
    )
}

fn runtime_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    severity: DiagnosticSeverity,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(details),
    }
}
