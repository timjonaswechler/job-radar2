//! Compiler boundedness checks for declarative Profile DSL plans.
//!
//! These checks intentionally inspect only declared plan shape. They do not
//! execute network, browser, parser, selector, extractor, transform,
//! pagination, or runtime behavior.

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::fetch::{BrowserInteraction, BrowserWait};
use crate::profile_dsl::documents::{
    Fetch, Pagination, PaginationLimits, PostingDetailStep, PostingDiscoveryStep,
};

use super::compiler_error;

const MAX_FALLBACK_STRATEGIES: usize = 50;

pub(super) fn validate_boundedness(
    posting_discovery: &PostingDiscoveryStep,
    posting_detail: Option<&PostingDetailStep>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    validate_discovery_strategy_list(posting_discovery, &base_path, diagnostics);

    for (index, strategy) in posting_discovery.strategies.iter().enumerate() {
        let strategy_path = format!("{base_path}/postingDiscovery/strategies/{index}");
        validate_fetch_bounds(
            &strategy.fetch,
            &format!("{strategy_path}/fetch"),
            &strategy.key,
            diagnostics,
        );
        if let Some(pagination) = &strategy.pagination {
            validate_pagination_bounds(
                pagination,
                &format!("{strategy_path}/pagination"),
                &strategy.key,
                diagnostics,
            );
        }
    }

    if let Some(posting_detail) = posting_detail {
        validate_detail_strategy_list(posting_detail, &base_path, diagnostics);
        for (index, strategy) in posting_detail.strategies.iter().enumerate() {
            let strategy_path = format!("{base_path}/postingDetail/strategies/{index}");
            validate_fetch_bounds(
                &strategy.fetch,
                &format!("{strategy_path}/fetch"),
                &strategy.key,
                diagnostics,
            );
        }
    }
}

fn validate_discovery_strategy_list(
    posting_discovery: &PostingDiscoveryStep,
    base_path: &str,
    diagnostics: &mut Diagnostics,
) {
    validate_strategy_list_len(
        posting_discovery.strategies.len(),
        &format!("{base_path}/postingDiscovery/strategies"),
        "postingDiscovery",
        diagnostics,
    );
}

fn validate_detail_strategy_list(
    posting_detail: &PostingDetailStep,
    base_path: &str,
    diagnostics: &mut Diagnostics,
) {
    validate_strategy_list_len(
        posting_detail.strategies.len(),
        &format!("{base_path}/postingDetail/strategies"),
        "postingDetail",
        diagnostics,
    );
}

fn validate_strategy_list_len(len: usize, path: &str, step: &str, diagnostics: &mut Diagnostics) {
    if len == 0 {
        diagnostics.push(compiler_error(
            "empty_fallback_strategy_list",
            format!("{step} must declare a finite, ordered, non-empty Strategy list"),
            path,
            serde_json::json!({ "step": step }),
        ));
    } else if len > MAX_FALLBACK_STRATEGIES {
        diagnostics.push(compiler_error(
            "fallback_strategy_list_exceeds_limit",
            format!(
                "{step} declares {len} fallback Strategies, exceeding the compiler limit of {MAX_FALLBACK_STRATEGIES}"
            ),
            path,
            serde_json::json!({
                "step": step,
                "strategyCount": len,
                "maxStrategyCount": MAX_FALLBACK_STRATEGIES,
            }),
        ));
    }
}

fn validate_fetch_bounds(
    fetch: &Fetch,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    match fetch {
        Fetch::Http {
            timeout_ms, retry, ..
        } => {
            validate_timeout(
                *timeout_ms,
                "missing_fetch_timeout",
                "HTTP fetch must declare an explicit timeoutMs bound",
                &format!("{path}/timeoutMs"),
                strategy_key,
                diagnostics,
            );
            if let Some(retry) = retry {
                if retry
                    .max_attempts
                    .filter(|attempts| *attempts > 0)
                    .is_none()
                {
                    push_bounded_diagnostic(
                        diagnostics,
                        "unbounded_fetch_retry",
                        "HTTP fetch retry behavior must declare a positive maxAttempts bound"
                            .to_string(),
                        &format!("{path}/retry/maxAttempts"),
                        strategy_key,
                        serde_json::json!({ "bound": "maxAttempts" }),
                    );
                }
            }
        }
        Fetch::Browser {
            timeout_ms,
            waits,
            interactions,
            ..
        } => {
            validate_timeout(
                *timeout_ms,
                "missing_fetch_timeout",
                "Browser fetch must declare an explicit timeoutMs bound",
                &format!("{path}/timeoutMs"),
                strategy_key,
                diagnostics,
            );
            if let Some(waits) = waits {
                for (index, wait) in waits.iter().enumerate() {
                    validate_browser_wait_bounds(
                        wait,
                        &format!("{path}/waits/{index}"),
                        strategy_key,
                        diagnostics,
                    );
                }
            }
            if let Some(interactions) = interactions {
                for (index, interaction) in interactions.iter().enumerate() {
                    validate_browser_interaction_bounds(
                        interaction,
                        &format!("{path}/interactions/{index}"),
                        strategy_key,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn validate_timeout(
    timeout_ms: Option<u64>,
    code: &str,
    message: &str,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    if timeout_ms.filter(|timeout| *timeout > 0).is_none() {
        push_bounded_diagnostic(
            diagnostics,
            code,
            message.to_string(),
            path,
            strategy_key,
            serde_json::json!({ "bound": "timeoutMs" }),
        );
    }
}

fn validate_browser_wait_bounds(
    wait: &BrowserWait,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let timeout_ms = match wait {
        BrowserWait::Selector { timeout_ms, .. } | BrowserWait::NetworkIdle { timeout_ms, .. } => {
            *timeout_ms
        }
    };
    validate_timeout(
        timeout_ms,
        "unbounded_browser_wait",
        "Browser wait must declare an explicit timeoutMs bound",
        &format!("{path}/timeoutMs"),
        strategy_key,
        diagnostics,
    );
}

fn validate_browser_interaction_bounds(
    interaction: &BrowserInteraction,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let max_count = match interaction {
        BrowserInteraction::ClickIfVisible { max_count, .. }
        | BrowserInteraction::ClickUntilGone { max_count, .. } => *max_count,
        BrowserInteraction::ExecuteScript { .. }
        | BrowserInteraction::Eval { .. }
        | BrowserInteraction::MutateDom { .. }
        | BrowserInteraction::LoginFlow { .. }
        | BrowserInteraction::CaptchaBypass { .. } => return,
    };

    if max_count.filter(|count| *count > 0).is_none() {
        push_bounded_diagnostic(
            diagnostics,
            "unbounded_browser_interaction",
            "Browser interaction must declare a positive maxCount bound".to_string(),
            &format!("{path}/maxCount"),
            strategy_key,
            serde_json::json!({ "bound": "maxCount" }),
        );
    }
}

fn validate_pagination_bounds(
    pagination: &Pagination,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let (limits, pagination_type, requires_depth_bound) = match pagination {
        Pagination::Page { limits, .. } => (limits.as_ref(), "page", false),
        Pagination::OffsetLimit { limits, .. } => (limits.as_ref(), "offset_limit", false),
        Pagination::Cursor { limits, .. } => (limits.as_ref(), "cursor", false),
        Pagination::Sitemap {
            limits,
            child_sitemap_selector,
            ..
        } => (limits.as_ref(), "sitemap", child_sitemap_selector.is_some()),
    };

    let Some(limits) = limits else {
        push_bounded_diagnostic(
            diagnostics,
            "unbounded_pagination",
            "Pagination must declare explicit stop limits".to_string(),
            &format!("{path}/limits"),
            strategy_key,
            serde_json::json!({ "paginationType": pagination_type }),
        );
        return;
    };

    validate_pagination_limits(
        limits,
        path,
        pagination_type,
        requires_depth_bound,
        strategy_key,
        diagnostics,
    );
}

fn validate_pagination_limits(
    limits: &PaginationLimits,
    path: &str,
    pagination_type: &str,
    requires_depth_bound: bool,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let has_request_bound = limits.max_requests.filter(|limit| *limit > 0).is_some();
    let has_item_bound = limits.max_items.filter(|limit| *limit > 0).is_some();
    let has_depth_bound = limits.max_depth.is_some();

    if !(has_request_bound || has_item_bound || has_depth_bound) {
        push_bounded_diagnostic(
            diagnostics,
            "unbounded_pagination",
            "Pagination limits must include at least one positive stop rule such as maxRequests, maxItems, or maxDepth".to_string(),
            &format!("{path}/limits"),
            strategy_key,
            serde_json::json!({ "paginationType": pagination_type }),
        );
    }

    if requires_depth_bound && !has_depth_bound {
        push_bounded_diagnostic(
            diagnostics,
            "unbounded_pagination_depth",
            "Recursive sitemap pagination must declare maxDepth".to_string(),
            &format!("{path}/limits/maxDepth"),
            strategy_key,
            serde_json::json!({ "paginationType": pagination_type }),
        );
    }
}

fn push_bounded_diagnostic(
    diagnostics: &mut Diagnostics,
    code: &str,
    message: String,
    path: &str,
    strategy_key: &str,
    details: serde_json::Value,
) {
    let mut diagnostic = compiler_error(code, message, path, details);
    diagnostic.strategy_key = Some(strategy_key.to_string());
    diagnostics.push(diagnostic);
}
