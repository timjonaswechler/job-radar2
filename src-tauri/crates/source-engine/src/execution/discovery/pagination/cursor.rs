use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute<F>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_>,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    pagination: &CursorPaginationPlan,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
    execution_failed: &mut bool,
    context: RuntimeExecutionContext<'_>,
) -> Result<Vec<PostingOccurrence>, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    let context = context.with_pagination_limit(pagination.limits.max_requests);
    let mut candidates = Vec::new();
    let mut state = pagination.initial_state();
    loop {
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        let output = execute_single_strategy_fetch(
            plan,
            source_config,
            fetcher,
            browser,
            strategy_index,
            strategy,
            &pagination.overlay(&state),
            None,
            Some(&pagination.next_cursor_path),
            base_path,
            strategy_key,
            diagnostics,
            execution_failed,
            context,
        )
        .await?;
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        if append_candidates(
            &mut candidates,
            output.candidates,
            pagination.limits.max_items,
            "cursor",
            base_path,
            strategy_key,
            diagnostics,
        ) {
            break;
        }
        match pagination.advance(&mut state, output.next_cursor) {
            CursorAdvance::Advanced => {}
            CursorAdvance::MissingOrEmpty => break,
            CursorAdvance::Repeated => {
                diagnostics.push(runtime_warning(
                    "pagination_duplicate_cursor",
                    "Cursor pagination stopped after detecting a duplicate cursor value",
                    format!("{base_path}/pagination/nextCursorPath"),
                    strategy_key,
                    json!({ "paginationType": "cursor" }),
                ));
                break;
            }
        }
    }
    Ok(candidates)
}
