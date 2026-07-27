use crate::profile_dsl::primitives::{
    cardinality::CompiledCardinality,
    transform::{CompiledTransformPipeline, TransformValue},
};

use super::{
    error, eval_error, finish_values, CompiledValue, CompiledValueResult, ValueCompileContext,
    ValueCompileError, ValueCompileErrorKind, ValueDescriptor, ValueEvaluationContext,
    ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "capture" };

pub(super) fn compile(
    key: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    if !context.placement.admits_captures() {
        return Err(error(
            ValueCompileErrorKind::CaptureUnavailable,
            path,
            "captures are unavailable at this Value placement",
        ));
    }
    if !context.capture_keys.contains(key) {
        return Err(error(
            ValueCompileErrorKind::UnknownCaptureKey,
            path,
            "capture key is not declared by the Strategy",
        ));
    }
    Ok(CompiledValue::Capture {
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
        .captures()
        .ok_or_else(|| {
            eval_error(
                ValueEvaluationErrorKind::TypeMismatch,
                path,
                "capture context is unavailable",
            )
        })?
        .get(key)
        .cloned()
        .map(TransformValue::Text)
        .into_iter()
        .collect();
    finish_values(values, cardinality, transforms, path)
}
