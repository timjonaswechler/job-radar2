use super::document::RuntimeItem;
use super::*;
use crate::profile_dsl::primitives::{
    capture::{evaluate_compiled_captures, CompiledCapturePlan},
    predicate::CompiledPredicate,
    value::{evaluate_discovery_capture_value, DiscoveryCaptureValueContext, SourceValueView},
};
use crate::profile_dsl::template::json_pointer_segment;

mod fields;

use fields::{
    evaluate_list_field, evaluate_predicate, evaluate_value_scalar,
    evaluate_value_scalar_preserving_empty, push_value_error, FieldEvaluation,
};

pub(super) fn extract_candidate(
    item: &RuntimeItem<'_, '_>,
    capture_rules: Option<&CompiledCapturePlan>,
    conditions: Option<&Vec<CompiledPredicate>>,
    output: &ExecutionPlanDiscoveryOutput,
    source_config: &SourceConfig,
    source_key: &str,
    source_name: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<PostingOccurrence> {
    let captures = evaluate_strategy_captures(
        item,
        capture_rules,
        source_config,
        source_name,
        base_path,
        strategy_key,
        item_index,
        diagnostics,
    )?;
    if !item_matches_conditions(
        item,
        conditions,
        source_config,
        source_name,
        &captures,
        base_path,
        strategy_key,
        item_index,
        diagnostics,
    )? {
        return None;
    }

    let url_path = format!("{base_path}/extract/reference/url");
    let provider_url = match evaluate_value_scalar_preserving_empty(
        item,
        source_config,
        source_name,
        &captures,
        &output.reference.url,
        &url_path,
        strategy_key,
        item_index,
        diagnostics,
    ) {
        FieldEvaluation {
            value: Some(value),
            failed: false,
        } if !value.is_empty() => value,
        FieldEvaluation { failed: true, .. } => return None,
        _ => {
            diagnostics.push(runtime_error(
                "occurrence_reference_invalid",
                "Discovery item has an invalid posting reference",
                &url_path,
                strategy_key,
                json!({ "itemIndex": item_index, "reason": "empty_provider_url" }),
            ));
            return None;
        }
    };
    let provider_posting_id = match output.reference.provider_posting_id.as_ref() {
        Some(expression) => match evaluate_value_scalar_preserving_empty(
            item,
            source_config,
            source_name,
            &captures,
            expression,
            &format!("{base_path}/extract/reference/providerPostingId"),
            strategy_key,
            item_index,
            diagnostics,
        ) {
            FieldEvaluation {
                value,
                failed: false,
            } => value,
            FieldEvaluation { failed: true, .. } => return None,
        },
        None => None,
    };
    let (reference, identity) = match crate::profile_dsl::occurrence::validate_posting_reference(
        source_key,
        &provider_url,
        provider_posting_id,
    ) {
        Ok(reference) => reference,
        Err(error) => {
            let (code, path, reason) = match error {
                crate::profile_dsl::occurrence::OccurrenceReferenceError::InvalidUrl =>
                    ("occurrence_reference_invalid", url_path, "invalid_absolute_http_url"),
                crate::profile_dsl::occurrence::OccurrenceReferenceError::UserInfo =>
                    ("occurrence_reference_invalid", url_path, "userinfo_forbidden"),
                crate::profile_dsl::occurrence::OccurrenceReferenceError::EmptyProviderPostingId =>
                    ("occurrence_provider_id_empty", format!("{base_path}/extract/reference/providerPostingId"), "empty_provider_posting_id"),
                crate::profile_dsl::occurrence::OccurrenceReferenceError::FragmentWithoutProviderPostingId =>
                    ("occurrence_url_identity_unsupported", url_path, "fragment_without_provider_posting_id"),
            };
            diagnostics.push(runtime_error(
                code,
                "Discovery item has an invalid posting reference",
                path,
                strategy_key,
                json!({ "itemIndex": item_index, "reason": reason }),
            ));
            return None;
        }
    };

    let provider_values = output.provider_values.as_ref();
    let scalar =
        |expression: Option<&CompiledValue>, field: &str, diagnostics: &mut Diagnostics| {
            expression.and_then(|expression| {
                optional_scalar(
                    item,
                    source_config,
                    source_name,
                    &captures,
                    expression,
                    &format!("{base_path}/extract/providerValues/{field}"),
                    strategy_key,
                    item_index,
                    diagnostics,
                )
            })
        };
    let locations = provider_values
        .and_then(|values| values.locations.as_ref())
        .map(|expression| {
            extract_locations_field(
                item,
                source_config,
                source_name,
                &captures,
                expression,
                &format!("{base_path}/extract/providerValues/locations"),
                strategy_key,
                item_index,
                diagnostics,
            )
        })
        .unwrap_or_default();
    let provider_values = crate::profile_dsl::occurrence::ProviderValues {
        title: scalar(
            provider_values.and_then(|values| values.title.as_ref()),
            "title",
            diagnostics,
        ),
        company: scalar(
            provider_values.and_then(|values| values.company.as_ref()),
            "company",
            diagnostics,
        ),
        locations,
        description_text: scalar(
            provider_values.and_then(|values| values.description_text.as_ref()),
            "descriptionText",
            diagnostics,
        ),
    };

    let mut hints = BTreeMap::new();
    for (key, hint) in output.hints.iter().flatten() {
        if let Some(value) = optional_scalar(
            item,
            source_config,
            source_name,
            &captures,
            &hint.value,
            &format!(
                "{base_path}/extract/hints/{}/value",
                json_pointer_segment(key)
            ),
            strategy_key,
            item_index,
            diagnostics,
        ) {
            hints.insert(
                key.clone(),
                crate::profile_dsl::occurrence::DiscoveryHint {
                    value,
                    hint_use: hint.hint_use,
                },
            );
        }
    }
    let mut posting_meta = BTreeMap::new();
    for (key, expression) in output.posting_meta.iter().flatten() {
        if let Some(value) = optional_scalar(
            item,
            source_config,
            source_name,
            &captures,
            expression,
            &format!(
                "{base_path}/extract/postingMeta/{}",
                json_pointer_segment(key)
            ),
            strategy_key,
            item_index,
            diagnostics,
        ) {
            posting_meta.insert(key.clone(), value);
        }
    }

    Some(PostingOccurrence {
        identity,
        reference,
        provider_values,
        hints,
        posting_meta,
    })
}

fn optional_scalar(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &CompiledValue,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    match evaluate_value_scalar(
        item,
        source_config,
        source_name,
        captures,
        expression,
        path,
        strategy_key,
        item_index,
        diagnostics,
    ) {
        FieldEvaluation {
            value: Some(value),
            failed: false,
        } => Some(value),
        _ => None,
    }
}

fn evaluate_strategy_captures(
    item: &RuntimeItem<'_, '_>,
    capture_rules: Option<&CompiledCapturePlan>,
    source_config: &SourceConfig,
    source_name: &str,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<BTreeMap<String, String>> {
    let Some(plan) = capture_rules else {
        return Some(BTreeMap::new());
    };
    let context = DiscoveryCaptureValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        selected: item,
    };
    match evaluate_compiled_captures(plan, |value| {
        evaluate_discovery_capture_value(value, &context)
    }) {
        Ok(outputs) => Some(
            outputs
                .into_iter()
                .map(|output| (output.key, output.value))
                .collect(),
        ),
        Err(errors) => {
            for error in errors {
                let path = format!(
                    "{base_path}/captures/{}",
                    json_pointer_segment(&error.capture_key)
                );
                if let Some(value_error) = error.value_error {
                    push_value_error(
                        value_error,
                        &format!("{path}/from"),
                        strategy_key,
                        item_index,
                        diagnostics,
                    );
                    continue;
                }
                let (code, message) = error.kind.diagnostic();
                diagnostics.push(runtime_error(
                    code,
                    message,
                    path,
                    strategy_key,
                    json!({ "captureKey": error.capture_key, "itemIndex": item_index }),
                ));
            }
            None
        }
    }
}

fn item_matches_conditions(
    item: &RuntimeItem<'_, '_>,
    conditions: Option<&Vec<CompiledPredicate>>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    base_path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let Some(conditions) = conditions else {
        return Some(true);
    };

    for (condition_index, condition) in conditions.iter().enumerate() {
        let condition_path = format!("{base_path}/where/{condition_index}");
        if !evaluate_predicate(
            item,
            source_config,
            source_name,
            captures,
            condition,
            &condition_path,
            strategy_key,
            item_index,
            diagnostics,
        )? {
            return Some(false);
        }
    }

    Some(true)
}

fn extract_locations_field(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &CompiledListValue,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Vec<String> {
    let (expressions, is_single): (Vec<&CompiledValue>, bool) = match expression {
        CompiledListValue::Single(expression) => (vec![expression], true),
        CompiledListValue::Multiple(expressions) => (expressions.iter().collect(), false),
    };

    let mut locations = Vec::new();
    for (index, expression) in expressions.into_iter().enumerate() {
        let expression_path = if is_single {
            path.to_string()
        } else {
            format!("{path}/{index}")
        };
        let Some(values) = evaluate_list_field(
            item,
            source_config,
            source_name,
            captures,
            expression,
            &expression_path,
            strategy_key,
            item_index,
            diagnostics,
        ) else {
            continue;
        };
        locations.extend(values);
    }
    locations
}
