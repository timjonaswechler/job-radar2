use super::*;
use crate::profile_dsl::primitives::pagination::{
    CursorPaginationPlan, OffsetLimitPaginationPlan, PagePaginationPlan, SitemapPaginationPlan,
};

mod cursor;
mod offset_limit;
mod page;
mod sitemap;

pub(super) async fn execute_paginated_strategy<F>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &DiscoveryBrowserBackend<'_>,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    pagination: &CompiledPagination,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
    execution_failed: &mut bool,
    context: RuntimeExecutionContext<'_>,
) -> Result<Vec<PostingOccurrence>, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    match pagination {
        CompiledPagination::Page(pagination) => {
            page::execute(
                plan,
                source_config,
                fetcher,
                browser,
                strategy_index,
                strategy,
                pagination,
                base_path,
                strategy_key,
                diagnostics,
                execution_failed,
                context,
            )
            .await
        }
        CompiledPagination::OffsetLimit(pagination) => {
            offset_limit::execute(
                plan,
                source_config,
                fetcher,
                browser,
                strategy_index,
                strategy,
                pagination,
                base_path,
                strategy_key,
                diagnostics,
                execution_failed,
                context,
            )
            .await
        }
        CompiledPagination::Cursor(pagination) => {
            cursor::execute(
                plan,
                source_config,
                fetcher,
                browser,
                strategy_index,
                strategy,
                pagination,
                base_path,
                strategy_key,
                diagnostics,
                execution_failed,
                context,
            )
            .await
        }
        CompiledPagination::Sitemap(pagination) => {
            sitemap::execute(
                plan,
                source_config,
                fetcher,
                browser,
                strategy_index,
                strategy,
                pagination,
                base_path,
                strategy_key,
                diagnostics,
                execution_failed,
                context,
            )
            .await
        }
    }
}

fn ensure_not_cancelled(
    context: RuntimeExecutionContext<'_>,
    strategy_index: usize,
    strategy_key: Option<&str>,
) -> Result<(), TypedCancellation> {
    if context.is_cancelled() {
        Err(TypedCancellation::strategy(
            RuntimePhase::Discovery,
            strategy_index,
            strategy_key.expect("compiled strategy has a key"),
            CancellationOperation::Pagination,
        ))
    } else {
        Ok(())
    }
}

fn progression_overflow(
    diagnostics: &mut Diagnostics,
    base_path: &str,
    strategy_key: Option<&str>,
    pagination_type: &str,
) {
    diagnostics.push(runtime_warning(
        "pagination_progression_overflow",
        "Pagination stopped because the next progression value overflowed",
        format!("{base_path}/pagination"),
        strategy_key,
        json!({ "paginationType": pagination_type }),
    ));
}

fn append_candidates(
    candidates: &mut Vec<PostingOccurrence>,
    page: Vec<PostingOccurrence>,
    max_items: Option<u64>,
    pagination_type: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> bool {
    for candidate in page {
        candidates.push(candidate);
        if max_items.is_some_and(|max| candidates.len() as u64 == max) {
            diagnostics.push(runtime_warning(
                "pagination_max_items_reached",
                "Pagination stopped accumulating candidates after reaching maxItems",
                format!("{base_path}/pagination/limits/maxItems"),
                strategy_key,
                json!({ "maxItems": max_items, "paginationType": pagination_type }),
            ));
            return true;
        }
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
    total: Option<u64>,
    request_index: u64,
    page_size: Option<u64>,
    accumulated: usize,
) -> bool {
    let Some(total) = total else {
        return false;
    };
    if let Some(page_size) = page_size {
        request_index
            .checked_add(1)
            .and_then(|count| count.checked_mul(page_size))
            .is_none_or(|covered| covered >= total)
    } else {
        accumulated as u64 >= total
    }
}
