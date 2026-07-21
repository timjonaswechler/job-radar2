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
pub struct Equal {
    pub left: FieldExpression,
    pub right: FieldExpression,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EqualPlan {
    left: CompiledValue,
    right: CompiledValue,
}

impl EqualPlan {
    pub fn left(&self) -> &CompiledValue {
        &self.left
    }

    pub fn right(&self) -> &CompiledValue {
        &self.right
    }

    pub(super) fn references_source_name(&self) -> bool {
        self.left.references_source_name() || self.right.references_source_name()
    }
}

pub const DESCRIPTOR: PredicateDescriptor = PredicateDescriptor { key: "equal" };

/// Canonical typed equality used both by the registered `equal` Predicate and
/// Detection Strategy options that project comparable typed values.
pub fn values_equal<T: PartialEq + ?Sized>(left: &T, right: &T) -> bool {
    left == right
}

pub(super) fn compile(
    predicate: &Equal,
    context: &PredicateCompileContext,
) -> Result<EqualPlan, PredicateCompileError> {
    if context.placement != PredicatePlacement::DetailMatch {
        return Err(PredicateCompileError {
            kind: PredicateCompileErrorKind::Placement,
            path: String::new(),
            message: format!(
                "equal predicate is unavailable in {:?} placement",
                context.placement
            ),
            value_error: None,
        });
    }
    Ok(EqualPlan {
        left: compile_value(&predicate.left, &context.value)
            .map_err(|error| value_error("left", error))?,
        right: compile_value(&predicate.right, &context.value)
            .map_err(|error| value_error("right", error))?,
    })
}

pub(super) fn execute<F>(
    plan: &EqualPlan,
    evaluate: &mut F,
) -> Result<bool, PredicateEvaluationError>
where
    F: FnMut(&CompiledValue) -> Result<CompiledValueResult, ValueEvaluationError>,
{
    let left = evaluate(&plan.left).map_err(|source| PredicateEvaluationError {
        operand_path: "/left",
        source,
    })?;
    let right = evaluate(&plan.right).map_err(|source| PredicateEvaluationError {
        operand_path: "/right",
        source,
    })?;
    Ok(values_equal(&left, &right))
}
