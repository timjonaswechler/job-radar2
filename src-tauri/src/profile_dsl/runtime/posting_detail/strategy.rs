use super::*;

pub(super) struct PostingDetailStrategyAttempt {
    pub(super) result: PostingDetailExecutionResult,
    pub(super) accepted: bool,
}

pub(super) async fn execute_strategy<F, B>(
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    fetcher: &F,
    browser: &B,
    strategy_index: usize,
    strategy: &ExecutionPlanPostingDetailStrategy,
    step_acceptance: Option<&Acceptance>,
) -> PostingDetailStrategyAttempt
where
    F: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let base_path = format!("/postingDetail/strategies/{strategy_index}");
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
                "postingDetail descriptionText did not resolve to non-empty text",
                &description_path,
                strategy_key.as_deref(),
                json!({}),
            ));
        }
        return rejected_detail_attempt(diagnostics);
    };

    let description = normalize_whitespace(description.trim());
    let accepted = accept_posting_detail_result(
        &description,
        step_acceptance,
        strategy.accept_when.as_ref(),
        &base_path,
        strategy_key.as_deref(),
        &mut diagnostics,
    );
    PostingDetailStrategyAttempt {
        result: PostingDetailExecutionResult {
            description_text: accepted.then_some(description),
            diagnostics,
        },
        accepted,
    }
}

fn match_detail_document<'doc, 'body>(
    selected_document: RuntimeItem<'doc, 'body>,
    plan: &SourceExecutionPlan,
    posting: &PostingDetailPostingOccurrence,
    captures: &BTreeMap<String, String>,
    strategy: &ExecutionPlanPostingDetailStrategy,
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
            format!("postingDetail match requires missing postingMeta `{key}`"),
            format!("{base_path}/match/right"),
            strategy_key,
            json!({ "postingMetaKey": key }),
        ));
        return None;
    }

    let RuntimeItem::Json(Value::Array(items)) = selected_document else {
        diagnostics.push(runtime_error(
            "detail_match_unsupported_selection",
            "postingDetail match currently requires a JSON array selected by the strategy",
            format!("{base_path}/match"),
            strategy_key,
            json!({}),
        ));
        return None;
    };

    let mut matches = Vec::new();
    let left_path = format!("{base_path}/match/left");
    let right_path = format!("{base_path}/match/right");
    for item in items {
        let item_document = RuntimeItem::Json(item);
        let left = evaluate_string_field(
            &item_document,
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
            &item_document,
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
        if left.value.is_some() && left.value == right.value {
            matches.push(item);
        }
    }

    match matches.len() {
        0 => {
            diagnostics.push(runtime_error(
                "detail_match_missing",
                "postingDetail match found no detail item for the selected posting",
                format!("{base_path}/match"),
                strategy_key,
                json!({}),
            ));
            None
        }
        1 => Some(RuntimeItem::Json(matches.remove(0))),
        count => {
            diagnostics.push(runtime_error(
                "detail_match_multiple",
                format!(
                    "postingDetail match found {count} detail items for the selected posting; expected exactly one"
                ),
                format!("{base_path}/match"),
                strategy_key,
                json!({ "actualCount": count }),
            ));
            None
        }
    }
}

fn missing_posting_meta_key<'a>(
    expression: &'a FieldExpression,
    posting: &PostingDetailPostingOccurrence,
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

pub(super) fn rejected_detail_attempt(diagnostics: Diagnostics) -> PostingDetailStrategyAttempt {
    PostingDetailStrategyAttempt {
        result: PostingDetailExecutionResult {
            description_text: None,
            diagnostics,
        },
        accepted: false,
    }
}
