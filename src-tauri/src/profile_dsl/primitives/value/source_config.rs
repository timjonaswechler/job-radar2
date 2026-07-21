use crate::profile_dsl::primitives::{
    cardinality::CompiledCardinality,
    transform::{CompiledTransformPipeline, TransformValue},
};
use serde_json::Value;

use super::{
    error, finish_values, CompiledValue, CompiledValueResult, ValueCompileContext,
    ValueCompileError, ValueCompileErrorKind, ValueDescriptor, ValueEvaluationContext,
    ValueEvaluationError,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor {
    key: "source_config",
};

pub(super) fn compile(
    key: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    if !context.source_config_keys.contains(key) {
        return Err(error(
            ValueCompileErrorKind::UnknownSourceConfigKey,
            path,
            "Source Config key is not declared by the Effective Source Profile",
        ));
    }
    Ok(CompiledValue::SourceConfig {
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
    let values = context
        .source()
        .source_config
        .get(key)
        .map(transform_values)
        .unwrap_or_default();
    finish_values(values, cardinality, transforms, path)
}

fn transform_values(value: &Value) -> Vec<TransformValue<'static, 'static>> {
    match value {
        Value::Array(values) => values.iter().cloned().map(TransformValue::Json).collect(),
        value => vec![TransformValue::Json(value.clone())],
    }
}
