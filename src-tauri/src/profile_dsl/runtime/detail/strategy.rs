use super::*;

pub(super) async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanDetailStrategy,
    step_acceptance: Option<&Acceptance>,
    context: RuntimeExecutionContext<'_>,
) -> StrategyExecution<String>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/detail/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let captures = match evaluate_strategy_captures(
        strategy,
        source_config,
        &plan.source.name,
        posting,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(captures) => captures,
        None => return rejected_detail_attempt(diagnostics),
    };

    let response = match fetch_strategy_document(
        fetcher,
        browser,
        &strategy.fetch,
        strategy.parse.authored_charset(),
        source_config,
        &plan.source.name,
        posting,
        &captures,
        &base_path,
        strategy_key.as_deref(),
        strategy_index,
        &mut diagnostics,
        context,
    )
    .await
    {
        Ok(Some(response)) => response,
        Ok(None) => return rejected_detail_attempt(diagnostics),
        Err(cancellation) => {
            return StrategyExecution {
                diagnostics,
                completion: StrategyAttemptCompletion::Cancelled(cancellation),
            };
        }
    };

    let document = match strategy.parse.parse_with_diagnostics(
        response.as_input(),
        ParseDiagnosticContext {
            base_path: &base_path,
            strategy_key: strategy_key.as_deref(),
        },
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };

    let selected_document = match select_detail_document(
        &document,
        &strategy.select,
        strategy.field_match.is_some(),
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };
    let selected_document = match match_detail_document(
        selected_document,
        plan,
        source_config,
        posting,
        &captures,
        strategy,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    ) {
        Some(document) => document,
        None => return rejected_detail_attempt(diagnostics),
    };

    if strategy.field_match.is_none() {
        match detail_document_matches_conditions(
            &selected_document,
            plan,
            source_config,
            posting,
            &captures,
            strategy.conditions.as_ref(),
            &base_path,
            strategy_key.as_deref(),
            &mut diagnostics,
        ) {
            Some(true) => {}
            Some(false) => {
                diagnostics.push(runtime_error(
                    "where_condition_not_matched",
                    "detail where filters rejected the selected detail document",
                    format!("{base_path}/where"),
                    strategy_key.as_deref(),
                    json!({}),
                ));
                return rejected_detail_attempt(diagnostics);
            }
            None => return rejected_detail_attempt(diagnostics),
        }
    }

    let description_path = format!("{base_path}/extract/fields/descriptionText");
    let description = evaluate_value_scalar(
        &selected_document,
        source_config,
        &plan.source.name,
        posting,
        &captures,
        &strategy.extract.fields.description_text,
        &description_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );

    let Some(description) = description.value.filter(|value| !value.trim().is_empty()) else {
        if !description.failed {
            diagnostics.push(runtime_error(
                "description_empty",
                "detail descriptionText did not resolve to non-empty text",
                &description_path,
                strategy_key.as_deref(),
                json!({}),
            ));
        }
        return rejected_detail_attempt(diagnostics);
    };

    let description = normalize_whitespace_text(description.trim());
    let accepted = accept_detail_result(
        &description,
        step_acceptance,
        strategy.accept_when.as_ref(),
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );
    if !accepted {
        return StrategyExecution {
            diagnostics,
            completion: StrategyAttemptCompletion::Rejected,
        };
    }
    if context.is_cancelled() {
        return StrategyExecution {
            diagnostics,
            completion: StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
                RuntimePhase::Detail,
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
    StrategyExecution {
        diagnostics,
        completion: StrategyAttemptCompletion::Accepted(description),
    }
}

fn match_detail_document<'doc, 'body>(
    selected_document: RuntimeItem<'doc, 'body>,
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    if strategy.field_match.is_none() {
        return Some(selected_document);
    }

    match selected_document {
        RuntimeItem::Json(Value::Array(items)) => match_json_detail_collection(
            items,
            plan,
            source_config,
            posting,
            captures,
            strategy,
            base_path,
            strategy_key,
            diagnostics,
        ),
        RuntimeItem::XmlCollection(items) => match_xml_detail_collection(
            items,
            plan,
            source_config,
            posting,
            captures,
            strategy,
            base_path,
            strategy_key,
            diagnostics,
        ),
        _ => {
            diagnostics.push(runtime_error(
                "detail_match_unsupported_selection",
                "detail match requires a JSON array or XML element collection selected by the strategy",
                format!("{base_path}/match"),
                strategy_key,
                json!({}),
            ));
            None
        }
    }
}

fn match_json_detail_collection<'doc, 'body>(
    items: &'doc [Value],
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    let field_match = strategy.field_match.as_ref()?;
    let mut matches = Vec::new();
    for item in items {
        let item_document = RuntimeItem::Json(item);
        if !detail_document_matches_conditions(
            &item_document,
            plan,
            source_config,
            posting,
            captures,
            strategy.conditions.as_ref(),
            base_path,
            strategy_key,
            diagnostics,
        )? {
            continue;
        }
        if evaluate_predicate(
            &item_document,
            source_config,
            &plan.source.name,
            posting,
            captures,
            field_match,
            &format!("{base_path}/match"),
            strategy_key,
            diagnostics,
        )? {
            matches.push(item);
        }
    }

    finish_detail_matches(
        matches,
        RuntimeItem::Json,
        base_path,
        strategy_key,
        diagnostics,
    )
}

fn match_xml_detail_collection<'doc, 'body>(
    items: Vec<roxmltree::Node<'doc, 'body>>,
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    let field_match = strategy.field_match.as_ref()?;
    let mut matches = Vec::new();
    for item in items {
        let item_document = RuntimeItem::Xml(item);
        if !detail_document_matches_conditions(
            &item_document,
            plan,
            source_config,
            posting,
            captures,
            strategy.conditions.as_ref(),
            base_path,
            strategy_key,
            diagnostics,
        )? {
            continue;
        }
        if evaluate_predicate(
            &item_document,
            source_config,
            &plan.source.name,
            posting,
            captures,
            field_match,
            &format!("{base_path}/match"),
            strategy_key,
            diagnostics,
        )? {
            matches.push(item);
        }
    }

    finish_detail_matches(
        matches,
        RuntimeItem::Xml,
        base_path,
        strategy_key,
        diagnostics,
    )
}

fn finish_detail_matches<'doc, 'body, T>(
    mut matches: Vec<T>,
    into_runtime_item: impl Fn(T) -> RuntimeItem<'doc, 'body>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    match matches.len() {
        0 => {
            diagnostics.push(runtime_error(
                "detail_match_missing",
                "detail match found no detail item for the selected posting",
                format!("{base_path}/match"),
                strategy_key,
                json!({}),
            ));
            None
        }
        1 => Some(into_runtime_item(matches.remove(0))),
        count => {
            diagnostics.push(runtime_error(
                "detail_match_multiple",
                format!(
                    "detail match found {count} detail items for the selected posting; expected exactly one"
                ),
                format!("{base_path}/match"),
                strategy_key,
                json!({ "actualCount": count }),
            ));
            None
        }
    }
}

fn detail_document_matches_conditions(
    item_document: &RuntimeItem<'_, '_>,
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    conditions: Option<&Vec<CompiledPredicate>>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let Some(conditions) = conditions else {
        return Some(true);
    };

    for (condition_index, condition) in conditions.iter().enumerate() {
        let condition_path = format!("{base_path}/where/{condition_index}");
        if !evaluate_predicate(
            item_document,
            source_config,
            &plan.source.name,
            posting,
            captures,
            condition,
            &condition_path,
            strategy_key,
            diagnostics,
        )? {
            return Some(false);
        }
    }

    Some(true)
}

pub(super) fn rejected_detail_attempt(diagnostics: Diagnostics) -> StrategyExecution<String> {
    StrategyExecution {
        diagnostics,
        completion: StrategyAttemptCompletion::Failed,
    }
}
