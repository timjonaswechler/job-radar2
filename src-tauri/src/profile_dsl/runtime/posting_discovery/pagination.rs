use super::*;

pub(super) async fn execute_paginated_strategy<F, B>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
    browser: &B,
    strategy: &ExecutionPlanPostingDiscoveryStrategy,
    pagination: &ExecutionPlanPagination,
    base_path: &str,
    strategy_key: Option<&str>,
    mut diagnostics: Diagnostics,
    context: RuntimeExecutionContext<'_>,
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
            let configured_max_requests = limits.max_requests.unwrap_or(1);
            let (max_requests, constrained_by_execution_budget) =
                context.posting_discovery_request_limit(configured_max_requests);
            let mut candidates = Vec::new();
            for request_index in 0..max_requests {
                if stop_pagination_if_cancelled(context, base_path, strategy_key, &mut diagnostics)
                {
                    break;
                }
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
                    context,
                )
                .await;
                if contains_runtime_execution_cancelled(&diagnostics)
                    || stop_pagination_if_cancelled(
                        context,
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    )
                {
                    break;
                }
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
                    push_request_limit_diagnostic(
                        &mut diagnostics,
                        base_path,
                        strategy_key,
                        "page",
                        configured_max_requests,
                        max_requests,
                        constrained_by_execution_budget,
                    );
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
            let configured_max_requests = limits.max_requests.unwrap_or(1);
            let (max_requests, constrained_by_execution_budget) =
                context.posting_discovery_request_limit(configured_max_requests);
            let mut candidates = Vec::new();
            let mut highest_total_count = None;
            for request_index in 0..max_requests {
                if stop_pagination_if_cancelled(context, base_path, strategy_key, &mut diagnostics)
                {
                    break;
                }
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
                    context,
                )
                .await;
                if contains_runtime_execution_cancelled(&diagnostics)
                    || stop_pagination_if_cancelled(
                        context,
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    )
                {
                    break;
                }
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
                highest_total_count = highest_total_count.max(page_output.total_count);
                if highest_total_count.is_some_and(|total| offset.saturating_add(*limit) >= total) {
                    break;
                }
                if request_index + 1 == max_requests {
                    push_request_limit_diagnostic(
                        &mut diagnostics,
                        base_path,
                        strategy_key,
                        "offset_limit",
                        configured_max_requests,
                        max_requests,
                        constrained_by_execution_budget,
                    );
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
            let configured_max_requests = limits.max_requests.unwrap_or(1);
            let (max_requests, constrained_by_execution_budget) =
                context.posting_discovery_request_limit(configured_max_requests);
            let mut candidates = Vec::new();
            let mut seen_cursors = HashSet::new();
            let mut cursor = None::<String>;

            for request_index in 0..max_requests {
                if stop_pagination_if_cancelled(context, base_path, strategy_key, &mut diagnostics)
                {
                    break;
                }
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
                    context,
                )
                .await;
                if contains_runtime_execution_cancelled(&diagnostics)
                    || stop_pagination_if_cancelled(
                        context,
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    )
                {
                    break;
                }

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
                    push_request_limit_diagnostic(
                        &mut diagnostics,
                        base_path,
                        strategy_key,
                        "cursor",
                        configured_max_requests,
                        max_requests,
                        constrained_by_execution_budget,
                    );
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
            let configured_max_requests = limits.max_requests.unwrap_or(1);
            let (max_requests, constrained_by_execution_budget) =
                context.posting_discovery_request_limit(configured_max_requests);
            let max_depth = limits.max_depth.unwrap_or(0);

            while let Some((url_override, depth)) = queue.pop_front() {
                if stop_pagination_if_cancelled(context, base_path, strategy_key, &mut diagnostics)
                {
                    break;
                }
                if request_count >= max_requests {
                    push_request_limit_diagnostic(
                        &mut diagnostics,
                        base_path,
                        strategy_key,
                        "sitemap",
                        configured_max_requests,
                        max_requests,
                        constrained_by_execution_budget,
                    );
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
                            context,
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
                            context,
                        )
                        .await
                    }
                };
                if contains_runtime_execution_cancelled(&diagnostics)
                    || stop_pagination_if_cancelled(
                        context,
                        base_path,
                        strategy_key,
                        &mut diagnostics,
                    )
                {
                    break;
                }
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

fn push_request_limit_diagnostic(
    diagnostics: &mut Diagnostics,
    base_path: &str,
    strategy_key: Option<&str>,
    pagination_type: &str,
    configured_max_requests: u64,
    effective_max_requests: u64,
    constrained_by_execution_budget: bool,
) {
    if constrained_by_execution_budget {
        diagnostics.push(runtime_info(
            "posting_discovery_request_budget_reached",
            "Posting discovery stopped at the caller execution budget",
            format!("{base_path}/executionBudget/maxRequestsPerStrategy"),
            strategy_key,
            json!({
                "configuredMaxRequests": configured_max_requests,
                "effectiveMaxRequests": effective_max_requests,
                "paginationType": pagination_type
            }),
        ));
    } else {
        diagnostics.push(runtime_warning(
            "pagination_max_requests_reached",
            "Pagination stopped after reaching maxRequests",
            format!("{base_path}/pagination/limits/maxRequests"),
            strategy_key,
            json!({ "maxRequests": effective_max_requests, "paginationType": pagination_type }),
        ));
    }
}

fn stop_pagination_if_cancelled(
    context: RuntimeExecutionContext<'_>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    if !context.is_cancelled() {
        return false;
    }
    push_runtime_execution_cancelled(diagnostics, format!("{base_path}/pagination"), strategy_key);
    true
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
