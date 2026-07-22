use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    pagination: &OffsetLimitPaginationPlan,
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
    let mut state = pagination.initial_state();
    let mut highest_total = None;
    loop {
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        let offset = pagination.current_offset(&state);
        let output = execute_single_strategy_fetch(
            plan,
            source_config,
            fetcher,
            browser,
            strategy_index,
            strategy,
            &pagination.overlay(&state),
            pagination.total_path.as_ref(),
            None,
            base_path,
            strategy_key,
            diagnostics,
            execution_failed,
            context,
        )
        .await?;
        ensure_not_cancelled(context, strategy_index, strategy_key)?;
        if output.candidates.is_empty() {
            break;
        }
        if append_candidates(
            &mut candidates,
            output.candidates,
            pagination.limits.max_items,
            "offset_limit",
            base_path,
            strategy_key,
            diagnostics,
        ) {
            break;
        }
        highest_total = highest_total.max(output.total_count);
        if highest_total.is_some_and(|total| {
            offset
                .checked_add(pagination.limit)
                .is_some_and(|end| end >= total)
        }) {
            break;
        }
        if !pagination.advance(&mut state) {
            progression_overflow(diagnostics, base_path, strategy_key, "offset_limit");
            break;
        }
    }
    Ok(candidates)
}
