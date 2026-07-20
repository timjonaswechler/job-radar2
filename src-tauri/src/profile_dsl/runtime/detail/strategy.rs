use super::*;

pub(super) struct DetailStrategyAttempt {
    pub(super) result: DetailExecutionResult,
    pub(super) accepted: bool,
}

pub(super) async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanDetailStrategy,
    step_acceptance: Option<&Acceptance>,
    context: RuntimeExecutionContext<'_>,
) -> DetailStrategyAttempt
where
    F: DetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/detail/strategies/{strategy_index}");
    let strategy_key = Some(strategy.key.clone());
    let mut diagnostics = Vec::new();

    let captures = match evaluate_strategy_captures(
        strategy,
        &plan.source_config,
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
        &plan.source_config,
        &plan.source.name,
        posting,
        &captures,
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
        context,
    )
    .await
    {
        Some(response) => response,
        None => return rejected_detail_attempt(diagnostics),
    };

    let document = match parse_response_document(
        &response.body,
        strategy,
        &base_path,
        strategy_key.as_deref(),
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
    let description = evaluate_string_field(
        &selected_document,
        &plan.source_config,
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

    let description = normalize_whitespace(description.trim());
    let accepted = accept_detail_result(
        &description,
        step_acceptance,
        strategy.accept_when.as_ref(),
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );
    DetailStrategyAttempt {
        result: DetailExecutionResult {
            description_text: accepted.then_some(description),
            diagnostics,
        },
        accepted,
    }
}

fn match_detail_document<'doc, 'body>(
    selected_document: RuntimeItem<'doc, 'body>,
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanDetailStrategy,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    let Some(field_match) = &strategy.field_match else {
        return Some(selected_document);
    };

    if let Some(key) = missing_posting_meta_key(&field_match.right, posting) {
        diagnostics.push(runtime_error(
            "posting_meta_missing",
            format!("detail match requires missing postingMeta `{key}`"),
            format!("{base_path}/match/right"),
            strategy_key,
            json!({ "postingMetaKey": key }),
        ));
        return None;
    }

    match selected_document {
        RuntimeItem::Json(Value::Array(items)) => match_json_detail_collection(
            items,
            plan,
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
            posting,
            captures,
            strategy.conditions.as_ref(),
            base_path,
            strategy_key,
            diagnostics,
        )? {
            continue;
        }
        if detail_document_matches_field(
            &item_document,
            field_match,
            plan,
            posting,
            captures,
            base_path,
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
            posting,
            captures,
            strategy.conditions.as_ref(),
            base_path,
            strategy_key,
            diagnostics,
        )? {
            continue;
        }
        if detail_document_matches_field(
            &item_document,
            field_match,
            plan,
            posting,
            captures,
            base_path,
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

fn detail_document_matches_field(
    item_document: &RuntimeItem<'_, '_>,
    field_match: &crate::profile_dsl::documents::strategy::FieldMatch,
    plan: &SourceExecutionPlan,
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let left_path = format!("{base_path}/match/left");
    let right_path = format!("{base_path}/match/right");
    let left = evaluate_string_field(
        item_document,
        &plan.source_config,
        &plan.source.name,
        posting,
        captures,
        &field_match.left,
        &left_path,
        strategy_key,
        diagnostics,
    );
    let right = evaluate_string_field(
        item_document,
        &plan.source_config,
        &plan.source.name,
        posting,
        captures,
        &field_match.right,
        &right_path,
        strategy_key,
        diagnostics,
    );
    if left.failed || right.failed {
        return None;
    }

    Some(left.value.is_some() && left.value == right.value)
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
    posting: &DetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    conditions: Option<&Vec<Filter>>,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let Some(conditions) = conditions else {
        return Some(true);
    };

    for (condition_index, condition) in conditions.iter().enumerate() {
        let condition_path = format!("{base_path}/where/{condition_index}");
        match condition {
            Filter::NonEmpty { field } => {
                let evaluation = evaluate_string_field(
                    item_document,
                    &plan.source_config,
                    &plan.source.name,
                    posting,
                    captures,
                    field,
                    &format!("{condition_path}/field"),
                    strategy_key,
                    diagnostics,
                );
                if evaluation.failed {
                    return None;
                }
                if evaluation.value.is_none() {
                    return Some(false);
                }
            }
            Filter::Regex { field, pattern } => {
                let regex = match Regex::new(pattern) {
                    Ok(regex) => regex,
                    Err(error) => {
                        diagnostics.push(runtime_error(
                            "where_pattern_invalid",
                            format!("Where filter regex pattern is invalid: {error}"),
                            format!("{condition_path}/pattern"),
                            strategy_key,
                            json!({ "pattern": pattern, "error": error.to_string() }),
                        ));
                        return None;
                    }
                };
                let evaluation = evaluate_string_field(
                    item_document,
                    &plan.source_config,
                    &plan.source.name,
                    posting,
                    captures,
                    field,
                    &format!("{condition_path}/field"),
                    strategy_key,
                    diagnostics,
                );
                if evaluation.failed {
                    return None;
                }
                let Some(value) = evaluation.value else {
                    return Some(false);
                };
                if !regex.is_match(&value) {
                    return Some(false);
                }
            }
        }
    }

    Some(true)
}

fn missing_posting_meta_key<'a>(
    expression: &'a FieldExpression,
    posting: &DetailPostingOccurrence,
) -> Option<&'a str> {
    match expression {
        FieldExpression::PostingMeta { key, .. } if !posting.posting_meta.contains_key(key) => {
            Some(key.as_str())
        }
        FieldExpression::Combine { parts, .. } => parts
            .iter()
            .find_map(|part| missing_posting_meta_key(&part.value, posting)),
        _ => None,
    }
}

pub(super) fn rejected_detail_attempt(diagnostics: Diagnostics) -> DetailStrategyAttempt {
    DetailStrategyAttempt {
        result: DetailExecutionResult {
            description_text: None,
            diagnostics,
        },
        accepted: false,
    }
}
