use crate::profile_dsl::{
    documents::ParseType,
    primitives::{
        cardinality::CompiledCardinality,
        select::{json_path, JsonPathSelectPlan, SelectedItem},
        transform::{CompiledTransformPipeline, TransformValue},
    },
};
use serde_json::Value;

use super::{
    error, eval_error, finish_values, member_path, require_document, CompiledValue,
    CompiledValueResult, ValueCompileContext, ValueCompileError, ValueCompileErrorKind,
    ValueDescriptor, ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "json_path" };

pub(super) fn compile(
    authored: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    require_document(context, path, ParseType::Json, "json_path")?;
    let selector = json_path::compile(authored).map_err(|message| {
        error(
            ValueCompileErrorKind::SelectorSyntax,
            &member_path(path, "jsonPath"),
            &message,
        )
    })?;
    Ok(CompiledValue::JsonPath {
        selector,
        cardinality,
        transforms,
    })
}

pub(super) fn execute<'a, 'doc, 'body>(
    selector: &JsonPathSelectPlan,
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
    let SelectedItem::Json(root) = selected else {
        return Err(eval_error(
            ValueEvaluationErrorKind::TypeMismatch,
            path,
            "compiled JSON Value received an incompatible selected item",
        ));
    };
    let values = json_path::execute(selector, root)
        .into_vec()
        .into_iter()
        .flat_map(transform_values)
        .collect();
    finish_values(values, cardinality, transforms, path)
}

fn transform_values<'doc, 'body>(
    selected: SelectedItem<'doc, 'body>,
) -> Vec<TransformValue<'doc, 'body>> {
    match selected {
        SelectedItem::Json(Value::Array(values)) => {
            values.iter().cloned().map(TransformValue::Json).collect()
        }
        SelectedItem::Json(value) => vec![TransformValue::Json(value.clone())],
        value => vec![value.into()],
    }
}
