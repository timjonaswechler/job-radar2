use serde::{Deserialize, Serialize};

use super::{
    value_error, PredicateCompileContext, PredicateCompileError, PredicateCompileErrorKind,
    PredicateDescriptor, PredicateEvaluationError, PredicatePlacement,
};
use crate::profile_dsl::{
    documents::FieldExpression,
    primitives::value::{compile_value, CompiledValue, CompiledValueResult, ValueEvaluationError},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NonEmpty {
    pub field: FieldExpression,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonEmptyPlan {
    field: CompiledValue,
}

impl NonEmptyPlan {
    pub fn field(&self) -> &CompiledValue {
        &self.field
    }

    pub(super) fn references_source_name(&self) -> bool {
        self.field.references_source_name()
    }
}

pub const DESCRIPTOR: PredicateDescriptor = PredicateDescriptor { key: "non_empty" };

pub(super) fn compile(
    predicate: &NonEmpty,
    context: &PredicateCompileContext,
) -> Result<NonEmptyPlan, PredicateCompileError> {
    if context.placement != PredicatePlacement::Where {
        return Err(PredicateCompileError {
            kind: PredicateCompileErrorKind::Placement,
            path: String::new(),
            message: format!(
                "non_empty predicate is unavailable in {:?} placement",
                context.placement
            ),
            value_error: None,
        });
    }
    Ok(NonEmptyPlan {
        field: compile_value(&predicate.field, &context.value)
            .map_err(|error| value_error("field", error))?,
    })
}

pub(super) fn execute<F>(
    plan: &NonEmptyPlan,
    evaluate: &mut F,
) -> Result<bool, PredicateEvaluationError>
where
    F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
{
    let result = evaluate(&plan.field).map_err(|source| PredicateEvaluationError {
        operand_path: "/field",
        source,
    })?;
    Ok(match result {
        CompiledValueResult::Scalar(value) => value.is_some_and(|value| !value.is_empty()),
        CompiledValueResult::Sequence(values) => values.iter().any(|value| !value.is_empty()),
    })
}
