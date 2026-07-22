use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_, B>,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    pagination: &SitemapPaginationPlan,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
    execution_failed: &mut bool,
    context: RuntimeExecutionContext<'_>,
) -> Result<Vec<PostingOccurrence>, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let context = context.with_pagination_limit(pagination.limits.max_requests);
    let mut candidates = Vec::new();
    let mut queue = VecDeque::from([(None::<String>, 0_u64)]);
    let mut seen = HashSet::new();
    while let Some((url, depth)) = queue.pop_front() {
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        let page_context = context.with_page_request(true);
        let response = match url.as_deref() {
            Some(url) => {
                fetch_strategy_document_at_url(
                    fetcher,
                    browser,
                    &strategy.fetch,
                    strategy.parse.authored_charset(),
                    source_config,
                    &plan.source.name,
                    url,
                    base_path,
                    strategy_key,
                    strategy_index,
                    diagnostics,
                    page_context,
                )
                .await?
            }
            None => {
                fetch_strategy_document_with_overlay(
                    fetcher,
                    browser,
                    &strategy.fetch,
                    strategy.parse.authored_charset(),
                    source_config,
                    &plan.source.name,
                    &PaginationOverlay::default(),
                    base_path,
                    strategy_key,
                    strategy_index,
                    diagnostics,
                    page_context,
                )
                .await?
            }
        };
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        let response = match response {
            DiscoveryFetchOutcome::Complete(response) => response,
            DiscoveryFetchOutcome::ExecutionFailed => {
                *execution_failed = true;
                break;
            }
        };
        let document = match strategy.parse.parse_with_diagnostics(
            response.as_input(),
            ParseDiagnosticContext {
                base_path,
                strategy_key,
            },
            diagnostics,
        ) {
            Some(document) => document,
            None => {
                *execution_failed = true;
                break;
            }
        };
        let Some(items) = document::select_items_raw(
            &document,
            &pagination.posting_url_selector,
            &format!("{base_path}/pagination/postingUrlSelector"),
            strategy_key,
            diagnostics,
        ) else {
            *execution_failed = true;
            break;
        };
        let page_candidates = extract_candidates_from_items(
            plan,
            source_config,
            strategy,
            items,
            base_path,
            strategy_key,
            diagnostics,
        );
        if append_candidates(
            &mut candidates,
            page_candidates,
            pagination.limits.max_items,
            "sitemap",
            base_path,
            strategy_key,
            diagnostics,
        ) {
            break;
        }
        if let Some(selector) = &pagination.child_sitemap_selector {
            let Some(items) = document::select_items_raw(
                &document,
                selector,
                &format!("{base_path}/pagination/childSitemapSelector"),
                strategy_key,
                diagnostics,
            ) else {
                *execution_failed = true;
                break;
            };
            let urls = text_items_to_urls(items);
            let max_depth = pagination.limits.max_depth.unwrap_or(0);
            if depth < max_depth {
                for child in urls {
                    ensure_not_cancelled(context, strategy_index, strategy_key)?;
                    if !seen.insert(child.clone()) {
                        continue;
                    }
                    if context
                        .debit(AllowanceCharge {
                            fan_out: 1,
                            ..AllowanceCharge::default()
                        })
                        .is_err()
                    {
                        queue.clear();
                        break;
                    }
                    queue.push_back((Some(child), depth + 1));
                }
            } else if !urls.is_empty() {
                diagnostics.push(runtime_warning(
                    "pagination_max_depth_reached",
                    "Sitemap pagination did not follow child sitemap URLs because maxDepth was reached",
                    format!("{base_path}/pagination/limits/maxDepth"), strategy_key,
                    json!({ "maxDepth": max_depth, "paginationType": "sitemap" }),
                ));
            }
        }
    }
    Ok(candidates)
}
