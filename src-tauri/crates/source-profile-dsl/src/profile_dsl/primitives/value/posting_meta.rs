use crate::profile_dsl::primitives::{
    cardinality::CompiledCardinality,
    transform::{CompiledTransformPipeline, TransformValue},
};

use super::{
    error, eval_error, finish_values, CompiledValue, CompiledValueResult, ValueCompileContext,
    ValueCompileError, ValueCompileErrorKind, ValueDescriptor, ValueEvaluationContext,
    ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor {
    key: "posting_meta",
};

pub(super) fn compile(
    key: &str,
    context: &ValueCompileContext,
    path: &str,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> Result<CompiledValue, ValueCompileError> {
    if !context.placement.admits_posting() {
        return Err(error(
            ValueCompileErrorKind::PostingMetaUnavailable,
            path,
            "postingMeta is unavailable at this Value placement",
        ));
    }
    if !context.posting_meta_keys.contains(key) {
        return Err(error(
            ValueCompileErrorKind::UnknownPostingMetaKey,
            path,
            "postingMeta key is not declared by Discovery",
        ));
    }
    Ok(CompiledValue::PostingMeta {
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
        .posting()
        .ok_or_else(|| {
            eval_error(
                ValueEvaluationErrorKind::TypeMismatch,
                path,
                "posting context is unavailable",
            )
        })?
        .posting_meta
        .get(key)
        .cloned()
        .map(TransformValue::Text)
        .into_iter()
        .collect();
    finish_values(values, cardinality, transforms, path)
}
