//! Compiler boundedness checks for declarative Profile DSL plans.
//!
//! These checks intentionally inspect only declared plan shape. They do not
//! execute network, browser, parser, selector, extractor, transform,
//! pagination, or runtime behavior.

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::fetch::{
    BrowserInteraction, BrowserWait, MAX_BROWSER_FETCH_TIMEOUT_MS, MAX_BROWSER_INTERACTION_COUNT,
    MAX_BROWSER_WAIT_AFTER_MS, MAX_BROWSER_WAIT_TIMEOUT_MS,
};
use crate::profile_dsl::documents::{DetailStep, DiscoveryStep, Fetch, PhaseLimits};
use crate::profile_dsl::policy::StrategyPolicy;

use super::compiler_error;

pub(crate) const MAX_FALLBACK_STRATEGIES: usize = 50;

pub(super) fn validate_boundedness(
    discovery: &DiscoveryStep,
    detail: Option<&DetailStep>,
    base_path: String,
    diagnostics: &mut Diagnostics,
) {
    validate_discovery_strategy_list(discovery, &base_path, diagnostics);
    validate_policy_cardinality(
        discovery.policy,
        discovery.strategies.len(),
        &format!("{base_path}/discovery/policy"),
        diagnostics,
    );
    validate_phase_limits(
        discovery.limits,
        &format!("{base_path}/discovery/limits"),
        diagnostics,
        discovery
            .strategies
            .iter()
            .any(|strategy| matches!(strategy.fetch, Fetch::Browser { .. })),
    );

    let discovery_limits = discovery.limits.unwrap_or(PhaseLimits::BACKEND);
    for (index, strategy) in discovery.strategies.iter().enumerate() {
        let strategy_path = format!("{base_path}/discovery/strategies/{index}");
        validate_fetch_bounds(
            &strategy.fetch,
            discovery_limits,
            &format!("{strategy_path}/fetch"),
            &strategy.key,
            diagnostics,
        );
    }

    if let Some(detail) = detail {
        validate_detail_strategy_list(detail, &base_path, diagnostics);
        validate_policy_cardinality(
            detail.policy,
            detail.strategies.len(),
            &format!("{base_path}/detail/policy"),
            diagnostics,
        );
        validate_phase_limits(
            detail.limits,
            &format!("{base_path}/detail/limits"),
            diagnostics,
            detail
                .strategies
                .iter()
                .any(|strategy| matches!(strategy.fetch, Fetch::Browser { .. })),
        );
        let detail_limits = detail.limits.unwrap_or(PhaseLimits::BACKEND);
        for (index, strategy) in detail.strategies.iter().enumerate() {
            let strategy_path = format!("{base_path}/detail/strategies/{index}");
            validate_fetch_bounds(
                &strategy.fetch,
                detail_limits,
                &format!("{strategy_path}/fetch"),
                &strategy.key,
                diagnostics,
            );
        }
    }
}

fn validate_phase_limits(
    limits: Option<PhaseLimits>,
    path: &str,
    diagnostics: &mut Diagnostics,
    has_browser: bool,
) {
    let Some(limits) = limits else { return };
    let fields = [
        (
            "maxStrategyAttempts",
            limits.max_strategy_attempts,
            PhaseLimits::BACKEND.max_strategy_attempts,
        ),
        (
            "maxRequests",
            limits.max_requests,
            PhaseLimits::BACKEND.max_requests,
        ),
        (
            "maxProducedItems",
            limits.max_produced_items,
            PhaseLimits::BACKEND.max_produced_items,
        ),
        (
            "maxDurationMs",
            limits.max_duration_ms,
            PhaseLimits::BACKEND.max_duration_ms,
        ),
        ("maxPages", limits.max_pages, PhaseLimits::BACKEND.max_pages),
        (
            "maxBrowserActions",
            limits.max_browser_actions,
            PhaseLimits::BACKEND.max_browser_actions,
        ),
        (
            "maxFanOut",
            limits.max_fan_out,
            PhaseLimits::BACKEND.max_fan_out,
        ),
        (
            "maxResponseBytes",
            limits.max_response_bytes,
            PhaseLimits::BACKEND.max_response_bytes,
        ),
        (
            "maxBrowserRenderedBytes",
            limits.max_browser_rendered_bytes,
            PhaseLimits::BACKEND.max_browser_rendered_bytes,
        ),
    ];
    for (field, value, ceiling) in fields {
        if value == 0 || value > ceiling {
            diagnostics.push(compiler_error(
                "phase_limit_out_of_bounds",
                format!(
                    "{field} must be positive and may not exceed the backend ceiling of {ceiling}"
                ),
                format!("{path}/{field}"),
                serde_json::json!({ "value": value, "backendCeiling": ceiling }),
            ));
        }
    }
    if has_browser
        && limits.max_duration_ms
            < crate::profile_dsl::runtime::allowance::BROWSER_TEARDOWN_RESERVE_MS
    {
        diagnostics.push(compiler_error(
            "browser_phase_duration_below_teardown_reserve",
            format!("maxDurationMs must be at least {} when a Strategy uses Browser acquisition", crate::profile_dsl::runtime::allowance::BROWSER_TEARDOWN_RESERVE_MS),
            format!("{path}/maxDurationMs"),
            serde_json::json!({ "value": limits.max_duration_ms, "minimum": crate::profile_dsl::runtime::allowance::BROWSER_TEARDOWN_RESERVE_MS }),
        ));
    }
}

fn validate_discovery_strategy_list(
    discovery: &DiscoveryStep,
    base_path: &str,
    diagnostics: &mut Diagnostics,
) {
    validate_strategy_list_len(
        discovery.strategies.len(),
        &format!("{base_path}/discovery/strategies"),
        "discovery",
        diagnostics,
    );
}

fn validate_detail_strategy_list(
    detail: &DetailStep,
    base_path: &str,
    diagnostics: &mut Diagnostics,
) {
    validate_strategy_list_len(
        detail.strategies.len(),
        &format!("{base_path}/detail/strategies"),
        "detail",
        diagnostics,
    );
}

fn validate_policy_cardinality(
    policy: StrategyPolicy,
    strategy_count: usize,
    policy_path: &str,
    diagnostics: &mut Diagnostics,
) {
    let (required_accepted, field, code, message) = match policy {
        StrategyPolicy::AtLeast { count } => (
            count,
            "count",
            "strategy_policy_at_least_count_exceeds_cardinality",
            "at_least count may not exceed the final merged Strategy cardinality",
        ),
        StrategyPolicy::CollectAll { min_accepted } => (
            min_accepted,
            "minAccepted",
            "strategy_policy_collect_all_min_accepted_exceeds_cardinality",
            "collect_all minAccepted may not exceed the final merged Strategy cardinality",
        ),
        StrategyPolicy::FirstAccepted | StrategyPolicy::AllRequired => return,
    };
    if required_accepted > strategy_count {
        diagnostics.push(compiler_error(
            code,
            message,
            format!("{policy_path}/{field}"),
            serde_json::json!({
                "requiredAccepted": required_accepted,
                "strategyCount": strategy_count,
            }),
        ));
    }
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
    phase_limits: PhaseLimits,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let Fetch::Browser {
        timeout_ms,
        waits,
        interactions,
        ..
    } = fetch
    else {
        return;
    };
    validate_positive_bound(
        *timeout_ms,
        MAX_BROWSER_FETCH_TIMEOUT_MS.min(phase_limits.max_duration_ms),
        "invalid_fetch_timeout",
        "Browser fetch timeoutMs must be positive and within the backend ceiling",
        &format!("{path}/timeoutMs"),
        strategy_key,
        diagnostics,
    );
    if let Some(waits) = waits {
        for (index, wait) in waits.iter().enumerate() {
            validate_browser_wait_bounds(
                wait,
                phase_limits,
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
                phase_limits,
                &format!("{path}/interactions/{index}"),
                strategy_key,
                diagnostics,
            );
        }
    }
}

fn validate_positive_bound(
    value: u64,
    max: u64,
    code: &str,
    message: &str,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    if !(1..=max).contains(&value) {
        push_bounded_diagnostic(
            diagnostics,
            code,
            message.to_string(),
            path,
            strategy_key,
            serde_json::json!({ "minimum": 1, "maximum": max }),
        );
    }
}

fn validate_browser_wait_bounds(
    wait: &BrowserWait,
    phase_limits: PhaseLimits,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let (timeout_ms, selector) = match wait {
        BrowserWait::Selector {
            selector,
            timeout_ms,
        } => (*timeout_ms, Some(selector)),
        BrowserWait::NetworkIdle { timeout_ms } => (*timeout_ms, None),
    };
    if selector.is_some_and(|selector| selector.trim().is_empty()) {
        push_bounded_diagnostic(
            diagnostics,
            "empty_browser_selector",
            "Browser selector wait must declare a non-empty selector".to_string(),
            &format!("{path}/selector"),
            strategy_key,
            serde_json::json!({ "field": "selector" }),
        );
    }
    validate_positive_bound(
        timeout_ms,
        MAX_BROWSER_WAIT_TIMEOUT_MS.min(phase_limits.max_duration_ms),
        "invalid_browser_wait_timeout",
        "Browser wait timeoutMs must be positive and within the backend ceiling",
        &format!("{path}/timeoutMs"),
        strategy_key,
        diagnostics,
    );
}

fn validate_browser_interaction_bounds(
    interaction: &BrowserInteraction,
    phase_limits: PhaseLimits,
    path: &str,
    strategy_key: &str,
    diagnostics: &mut Diagnostics,
) {
    let (selector, max_count, wait_after_ms) = match interaction {
        BrowserInteraction::ClickIfVisible {
            selector,
            max_count,
            wait_after_ms,
        }
        | BrowserInteraction::ClickUntilGone {
            selector,
            max_count,
            wait_after_ms,
        } => (selector, *max_count, *wait_after_ms),
    };
    if selector.trim().is_empty() {
        push_bounded_diagnostic(
            diagnostics,
            "empty_browser_selector",
            "Browser interaction must declare a non-empty selector".to_string(),
            &format!("{path}/selector"),
            strategy_key,
            serde_json::json!({ "field": "selector" }),
        );
    }
    validate_positive_bound(
        max_count,
        MAX_BROWSER_INTERACTION_COUNT.min(phase_limits.max_browser_actions),
        "invalid_browser_interaction_count",
        "Browser interaction maxCount must be positive and within the backend ceiling",
        &format!("{path}/maxCount"),
        strategy_key,
        diagnostics,
    );
    let max_wait_after_ms = MAX_BROWSER_WAIT_AFTER_MS.min(phase_limits.max_duration_ms);
    if wait_after_ms.is_some_and(|value| value > max_wait_after_ms) {
        push_bounded_diagnostic(
            diagnostics,
            "invalid_browser_wait_after",
            "Browser interaction waitAfterMs exceeds the backend ceiling".to_string(),
            &format!("{path}/waitAfterMs"),
            strategy_key,
            serde_json::json!({ "minimum": 0, "maximum": max_wait_after_ms }),
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
