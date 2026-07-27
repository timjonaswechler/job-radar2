use crate::profile_dsl::primitives::{
    cardinality::CompiledCardinality,
    select::SelectedItem,
    transform::{CompiledTransformPipeline, TransformValue},
};
use serde_json::Value;

use super::{
    eval_error, finish_values, require_selected, CompiledValue, CompiledValueResult,
    ValueCompileContext, ValueCompileError, ValueDescriptor, ValueEvaluationContext,
    ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "item_field" };

pub(super) fn compile(
    key: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    require_selected(context, path)?;
    Ok(CompiledValue::ItemField {
        key: key.to_string(),
        cardinality,
        transforms,
    })
}

pub(super) fn execute<'a, 'doc, 'body>(
    key: &str,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let selected = context.selected().ok_or_else(|| {
        eval_error(
            ValueEvaluationErrorKind::TypeMismatch,
            path,
            "selected item is unavailable",
        )
    })?;
    let values = match selected {
        SelectedItem::Json(value) => value.get(key).map(json_values).unwrap_or_default(),
        SelectedItem::Text(value) if key == "value" || key == "." => {
            vec![TransformValue::Text(value.clone())]
        }
        _ => Vec::new(),
    };
    finish_values(values, cardinality, transforms, path)
}

fn json_values<'doc, 'body>(value: &Value) -> Vec<TransformValue<'doc, 'body>> {
    match value {
        Value::Array(values) => values.iter().cloned().map(TransformValue::Json).collect(),
        value => vec![TransformValue::Json(value.clone())],
    }
}
