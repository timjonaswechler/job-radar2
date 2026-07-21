use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    documents::{CaptureRule, Captures, FieldExpression, FieldMatch, Filter, ListFieldExpression},
    primitives::value::{
        compile_list_value, compile_value, CompiledListValue, CompiledValue, ValueCompileContext,
        ValueCompileError,
    },
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledValueFilter {
    NonEmpty {
        field: CompiledValue,
    },
    Regex {
        field: CompiledValue,
        pattern: String,
    },
}

pub type CompiledValueCaptures = BTreeMap<String, CompiledValueCaptureRule>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledValueCaptureRule {
    pub from: CompiledValue,
    pub pattern: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledValueMatch {
    pub left: CompiledValue,
    pub right: CompiledValue,
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

pub(crate) fn compile_filters(
    values: Option<&Vec<Filter>>,
    context: &ValueCompileContext,
) -> Result<Option<Vec<CompiledValueFilter>>, ValueCompileError> {
    values
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    Ok(match value {
                        Filter::NonEmpty { field } => CompiledValueFilter::NonEmpty {
                            field: compile_value(field, context)?,
                        },
                        Filter::Regex { field, pattern } => CompiledValueFilter::Regex {
                            field: compile_value(field, context)?,
                            pattern: pattern.clone(),
                        },
                    })
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

pub(crate) fn compile_field_match(
    value: Option<&FieldMatch>,
    context: &ValueCompileContext,
) -> Result<Option<CompiledValueMatch>, ValueCompileError> {
    value
        .map(|value| {
            Ok(CompiledValueMatch {
                left: compile_value(&value.left, context)?,
                right: compile_value(&value.right, context)?,
            })
        })
        .transpose()
}
