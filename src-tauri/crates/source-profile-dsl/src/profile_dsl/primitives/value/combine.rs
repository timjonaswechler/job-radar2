use crate::profile_dsl::{
    documents::CombinePart,
    primitives::{
        cardinality::CompiledCardinality,
        transform::{CompiledTransformPipeline, TransformValue},
    },
};

use super::{
    error, eval_error, evaluate_value_node, finish_values, member_path, CompiledCombinePart,
    CompiledValue, CompiledValueResult, ValueCompileError, ValueCompileErrorKind, ValueDescriptor,
    ValueEvaluationContext, ValueEvaluationError, ValueEvaluationErrorKind,
};

pub(super) const DESCRIPTOR: ValueDescriptor = ValueDescriptor { key: "combine" };

pub(super) fn validate(parts: &[CombinePart], path: &str) -> Result<(), ValueCompileError> {
    if parts.is_empty() {
        return Err(error(
            ValueCompileErrorKind::EmptyCombineParts,
            &member_path(path, "parts"),
            "combine requires at least one part",
        ));
    }
    Ok(())
}

pub(super) fn compile(
    parts: Vec<CompiledCombinePart>,
    join: Option<&str>,
    cardinality: CompiledCardinality,
    transforms: CompiledTransformPipeline,
) -> CompiledValue {
    CompiledValue::Combine {
        parts,
        join: join.unwrap_or_default().to_string(),
        cardinality,
        transforms,
    }
}

pub(super) fn references_source_name(parts: &[CompiledCombinePart]) -> bool {
    parts.iter().any(|part| part.value.references_source_name())
}

pub(super) fn execute<'a, 'doc, 'body>(
    parts: &[CompiledCombinePart],
    join: &str,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let mut pieces = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let part_path = format!("{path}/parts/{index}/value");
        let result = evaluate_value_node(&part.value, context, &part_path)?;
        if let Some(piece) = result.non_empty_first() {
            pieces.push(piece.to_string());
        } else if !part.optional {
            return Err(eval_error(
                ValueEvaluationErrorKind::RequiredCombinePartMissing,
                &part_path,
                "required combine part resolved to an empty scalar",
            ));
        }
    }
    finish_values(
        vec![TransformValue::Text(pieces.join(join))],
        cardinality,
        transforms,
        path,
    )
}
