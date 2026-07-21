use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    documents::{CaptureRule, Captures, FieldExpression, ListFieldExpression},
    primitives::{
        predicate::{
            compile_predicate, CompiledPredicate, Predicate, PredicateCompileContext,
            PredicateCompileError,
        },
        value::{
            compile_list_value, compile_value, CompiledListValue, CompiledValue,
            ValueCompileContext, ValueCompileError,
        },
    },
};

pub type CompiledValueCaptures = BTreeMap<String, CompiledValueCaptureRule>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledValueCaptureRule {
    pub from: CompiledValue,
    pub pattern: String,
}

pub(crate) fn compile_field_expression(
    value: &FieldExpression,
    context: &ValueCompileContext,
) -> Result<CompiledValue, ValueCompileError> {
    compile_value(value, context)
}

pub(crate) fn compile_list_field_expression(
    value: &ListFieldExpression,
    context: &ValueCompileContext,
) -> Result<CompiledListValue, ValueCompileError> {
    compile_list_value(value, context)
}

pub(crate) struct IndexedPredicateCompileError {
    pub index: usize,
    pub source: PredicateCompileError,
}

pub(crate) fn compile_predicates(
    predicates: Option<&[Predicate]>,
    context: &PredicateCompileContext,
) -> Result<Option<Vec<CompiledPredicate>>, IndexedPredicateCompileError> {
    predicates
        .map(|predicates| {
            predicates
                .iter()
                .enumerate()
                .map(|(index, predicate)| {
                    compile_predicate(predicate, context)
                        .map_err(|source| IndexedPredicateCompileError { index, source })
                })
                .collect()
        })
        .transpose()
}

pub(crate) fn compile_captures(
    values: Option<&Captures>,
    context: &ValueCompileContext,
) -> Result<Option<CompiledValueCaptures>, ValueCompileError> {
    values
        .map(|values| {
            values
                .iter()
                .map(|(key, CaptureRule { from, pattern })| {
                    Ok((
                        key.clone(),
                        CompiledValueCaptureRule {
                            from: compile_value(from, context)?,
                            pattern: pattern.clone(),
                        },
                    ))
                })
                .collect()
        })
        .transpose()
}
