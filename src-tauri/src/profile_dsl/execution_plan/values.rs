use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::documents::extract::{CombinePart, FieldExpression, ListFieldExpression};
use crate::profile_dsl::documents::select::{CaptureRule, Captures, Filter};
use crate::profile_dsl::documents::strategy::FieldMatch;
use crate::profile_dsl::documents::transform::Transform;
use crate::profile_dsl::primitives::cardinality::{compile_cardinality, CompiledCardinality};
use crate::profile_dsl::template::{
    compile_template, CompiledTemplate, TemplateCompileError, TemplateDescriptor,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanFieldExpression {
    Const {
        value: Value,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Template {
        template: CompiledTemplate,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    SourceConfig {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    PostingMeta {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Capture {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    ItemField {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    JsonPath {
        #[serde(rename = "jsonPath")]
        json_path: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    XmlText {
        #[serde(rename = "textPath")]
        text_path: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    XmlElement {
        element: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    CssText {
        selector: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    CssAttribute {
        selector: String,
        attribute: String,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
    Combine {
        parts: Vec<ExecutionPlanCombinePart>,
        #[serde(skip_serializing_if = "Option::is_none")]
        join: Option<String>,
        cardinality: CompiledCardinality,
        #[serde(skip_serializing_if = "Option::is_none")]
        transforms: Option<Vec<Transform>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ExecutionPlanListFieldExpression {
    Single(ExecutionPlanFieldExpression),
    Multiple(Vec<ExecutionPlanFieldExpression>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanCombinePart {
    pub value: Box<ExecutionPlanFieldExpression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionPlanFilter {
    NonEmpty {
        field: ExecutionPlanFieldExpression,
    },
    Regex {
        field: ExecutionPlanFieldExpression,
        pattern: String,
    },
}

pub type ExecutionPlanCaptures = BTreeMap<String, ExecutionPlanCaptureRule>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanCaptureRule {
    pub from: ExecutionPlanFieldExpression,
    pub pattern: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlanFieldMatch {
    pub left: ExecutionPlanFieldExpression,
    pub right: ExecutionPlanFieldExpression,
}

pub(crate) fn compile_field_expression(
    value: &FieldExpression,
    descriptor: &TemplateDescriptor,
) -> Result<ExecutionPlanFieldExpression, TemplateCompileError> {
    use ExecutionPlanFieldExpression as C;
    Ok(match value {
        FieldExpression::Const {
            value,
            cardinality,
            transforms,
        } => C::Const {
            value: value.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::Template {
            template,
            cardinality,
            transforms,
        } => C::Template {
            template: compile_template(template, descriptor)?,
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => C::SourceConfig {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::PostingMeta {
            key,
            cardinality,
            transforms,
        } => C::PostingMeta {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::Capture {
            key,
            cardinality,
            transforms,
        } => C::Capture {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => C::ItemField {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => C::JsonPath {
            json_path: json_path.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::XmlText {
            text_path,
            cardinality,
            transforms,
        } => C::XmlText {
            text_path: text_path.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::XmlElement {
            element,
            cardinality,
            transforms,
        } => C::XmlElement {
            element: element.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::CssText {
            selector,
            cardinality,
            transforms,
        } => C::CssText {
            selector: selector.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::CssAttribute {
            selector,
            attribute,
            cardinality,
            transforms,
        } => C::CssAttribute {
            selector: selector.clone(),
            attribute: attribute.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
        FieldExpression::Combine {
            parts,
            join,
            cardinality,
            transforms,
        } => C::Combine {
            parts: parts
                .iter()
                .map(|part: &CombinePart| {
                    Ok(ExecutionPlanCombinePart {
                        value: Box::new(compile_field_expression(&part.value, descriptor)?),
                        optional: part.optional,
                    })
                })
                .collect::<Result<_, TemplateCompileError>>()?,
            join: join.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: transforms.clone(),
        },
    })
}

pub(crate) fn compile_list_field_expression(
    value: &ListFieldExpression,
    descriptor: &TemplateDescriptor,
) -> Result<ExecutionPlanListFieldExpression, TemplateCompileError> {
    Ok(match value {
        ListFieldExpression::Single(value) => {
            ExecutionPlanListFieldExpression::Single(compile_field_expression(value, descriptor)?)
        }
        ListFieldExpression::Multiple(values) => ExecutionPlanListFieldExpression::Multiple(
            values
                .iter()
                .map(|value| compile_field_expression(value, descriptor))
                .collect::<Result<_, _>>()?,
        ),
    })
}

pub(crate) fn compile_filters(
    values: Option<&Vec<Filter>>,
    descriptor: &TemplateDescriptor,
) -> Result<Option<Vec<ExecutionPlanFilter>>, TemplateCompileError> {
    values
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    Ok(match value {
                        Filter::NonEmpty { field } => ExecutionPlanFilter::NonEmpty {
                            field: compile_field_expression(field, descriptor)?,
                        },
                        Filter::Regex { field, pattern } => ExecutionPlanFilter::Regex {
                            field: compile_field_expression(field, descriptor)?,
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
    descriptor: &TemplateDescriptor,
) -> Result<Option<ExecutionPlanCaptures>, TemplateCompileError> {
    values
        .map(|values| {
            values
                .iter()
                .map(|(key, CaptureRule { from, pattern })| {
                    Ok((
                        key.clone(),
                        ExecutionPlanCaptureRule {
                            from: compile_field_expression(from, descriptor)?,
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
    descriptor: &TemplateDescriptor,
) -> Result<Option<ExecutionPlanFieldMatch>, TemplateCompileError> {
    value
        .map(|value| {
            Ok(ExecutionPlanFieldMatch {
                left: compile_field_expression(&value.left, descriptor)?,
                right: compile_field_expression(&value.right, descriptor)?,
            })
        })
        .transpose()
}
