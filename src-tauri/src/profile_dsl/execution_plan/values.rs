use crate::profile_dsl::{
    documents::{FieldExpression, ListFieldExpression},
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
