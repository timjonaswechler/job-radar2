use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::documents::extract::{CombinePart, FieldExpression, ListFieldExpression};
use crate::profile_dsl::documents::select::{CaptureRule, Captures, Filter};
use crate::profile_dsl::documents::strategy::FieldMatch;
use crate::profile_dsl::primitives::cardinality::{compile_cardinality, CompiledCardinality};
use crate::profile_dsl::primitives::transform::{
    compile_transform_pipeline, CompileTransformError, CompiledTransformPipeline,
};
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
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    Template {
        template: CompiledTemplate,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    SourceConfig {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    PostingMeta {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    Capture {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    ItemField {
        key: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    JsonPath {
        #[serde(rename = "jsonPath")]
        json_path: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    XmlText {
        #[serde(rename = "textPath")]
        text_path: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    XmlElement {
        element: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    CssText {
        selector: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    CssAttribute {
        selector: String,
        attribute: String,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
    },
    Combine {
        parts: Vec<ExecutionPlanCombinePart>,
        #[serde(skip_serializing_if = "Option::is_none")]
        join: Option<String>,
        cardinality: CompiledCardinality,
        #[serde(default)]
        transforms: CompiledTransformPipeline,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FieldExpressionCompileError {
    Template(TemplateCompileError),
    Transform(CompileTransformError),
}

impl std::fmt::Display for FieldExpressionCompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template(error) => error.fmt(formatter),
            Self::Transform(error) => error.fmt(formatter),
        }
    }
}

impl From<TemplateCompileError> for FieldExpressionCompileError {
    fn from(error: TemplateCompileError) -> Self {
        Self::Template(error)
    }
}

impl From<CompileTransformError> for FieldExpressionCompileError {
    fn from(error: CompileTransformError) -> Self {
        Self::Transform(error)
    }
}

pub(crate) fn compile_field_expression(
    value: &FieldExpression,
    descriptor: &TemplateDescriptor,
) -> Result<ExecutionPlanFieldExpression, FieldExpressionCompileError> {
    use ExecutionPlanFieldExpression as C;
    Ok(match value {
        FieldExpression::Const {
            value,
            cardinality,
            transforms,
        } => C::Const {
            value: value.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::Template {
            template,
            cardinality,
            transforms,
        } => C::Template {
            template: compile_template(template, descriptor)?,
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::SourceConfig {
            key,
            cardinality,
            transforms,
        } => C::SourceConfig {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::PostingMeta {
            key,
            cardinality,
            transforms,
        } => C::PostingMeta {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::Capture {
            key,
            cardinality,
            transforms,
        } => C::Capture {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::ItemField {
            key,
            cardinality,
            transforms,
        } => C::ItemField {
            key: key.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::JsonPath {
            json_path,
            cardinality,
            transforms,
        } => C::JsonPath {
            json_path: json_path.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::XmlText {
            text_path,
            cardinality,
            transforms,
        } => C::XmlText {
            text_path: text_path.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::XmlElement {
            element,
            cardinality,
            transforms,
        } => C::XmlElement {
            element: element.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
        FieldExpression::CssText {
            selector,
            cardinality,
            transforms,
        } => C::CssText {
            selector: selector.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
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
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
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
                .collect::<Result<_, FieldExpressionCompileError>>()?,
            join: join.clone(),
            cardinality: compile_cardinality(cardinality.unwrap_or_default()),
            transforms: compile_transform_pipeline(transforms.as_deref().unwrap_or(&[]))?,
        },
    })
}

pub(crate) fn compile_list_field_expression(
    value: &ListFieldExpression,
    descriptor: &TemplateDescriptor,
) -> Result<ExecutionPlanListFieldExpression, FieldExpressionCompileError> {
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
) -> Result<Option<Vec<ExecutionPlanFilter>>, FieldExpressionCompileError> {
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
) -> Result<Option<ExecutionPlanCaptures>, FieldExpressionCompileError> {
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
) -> Result<Option<ExecutionPlanFieldMatch>, FieldExpressionCompileError> {
    value
        .map(|value| {
            Ok(ExecutionPlanFieldMatch {
                left: compile_field_expression(&value.left, descriptor)?,
                right: compile_field_expression(&value.right, descriptor)?,
            })
        })
        .transpose()
}
