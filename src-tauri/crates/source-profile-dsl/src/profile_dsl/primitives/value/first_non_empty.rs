use crate::profile_dsl::{
    documents::FieldExpression,
    primitives::transform::{CompiledTransformPipeline, TransformShape, TransformValue},
};

use super::{
    apply_pipeline, error, eval_error, evaluate_value_node, limit_error, member_path,
    CompiledValue, CompiledValueResult, ValueCompileError, ValueCompileErrorKind, ValueDescriptor,
    ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind, ValueShape,
    VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor {
    key: "first_non_empty",
};

pub(super) fn validate(
    candidates: &[FieldExpression],
    path: &str,
    inside_fallback_candidate: bool,
    output_shape: ValueShape,
) -> Result<(), ValueCompileError> {
    if inside_fallback_candidate {
        return Err(error(
            ValueCompileErrorKind::NestedFallback,
            path,
            "first_non_empty cannot occur inside another fallback candidate subtree",
        ));
    }
    if candidates.is_empty() {
        return Err(error(
            ValueCompileErrorKind::EmptyCandidates,
            &member_path(path, "candidates"),
            "first_non_empty requires at least one candidate",
        ));
    }
    if candidates.len() > VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES {
        return Err(limit_error(
            ValueCompileErrorKind::CandidateLimitExceeded,
            &member_path(path, "candidates"),
            candidates.len(),
            VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES,
            "first_non_empty candidate count exceeds the immutable maximum",
        ));
    }
    if output_shape != ValueShape::Scalar {
        return Err(error(
            ValueCompileErrorKind::CandidateSequence,
            &member_path(path, "transforms"),
            "first_non_empty wrapper transforms must retain scalar shape",
        ));
    }
    Ok(())
}

pub(super) fn validate_candidate(shape: ValueShape, path: &str) -> Result<(), ValueCompileError> {
    if shape != ValueShape::Scalar {
        return Err(error(
            ValueCompileErrorKind::CandidateSequence,
            path,
            "first_non_empty candidates must resolve to scalar values after transforms",
        ));
    }
    Ok(())
}

pub(super) fn compile(
    candidates: Vec<CompiledValue>,
    transforms: CompiledTransformPipeline,
) -> CompiledValue {
    CompiledValue::FirstNonEmpty {
        candidates,
        transforms,
    }
}

pub(super) fn references_source_name(candidates: &[CompiledValue]) -> bool {
    candidates.iter().any(CompiledValue::references_source_name)
}

pub(super) fn execute<'a, 'doc, 'body>(
    candidates: &[CompiledValue],
    transforms: &CompiledTransformPipeline,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let mut winner = String::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_path = format!("{path}/candidates/{index}");
        let result = evaluate_value_node(candidate, context, &candidate_path)?;
        match result {
            CompiledValueResult::Scalar(Some(value)) if !value.is_empty() => {
                winner = value;
                break;
            }
            CompiledValueResult::Scalar(_) => {}
            CompiledValueResult::Sequence(_) => {
                return Err(eval_error(
                    ValueEvaluationErrorKind::CandidateShape,
                    &candidate_path,
                    "first_non_empty candidate produced a sequence",
                ));
            }
        }
    }
    apply_pipeline(
        TransformShape::Scalar(TransformValue::Text(winner)),
        transforms,
        path,
    )
}
