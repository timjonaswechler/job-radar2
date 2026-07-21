use crate::profile_dsl::{
    documents::AuthoredScalar,
    primitives::{
        cardinality::CompiledCardinality,
        transform::{CompiledTransformPipeline, TransformValue},
    },
};
use serde_json::Value;

use super::{
    finish_values, CompiledValue, CompiledValueResult, ValueDescriptor, ValueEvaluationError,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "const" };

pub(super) fn compile(
    value: &AuthoredScalar,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> CompiledValue {
    CompiledValue::Const {
        value: value.clone(),
        cardinality,
        transforms,
    }
}

pub(super) fn execute(
    value: &AuthoredScalar,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let value = TransformValue::Json(match value {
        AuthoredScalar::String(value) => Value::String(value.clone()),
        AuthoredScalar::Number(value) => Value::Number(value.clone()),
        AuthoredScalar::Boolean(value) => Value::Bool(*value),
    });
    finish_values(vec![value], cardinality, transforms, path)
}
