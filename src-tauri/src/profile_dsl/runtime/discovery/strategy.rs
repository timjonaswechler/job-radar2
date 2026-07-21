use super::*;

pub(super) async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    step_acceptance: Option<&Acceptance>,
    context: RuntimeExecutionContext<'_>,
) -> StrategyExecution<Vec<PostingOccurrence>>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/discovery/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let candidates = if let Some(pagination) = &strategy.pagination {
        match execute_paginated_strategy(
            plan,
            source_config,
            fetcher,
            browser,
            strategy_index,
            strategy,
            pagination,
            &base_path,
            strategy_key.as_deref(),
            &mut diagnostics,
            context,
        )
        .await
        {
            Ok(candidates) => candidates,
            Err(cancellation) => {
                return StrategyExecution {
                    diagnostics,
                    completion: StrategyAttemptCompletion::Cancelled(cancellation),
                };
            }
        }
    } else {
        match execute_single_strategy_fetch(
            plan,
            source_config,
            fetcher,
            browser,
            strategy_index,
            strategy,
            &[],
            PaginationParameterLocation::Query,
            None,
            None,
            &base_path,
            strategy_key.as_deref(),
            &mut diagnostics,
            context,
        )
        .await
        {
            Ok(output) => output.candidates,
            Err(cancellation) => {
                return StrategyExecution {
                    diagnostics,
                    completion: StrategyAttemptCompletion::Cancelled(cancellation),
                };
            }
        }
    };

    if strategy.pagination.is_none() {
        for _ in &candidates {
            if context.is_cancelled() {
                return StrategyExecution {
                    diagnostics,
                    completion: StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
                        RuntimePhase::Discovery,
                        strategy_index,
                        strategy_key
                            .as_deref()
                            .expect("compiled strategy has a key"),
                        CancellationOperation::Phase,
                    )),
                };
            }
            if let Err(stop) = context.debit(AllowanceCharge {
                produced_items: 1,
                ..AllowanceCharge::default()
            }) {
                return StrategyExecution {
                    diagnostics,
                    completion: StrategyAttemptCompletion::Stopped(stop),
                };
            }
        }
    }

    let mut result = DiscoveryExecutionResult {
        candidates,
        provenance: Vec::new(),
        conflicts: Vec::new(),
        rejections: Vec::new(),
        diagnostics,
        report: None,
    };
    let execution_failed = discovery_execution_failed(&result);
    let accepted = !execution_failed
        && accept_discovery_result(
            &result.candidates,
            step_acceptance,
            strategy.accept_when.as_ref(),
            &base_path,
            strategy_key.as_deref(),
            &mut result.diagnostics,
        );
    let completion = if accepted {
        StrategyAttemptCompletion::Accepted(result.candidates)
    } else if execution_failed {
        StrategyAttemptCompletion::Failed
    } else {
        StrategyAttemptCompletion::Rejected
    };
    StrategyExecution {
        diagnostics: result.diagnostics,
        completion,
    }
}

fn discovery_execution_failed(result: &DiscoveryExecutionResult) -> bool {
    result.diagnostics.iter().any(is_strategy_level_error)
        || (result.candidates.is_empty()
            && result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error))
}

fn is_strategy_level_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == DiagnosticSeverity::Error
        && diagnostic
            .details
            .as_ref()
            .and_then(|details| details.get("itemIndex"))
            .is_none()
}

pub(super) struct StrategyFetchOutput {
    pub(super) candidates: Vec<PostingOccurrence>,
    pub(super) total_count: Option<u64>,
    pub(super) next_cursor: Option<String>,
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

pub(super) async fn execute_single_strategy_fetch<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanDiscoveryStrategy,
    pagination_params: &[(&str, String)],
    parameter_location: PaginationParameterLocation,
    total_path: Option<&str>,
    next_cursor_path: Option<&str>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
    context: RuntimeExecutionContext<'_>,
) -> Result<StrategyFetchOutput, TypedCancellation>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if context.is_cancelled() {
        return Err(TypedCancellation::strategy(
            RuntimePhase::Discovery,
            strategy_index,
            strategy_key.expect("compiled strategy has a key"),
            if strategy.pagination.is_some() {
                CancellationOperation::Pagination
            } else {
                CancellationOperation::Fetch
            },
        ));
    }
    let context = context.with_page_request(strategy.pagination.is_some());
    let response = match fetch_strategy_document_with_query_params(
        fetcher,
        browser,
        &strategy.fetch,
        strategy.parse.authored_charset(),
        source_config,
        &plan.source.name,
        query_params_for_location(pagination_params, parameter_location),
        json_body_params_for_location(pagination_params, parameter_location),
        base_path,
        strategy_key,
        strategy_index,
        diagnostics,
        context,
    )
    .await?
    {
        Some(response) => response,
        None => {
            return Ok(StrategyFetchOutput {
                candidates: Vec::new(),
                total_count: None,
                next_cursor: None,
            });
        }
    };

    Ok(extract_candidates_from_response(
        plan,
        source_config,
        strategy,
        &response,
        total_path,
        next_cursor_path,
        base_path,
        strategy_key,
        diagnostics,
    ))
}

fn extract_candidates_from_response(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    strategy: &ExecutionPlanDiscoveryStrategy,
    response: &CompleteParseText,
    total_path: Option<&str>,
    next_cursor_path: Option<&str>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> StrategyFetchOutput {
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

    let candidates = extract_candidates_from_items(
        plan,
        source_config,
        strategy,
        items,
        base_path,
        strategy_key,
        diagnostics,
    );
    StrategyFetchOutput {
        candidates,
        total_count,
        next_cursor,
    }
}

pub(super) fn extract_candidates_from_items(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    strategy: &ExecutionPlanDiscoveryStrategy,
    items: Vec<document::RuntimeItem<'_, '_>>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Vec<PostingOccurrence> {
    let mut candidates = Vec::new();
    for (item_index, item) in items.into_iter().enumerate() {
        if let Some(candidate) = extract_candidate(
            &item,
            strategy.captures.as_ref(),
            strategy.conditions.as_ref(),
            &strategy.extract.output,
            source_config,
            &plan.source.key,
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
