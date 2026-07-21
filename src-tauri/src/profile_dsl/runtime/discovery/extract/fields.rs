use super::*;
use crate::profile_dsl::primitives::{
    predicate::{evaluate_discovery_predicate, CompiledPredicate},
    value::{
        evaluate_discovery_output_value, DiscoveryFilterOutputValueContext, SourceValueView,
        ValueEvaluationError, ValueEvaluationErrorKind,
    },
};

pub(in crate::profile_dsl::runtime::discovery) struct FieldEvaluation {
    pub(in crate::profile_dsl::runtime::discovery) value: Option<String>,
    pub(in crate::profile_dsl::runtime::discovery) failed: bool,
}

pub(in crate::profile_dsl::runtime::discovery) fn evaluate_value_scalar(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &CompiledValue,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    let context = DiscoveryFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        selected: item,
        captures,
    };
    evaluate_result(
        evaluate_discovery_output_value(expression, &context),
        path,
        strategy_key,
        item_index,
        diagnostics,
    )
}

pub(in crate::profile_dsl::runtime::discovery) fn evaluate_predicate(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    predicate: &CompiledPredicate,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<bool> {
    let context = DiscoveryFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        selected: item,
        captures,
    };
    match evaluate_discovery_predicate(predicate, &context) {
        Ok(result) => Some(result),
        Err(error) => {
            push_value_error(
                error.source,
                &format!("{path}{}", error.operand_path),
                strategy_key,
                item_index,
                diagnostics,
            );
            None
        }
    }
}

pub(in crate::profile_dsl::runtime::discovery) fn evaluate_list_field(
    item: &RuntimeItem<'_, '_>,
    source_config: &SourceConfig,
    source_name: &str,
    captures: &BTreeMap<String, String>,
    expression: &CompiledValue,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<String>> {
    let context = DiscoveryFilterOutputValueContext {
        source: SourceValueView {
            source_name,
            source_config,
        },
        selected: item,
        captures,
    };
    match evaluate_discovery_output_value(expression, &context) {
        Ok(result) => Some(
            result
                .into_values()
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect(),
        ),
        Err(error) => {
            push_value_error(error, path, strategy_key, item_index, diagnostics);
            None
        }
    }
}

fn evaluate_result(
    result: Result<
        crate::profile_dsl::primitives::value::CompiledValueResult,
        ValueEvaluationError,
    >,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
    diagnostics: &mut Diagnostics,
) -> FieldEvaluation {
    match result {
        Ok(result) => FieldEvaluation {
            value: result.non_empty_first().map(str::to_string),
            failed: false,
        },
        Err(error) => {
            push_value_error(error, path, strategy_key, item_index, diagnostics);
            FieldEvaluation {
                value: None,
                failed: true,
            }
        }
    }
}

pub(in crate::profile_dsl::runtime::discovery) fn push_value_error(
    error: ValueEvaluationError,
    path: &str,
    strategy_key: Option<&str>,
    item_index: usize,
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
            "itemIndex": item_index,
        })
    } else {
        json!({
            "itemIndex": item_index,
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
