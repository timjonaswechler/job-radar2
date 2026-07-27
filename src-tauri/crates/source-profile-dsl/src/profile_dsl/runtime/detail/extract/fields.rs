use super::*;
use crate::profile_dsl::primitives::{
    predicate::{evaluate_detail_predicate, CompiledPredicate},
    select::SelectedItem,
    value::{
        evaluate_detail_output_value, DetailMatchFilterOutputValueContext, PostingValueView,
        SourceValueView, ValueEvaluationError, ValueEvaluationErrorKind,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::profile_dsl::runtime::detail) struct FieldEvaluation {
    pub(in crate::profile_dsl::runtime::detail) value: Option<String>,
    pub(in crate::profile_dsl::runtime::detail) failed: bool,
}

pub(in crate::profile_dsl::runtime::detail) fn evaluate_value_scalar(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &CompiledValue,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let Some(selected) = selected_item(document) else {
        diagnostics.push(runtime_error(
            "compiled_value_context_missing",
            "compiled Value cannot execute against an XML collection",
            path,
            strategy_key,
            json!({}),
        ));
        return FieldEvaluation {
            value: None,
            failed: true,
        };
    };
    let context = DetailMatchFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        posting: posting_view(posting),
        selected: &selected,
        captures,
    };
    evaluate_result(
        evaluate_detail_output_value(expression, &context),
        path,
        strategy_key,
        diagnostics,
    )
}

pub(in crate::profile_dsl::runtime::detail) fn evaluate_value_list(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingOccurrence,
    captures: &BTreeMap<String, String>,
    expression: &CompiledValue,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    let selected = selected_item(document)?;
    let context = DetailMatchFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        posting: posting_view(posting),
        selected: &selected,
        captures,
    };
    match evaluate_detail_output_value(expression, &context) {
        Ok(result) => Some(
            result
                .into_values()
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect(),
        ),
        Err(error) => {
            push_value_error(error, path, strategy_key, diagnostics);
            None
        }
    }
}

pub(in crate::profile_dsl::runtime::detail) fn evaluate_predicate(
    document: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    posting: &PostingOccurrence,
    captures: &BTreeMap<String, String>,
    predicate: &CompiledPredicate,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let Some(selected) = selected_item(document) else {
        diagnostics.push(runtime_error(
            "compiled_value_context_missing",
            "compiled Predicate cannot execute against an XML collection",
            path,
            strategy_key,
            json!({}),
        ));
        return None;
    };
    let context = DetailMatchFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        posting: posting_view(posting),
        selected: &selected,
        captures,
    };
    match evaluate_detail_predicate(predicate, &context) {
        Ok(result) => Some(result),
        Err(error) => {
            push_value_error(
                error.source,
                &format!("{path}{}", error.operand_path),
                strategy_key,
                diagnostics,
            );
            None
        }
    }
}

fn selected_item<'doc, 'body>(
    item: &RuntimeItem<'doc, 'body>,
) -> Option<SelectedItem<'doc, 'body>> {
    match item {
        RuntimeItem::Json(value) => Some(SelectedItem::Json(value)),
        RuntimeItem::Xml(value) => Some(SelectedItem::Xml(*value)),
        RuntimeItem::Html(value) => Some(SelectedItem::Html(value.clone())),
        RuntimeItem::Text(value) => Some(SelectedItem::Text(value.clone())),
        RuntimeItem::XmlCollection(_) => None,
    }
}

pub(in crate::profile_dsl::runtime::detail) fn posting_view(
    posting: &PostingOccurrence,
) -> PostingValueView<'_> {
    PostingValueView {
        title: posting.provider_values.title.as_deref().unwrap_or_default(),
        company: posting
            .provider_values
            .company
            .as_deref()
            .unwrap_or_default(),
        url: &posting.reference.provider_url,
        locations: &posting.provider_values.locations,
        description_text: posting.provider_values.description_text.as_deref(),
        posting_meta: &posting.posting_meta,
    }
}

fn evaluate_result(
    result: Result<
        crate::profile_dsl::primitives::value::CompiledValueResult,
        ValueEvaluationError,
    >,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    match result {
        Ok(result) => FieldEvaluation {
            value: result.non_empty_first().map(str::to_string),
            failed: false,
        },
        Err(error) => {
            push_value_error(error, path, strategy_key, diagnostics);
            FieldEvaluation {
                value: None,
                failed: true,
            }
        }
    }
}

pub(in crate::profile_dsl::runtime::detail) fn push_value_error(
    error: ValueEvaluationError,
    path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) {
    let code = match error.kind {
        ValueEvaluationErrorKind::Template => "field_template_failed",
        ValueEvaluationErrorKind::Cardinality => "field_cardinality_mismatch",
        ValueEvaluationErrorKind::TransformTypeMismatch => "transform_type_mismatch",
        ValueEvaluationErrorKind::TransformInvalidPercentEncoding => {
            "transform_invalid_percent_encoding"
        }
        ValueEvaluationErrorKind::TransformInvalidUtf8 => "transform_invalid_utf8",
        ValueEvaluationErrorKind::TypeMismatch => "field_type_mismatch",
        ValueEvaluationErrorKind::RequiredCombinePartMissing => "required_combine_part_missing",
        ValueEvaluationErrorKind::CandidateShape => "first_non_empty_candidate_shape",
    };
    let details = if error.kind == ValueEvaluationErrorKind::Cardinality {
        json!({
            "expectedCardinality": error.expected_cardinality,
            "actualCount": error.actual_count,
        })
    } else {
        json!({
            "transformIndex": error.transform_index,
            "valueIndex": error.value_index,
        })
    };
    diagnostics.push(runtime_error(
        code,
        error.message,
        format!("{path}{}", error.relative_path),
        strategy_key,
        details,
    ));
}
