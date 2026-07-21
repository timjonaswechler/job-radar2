use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::profile_dsl::{
    documents::{AuthoredScalar, FieldExpression, ParseType},
    primitives::{
        cardinality::{compile_cardinality, Cardinality, CardinalityOutcome, CompiledCardinality},
        select::{
            selected_document_is_compatible, xml_text::node_text as xml_node_text, CssSelectPlan,
            JsonPathSelectPlan, SelectedDocumentType, SelectedItem, SelectedSequence,
            XmlElementSelectPlan, XmlTextSelectPlan,
        },
        transform::{
            compile_transform_pipeline, normalize_whitespace_text, CompileTransformErrorKind,
            CompiledTransformPipeline, TransformErrorKind, TransformShape, TransformValue,
        },
    },
    template::CompiledTemplate,
};

mod capture;
mod combine;
mod const_value;
mod css_attribute;
mod css_text;
mod first_non_empty;
mod item_field;
mod json_path;
mod posting_meta;
mod source_config;
mod template;
mod xml_element;
mod xml_text;

pub const VALUE_MAX_DEPTH: usize = 16;
pub const VALUE_MAX_NODES: usize = 1_024;
pub const VALUE_MAX_FIRST_NON_EMPTY_CANDIDATES: usize = 16;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValuePlacement {
    DiscoveryCaptureSource,
    DiscoveryFilterOutput,
    DetailCaptureSource,
    DetailMatchFilterOutput,
}

impl ValuePlacement {
    pub const ALL: [Self; 4] = [
        Self::DiscoveryCaptureSource,
        Self::DiscoveryFilterOutput,
        Self::DetailCaptureSource,
        Self::DetailMatchFilterOutput,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::DiscoveryCaptureSource => "discovery_capture_source",
            Self::DiscoveryFilterOutput => "discovery_filter_output",
            Self::DetailCaptureSource => "detail_capture_source",
            Self::DetailMatchFilterOutput => "detail_match_filter_output",
        }
    }

    pub const fn descriptor(self) -> &'static ValuePlacementDescriptor {
        match self {
            Self::DiscoveryCaptureSource => &VALUE_PLACEMENT_DESCRIPTORS[0],
            Self::DiscoveryFilterOutput => &VALUE_PLACEMENT_DESCRIPTORS[1],
            Self::DetailCaptureSource => &VALUE_PLACEMENT_DESCRIPTORS[2],
            Self::DetailMatchFilterOutput => &VALUE_PLACEMENT_DESCRIPTORS[3],
        }
    }

    pub(super) const fn admits_selected(self) -> bool {
        self.descriptor().selected_item
    }
    pub(super) const fn admits_posting(self) -> bool {
        self.descriptor().posting
    }
    pub(super) const fn admits_captures(self) -> bool {
        self.descriptor().captures
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValuePlacementDescriptor {
    pub key: &'static str,
    pub selected_item: bool,
    pub posting: bool,
    pub captures: bool,
}

const VALUE_PLACEMENT_DESCRIPTORS: [ValuePlacementDescriptor; 4] = [
    ValuePlacementDescriptor {
        key: "discovery_capture_source",
        selected_item: true,
        posting: false,
        captures: false,
    },
    ValuePlacementDescriptor {
        key: "discovery_filter_output",
        selected_item: true,
        posting: false,
        captures: true,
    },
    ValuePlacementDescriptor {
        key: "detail_capture_source",
        selected_item: false,
        posting: true,
        captures: false,
    },
    ValuePlacementDescriptor {
        key: "detail_match_filter_output",
        selected_item: true,
        posting: true,
        captures: true,
    },
];

pub fn value_placement_descriptors() -> &'static [ValuePlacementDescriptor] {
    &VALUE_PLACEMENT_DESCRIPTORS
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Const,
    Template,
    SourceConfig,
    PostingMeta,
    Capture,
    ItemField,
    JsonPath,
    XmlText,
    XmlElement,
    CssText,
    CssAttribute,
    Combine,
    FirstNonEmpty,
}

impl ValueKind {
    pub const ALL: [Self; 13] = [
        Self::Const,
        Self::Template,
        Self::SourceConfig,
        Self::PostingMeta,
        Self::Capture,
        Self::ItemField,
        Self::JsonPath,
        Self::XmlText,
        Self::XmlElement,
        Self::CssText,
        Self::CssAttribute,
        Self::Combine,
        Self::FirstNonEmpty,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Template => "template",
            Self::SourceConfig => "source_config",
            Self::PostingMeta => "posting_meta",
            Self::Capture => "capture",
            Self::ItemField => "item_field",
            Self::JsonPath => "json_path",
            Self::XmlText => "xml_text",
            Self::XmlElement => "xml_element",
            Self::CssText => "css_text",
            Self::CssAttribute => "css_attribute",
            Self::Combine => "combine",
            Self::FirstNonEmpty => "first_non_empty",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueDescriptor {
    pub key: &'static str,
}

const VALUE_DESCRIPTORS: [ValueDescriptor; 13] = [
    const_value::DESCRIPTOR,
    template::DESCRIPTOR,
    source_config::DESCRIPTOR,
    posting_meta::DESCRIPTOR,
    capture::DESCRIPTOR,
    item_field::DESCRIPTOR,
    json_path::DESCRIPTOR,
    xml_text::DESCRIPTOR,
    xml_element::DESCRIPTOR,
    css_text::DESCRIPTOR,
    css_attribute::DESCRIPTOR,
    combine::DESCRIPTOR,
    first_non_empty::DESCRIPTOR,
];

pub fn value_descriptors() -> &'static [ValueDescriptor] {
    &VALUE_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueRegistryError {
    Duplicate {
        layer: &'static str,
        keys: Vec<String>,
    },
    Missing {
        layer: &'static str,
        keys: Vec<String>,
    },
    Extra {
        layer: &'static str,
        keys: Vec<String>,
    },
}

pub fn validate_value_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), ValueRegistryError> {
    for (layer, keys) in [
        ("schema", schema_keys),
        ("serde", serde_keys),
        ("registration", registration_keys),
    ] {
        let mut counts = BTreeMap::new();
        for key in keys {
            *counts.entry(key.clone()).or_insert(0usize) += 1;
        }
        let duplicates = counts
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect::<Vec<_>>();
        if !duplicates.is_empty() {
            return Err(ValueRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let expected = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [("serde", serde_keys), ("registration", registration_keys)] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ValueRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(ValueRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValuePlacementRegistryError {
    Duplicate { keys: Vec<String> },
    Missing { keys: Vec<String> },
    Extra { keys: Vec<String> },
}

pub fn validate_value_placement_registration_keys(
    placement_keys: &[String],
    descriptor_keys: &[String],
) -> Result<(), ValuePlacementRegistryError> {
    let mut counts = BTreeMap::new();
    for key in descriptor_keys {
        *counts.entry(key.clone()).or_insert(0usize) += 1;
    }
    let duplicate = counts
        .into_iter()
        .filter_map(|(key, count)| (count > 1).then_some(key))
        .collect::<Vec<_>>();
    if !duplicate.is_empty() {
        return Err(ValuePlacementRegistryError::Duplicate { keys: duplicate });
    }
    let expected = placement_keys.iter().cloned().collect::<BTreeSet<_>>();
    let actual = descriptor_keys.iter().cloned().collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ValuePlacementRegistryError::Missing { keys: missing });
    }
    let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
    if !extra.is_empty() {
        return Err(ValuePlacementRegistryError::Extra { keys: extra });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueCompileContext {
    pub placement: ValuePlacement,
    pub document_type: Option<ParseType>,
    pub source_config_keys: BTreeSet<String>,
    pub posting_meta_keys: BTreeSet<String>,
    pub capture_keys: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueShape {
    Scalar,
    Sequence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueCompileErrorKind {
    UnknownSourceConfigKey,
    PostingMetaUnavailable,
    UnknownPostingMetaKey,
    CaptureUnavailable,
    UnknownCaptureKey,
    SelectedItemUnavailable,
    DocumentIncompatible,
    Template,
    TemplateTransformPipe,
    DepthLimitExceeded,
    NodeLimitExceeded,
    EmptyCandidates,
    CandidateLimitExceeded,
    EmptyCombineParts,
    CandidateSequence,
    NestedFallback,
    SelectorSyntax,
    TransformEmptySeparator,
    TransformInvalidRegex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueCompileError {
    pub kind: ValueCompileErrorKind,
    pub path: String,
    pub actual: Option<usize>,
    pub maximum: Option<usize>,
    pub message: String,
}

pub(crate) fn value_expression_node_count(expression: &FieldExpression) -> usize {
    match expression {
        FieldExpression::Combine { parts, .. } => {
            1 + parts
                .iter()
                .map(|part| value_expression_node_count(&part.value))
                .sum::<usize>()
        }
        FieldExpression::FirstNonEmpty { candidates, .. } => {
            1 + candidates
                .iter()
                .map(value_expression_node_count)
                .sum::<usize>()
        }
        _ => 1,
    }
}

#[derive(Default)]
struct Stats {
    nodes: usize,
    max_depth: usize,
}

pub(super) fn require_selected(
    context: &ValueCompileContext,
    path: &str,
) -> Result<(), ValueCompileError> {
    if context.placement.admits_selected() && context.document_type.is_some() {
        Ok(())
    } else {
        Err(error(
            ValueCompileErrorKind::SelectedItemUnavailable,
            path,
            "selected item/document is unavailable at this Value placement",
        ))
    }
}
pub(super) fn require_document(
    context: &ValueCompileContext,
    path: &str,
    expected: ParseType,
    capability: &str,
) -> Result<(), ValueCompileError> {
    require_selected(context, path)?;
    let required = match expected {
        ParseType::Json => SelectedDocumentType::Json,
        ParseType::Xml => SelectedDocumentType::Xml,
        ParseType::Html => SelectedDocumentType::Html,
    };
    if context
        .document_type
        .is_some_and(|document_type| selected_document_is_compatible(document_type, required))
    {
        Ok(())
    } else {
        Err(error(
            ValueCompileErrorKind::DocumentIncompatible,
            path,
            &format!("selected document is incompatible with {capability}"),
        ))
    }
}
pub(super) fn member_path(path: &str, member: &str) -> String {
    if path.is_empty() {
        format!("/{member}")
    } else {
        format!("{path}/{member}")
    }
}
pub(super) fn error(kind: ValueCompileErrorKind, path: &str, message: &str) -> ValueCompileError {
    ValueCompileError {
        kind,
        path: path.to_string(),
        actual: None,
        maximum: None,
        message: message.to_string(),
    }
}
pub(super) fn limit_error(
    kind: ValueCompileErrorKind,
    path: &str,
    actual: usize,
    maximum: usize,
    message: &str,
) -> ValueCompileError {
    ValueCompileError {
        kind,
        path: path.to_string(),
        actual: Some(actual),
        maximum: Some(maximum),
        message: message.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledValue {
    Const {
        value: AuthoredScalar,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    Template {
        template: CompiledTemplate,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    SourceConfig {
        key: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    PostingMeta {
        key: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    Capture {
        key: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    ItemField {
        key: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    JsonPath {
        selector: JsonPathSelectPlan,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    XmlText {
        selector: XmlTextSelectPlan,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    XmlElement {
        selector: XmlElementSelectPlan,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    CssText {
        selector: CssSelectPlan,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    CssAttribute {
        selector: CssSelectPlan,
        attribute: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    Combine {
        parts: Vec<CompiledCombinePart>,
        join: String,
        cardinality: CompiledCardinality,
        transforms: CompiledTransformPipeline,
    },
    FirstNonEmpty {
        candidates: Vec<CompiledValue>,
        transforms: CompiledTransformPipeline,
    },
}

impl CompiledValue {
    pub const fn kind(&self) -> ValueKind {
        match self {
            Self::Const { .. } => ValueKind::Const,
            Self::Template { .. } => ValueKind::Template,
            Self::SourceConfig { .. } => ValueKind::SourceConfig,
            Self::PostingMeta { .. } => ValueKind::PostingMeta,
            Self::Capture { .. } => ValueKind::Capture,
            Self::ItemField { .. } => ValueKind::ItemField,
            Self::JsonPath { .. } => ValueKind::JsonPath,
            Self::XmlText { .. } => ValueKind::XmlText,
            Self::XmlElement { .. } => ValueKind::XmlElement,
            Self::CssText { .. } => ValueKind::CssText,
            Self::CssAttribute { .. } => ValueKind::CssAttribute,
            Self::Combine { .. } => ValueKind::Combine,
            Self::FirstNonEmpty { .. } => ValueKind::FirstNonEmpty,
        }
    }

    pub fn shape(&self) -> ValueShape {
        match self {
            Self::FirstNonEmpty { .. } => ValueShape::Scalar,
            Self::Const {
                cardinality,
                transforms,
                ..
            }
            | Self::Template {
                cardinality,
                transforms,
                ..
            }
            | Self::SourceConfig {
                cardinality,
                transforms,
                ..
            }
            | Self::PostingMeta {
                cardinality,
                transforms,
                ..
            }
            | Self::Capture {
                cardinality,
                transforms,
                ..
            }
            | Self::ItemField {
                cardinality,
                transforms,
                ..
            }
            | Self::JsonPath {
                cardinality,
                transforms,
                ..
            }
            | Self::XmlText {
                cardinality,
                transforms,
                ..
            }
            | Self::XmlElement {
                cardinality,
                transforms,
                ..
            }
            | Self::CssText {
                cardinality,
                transforms,
                ..
            }
            | Self::CssAttribute {
                cardinality,
                transforms,
                ..
            }
            | Self::Combine {
                cardinality,
                transforms,
                ..
            } => {
                let initial = if matches!(cardinality, CompiledCardinality::All(_)) {
                    ValueShape::Sequence
                } else {
                    ValueShape::Scalar
                };
                transform_output_shape(transforms, initial)
            }
        }
    }

    pub(crate) fn references_source_name(&self) -> bool {
        match self {
            Self::Template { template, .. } => template::references_source_name(template),
            Self::Combine { parts, .. } => combine::references_source_name(parts),
            Self::FirstNonEmpty { candidates, .. } => {
                first_non_empty::references_source_name(candidates)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledCombinePart {
    pub value: Box<CompiledValue>,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompiledListValue {
    Single(CompiledValue),
    Multiple(Vec<CompiledValue>),
}

pub fn compile_value(
    expression: &FieldExpression,
    context: &ValueCompileContext,
) -> Result<CompiledValue, ValueCompileError> {
    let mut stats = Stats::default();
    compile_value_node(expression, context, "", 1, false, &mut stats).map(|compiled| compiled.value)
}

struct CompiledNode {
    value: CompiledValue,
    shape: ValueShape,
}

fn compile_value_node(
    expression: &FieldExpression,
    context: &ValueCompileContext,
    path: &str,
    depth: usize,
    inside_fallback_candidate: bool,
    stats: &mut Stats,
) -> Result<CompiledNode, ValueCompileError> {
    stats.nodes += 1;
    stats.max_depth = stats.max_depth.max(depth);
    if stats.nodes > VALUE_MAX_NODES {
        return Err(limit_error(
            ValueCompileErrorKind::NodeLimitExceeded,
            path,
            stats.nodes,
            VALUE_MAX_NODES,
            "Value expression node count exceeds the immutable maximum",
        ));
    }
    if depth > VALUE_MAX_DEPTH {
        return Err(limit_error(
            ValueCompileErrorKind::DepthLimitExceeded,
            path,
            depth,
            VALUE_MAX_DEPTH,
            "Value expression depth exceeds the immutable maximum",
        ));
    }

    let transforms = compile_pipeline(expression.transforms().unwrap_or(&[]), path)?;
    let cardinality = compile_cardinality(expression.cardinality());
    let cardinality_shape = if matches!(expression.cardinality(), Cardinality::All(_)) {
        ValueShape::Sequence
    } else {
        ValueShape::Scalar
    };
    let output_shape = transform_output_shape(&transforms, cardinality_shape);

    let value = match expression {
        FieldExpression::Const { value, .. } => {
            const_value::compile(value, cardinality, transforms)
        }
        FieldExpression::Template {
            template: authored, ..
        } => template::compile(authored, context, path, cardinality, transforms)?,
        FieldExpression::SourceConfig { key, .. } => {
            source_config::compile(key, context, path, cardinality, transforms)?
        }
        FieldExpression::PostingMeta { key, .. } => {
            posting_meta::compile(key, context, path, cardinality, transforms)?
        }
        FieldExpression::Capture { key, .. } => {
            capture::compile(key, context, path, cardinality, transforms)?
        }
        FieldExpression::ItemField { key, .. } => {
            item_field::compile(key, context, path, cardinality, transforms)?
        }
        FieldExpression::JsonPath {
            json_path: authored,
            ..
        } => json_path::compile(authored, context, path, cardinality, transforms)?,
        FieldExpression::XmlText { text_path, .. } => {
            xml_text::compile(text_path, context, path, cardinality, transforms)?
        }
        FieldExpression::XmlElement { element, .. } => {
            xml_element::compile(element, context, path, cardinality, transforms)?
        }
        FieldExpression::CssText { selector, .. } => {
            css_text::compile(selector, context, path, cardinality, transforms)?
        }
        FieldExpression::CssAttribute {
            selector,
            attribute,
            ..
        } => css_attribute::compile(selector, attribute, context, path, cardinality, transforms)?,
        FieldExpression::Combine { parts, join, .. } => {
            combine::validate(parts, path)?;
            let parts = parts
                .iter()
                .enumerate()
                .map(|(index, part)| {
                    let compiled = compile_value_node(
                        &part.value,
                        context,
                        &format!("{path}/parts/{index}/value"),
                        depth + 1,
                        inside_fallback_candidate,
                        stats,
                    )?;
                    Ok(CompiledCombinePart {
                        value: Box::new(compiled.value),
                        optional: part.optional.unwrap_or(false),
                    })
                })
                .collect::<Result<_, ValueCompileError>>()?;
            combine::compile(parts, join.as_deref(), cardinality, transforms)
        }
        FieldExpression::FirstNonEmpty { candidates, .. } => {
            first_non_empty::validate(candidates, path, inside_fallback_candidate, output_shape)?;
            let candidates = candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let candidate_path = format!("{path}/candidates/{index}");
                    let compiled = compile_value_node(
                        candidate,
                        context,
                        &candidate_path,
                        depth + 1,
                        true,
                        stats,
                    )?;
                    first_non_empty::validate_candidate(compiled.shape, &candidate_path)?;
                    Ok(compiled.value)
                })
                .collect::<Result<_, ValueCompileError>>()?;
            return Ok(CompiledNode {
                value: first_non_empty::compile(candidates, transforms),
                shape: ValueShape::Scalar,
            });
        }
    };
    Ok(CompiledNode {
        value,
        shape: output_shape,
    })
}

fn compile_pipeline(
    transforms: &[crate::profile_dsl::primitives::transform::Transform],
    path: &str,
) -> Result<CompiledTransformPipeline, ValueCompileError> {
    compile_transform_pipeline(transforms).map_err(|transform_error| {
        let kind = match transform_error.kind {
            CompileTransformErrorKind::EmptySeparator => {
                ValueCompileErrorKind::TransformEmptySeparator
            }
            CompileTransformErrorKind::InvalidRegex => ValueCompileErrorKind::TransformInvalidRegex,
        };
        error(
            kind,
            &format!("{path}/transforms/{}", transform_error.transform_index),
            &transform_error.message,
        )
    })
}

fn transform_output_shape(transforms: &CompiledTransformPipeline, input: ValueShape) -> ValueShape {
    use crate::profile_dsl::primitives::transform::TransformShapeKind;
    match transforms.output_shape(match input {
        ValueShape::Scalar => TransformShapeKind::Scalar,
        ValueShape::Sequence => TransformShapeKind::Sequence,
    }) {
        TransformShapeKind::Scalar => ValueShape::Scalar,
        TransformShapeKind::Sequence => ValueShape::Sequence,
    }
}

pub fn compile_list_value(
    expression: &crate::profile_dsl::documents::ListFieldExpression,
    context: &ValueCompileContext,
) -> Result<CompiledListValue, ValueCompileError> {
    match expression {
        crate::profile_dsl::documents::ListFieldExpression::Single(value) => {
            Ok(CompiledListValue::Single(compile_value(value, context)?))
        }
        crate::profile_dsl::documents::ListFieldExpression::Multiple(values) => {
            Ok(CompiledListValue::Multiple(
                values
                    .iter()
                    .map(|value| compile_value(value, context))
                    .collect::<Result<_, _>>()?,
            ))
        }
    }
}

#[derive(Clone)]
pub enum SelectedValueCarrier<'doc, 'body> {
    Scalar(Option<SelectedItem<'doc, 'body>>),
    Sequence(SelectedSequence<'doc, 'body>),
}
impl<'doc, 'body> From<SelectedSequence<'doc, 'body>> for SelectedValueCarrier<'doc, 'body> {
    fn from(value: SelectedSequence<'doc, 'body>) -> Self {
        Self::Sequence(value)
    }
}

pub struct SourceValueView<'a> {
    pub source_name: &'a str,
    pub source_config: &'a Map<String, Value>,
}
pub struct PostingValueView<'a> {
    pub title: &'a str,
    pub company: &'a str,
    pub url: &'a str,
    pub locations: &'a [String],
    pub description_text: Option<&'a str>,
    pub posting_meta: &'a BTreeMap<String, String>,
}
pub struct DiscoveryCaptureValueContext<'a, 'doc, 'body> {
    pub source: SourceValueView<'a>,
    pub selected: &'a SelectedItem<'doc, 'body>,
}
pub struct DiscoveryFilterOutputValueContext<'a, 'doc, 'body> {
    pub source: SourceValueView<'a>,
    pub selected: &'a SelectedItem<'doc, 'body>,
    pub captures: &'a BTreeMap<String, String>,
}
pub struct DetailCaptureValueContext<'a> {
    pub source: SourceValueView<'a>,
    pub posting: PostingValueView<'a>,
}
pub struct DetailMatchFilterOutputValueContext<'a, 'doc, 'body> {
    pub source: SourceValueView<'a>,
    pub posting: PostingValueView<'a>,
    pub selected: &'a SelectedItem<'doc, 'body>,
    pub captures: &'a BTreeMap<String, String>,
}

pub(super) enum ValueEvaluationContext<'a, 'doc, 'body> {
    DiscoveryCapture(&'a DiscoveryCaptureValueContext<'a, 'doc, 'body>),
    DiscoveryFilterOutput(&'a DiscoveryFilterOutputValueContext<'a, 'doc, 'body>),
    DetailCapture(&'a DetailCaptureValueContext<'a>),
    DetailMatchFilterOutput(&'a DetailMatchFilterOutputValueContext<'a, 'doc, 'body>),
}
impl<'a, 'doc, 'body> ValueEvaluationContext<'a, 'doc, 'body> {
    pub(super) fn source(&self) -> &SourceValueView<'a> {
        match self {
            Self::DiscoveryCapture(v) => &v.source,
            Self::DiscoveryFilterOutput(v) => &v.source,
            Self::DetailCapture(v) => &v.source,
            Self::DetailMatchFilterOutput(v) => &v.source,
        }
    }
    pub(super) fn posting(&self) -> Option<&PostingValueView<'a>> {
        match self {
            Self::DetailCapture(v) => Some(&v.posting),
            Self::DetailMatchFilterOutput(v) => Some(&v.posting),
            _ => None,
        }
    }
    pub(super) fn selected(&self) -> Option<&SelectedItem<'doc, 'body>> {
        match self {
            Self::DiscoveryCapture(v) => Some(v.selected),
            Self::DiscoveryFilterOutput(v) => Some(v.selected),
            Self::DetailMatchFilterOutput(v) => Some(v.selected),
            Self::DetailCapture(_) => None,
        }
    }
    pub(super) fn captures(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            Self::DiscoveryFilterOutput(v) => Some(v.captures),
            Self::DetailMatchFilterOutput(v) => Some(v.captures),
            _ => None,
        }
    }
}
pub(super) fn json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.clone()),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompiledValueResult {
    Scalar(Option<String>),
    Sequence(Vec<String>),
}
impl CompiledValueResult {
    pub fn first(&self) -> Option<&str> {
        match self {
            Self::Scalar(value) => value.as_deref(),
            Self::Sequence(values) => values.first().map(String::as_str),
        }
    }
    pub fn non_empty_first(&self) -> Option<&str> {
        self.first().filter(|value| !value.is_empty())
    }
    pub fn into_values(self) -> Vec<String> {
        match self {
            Self::Scalar(value) => value.into_iter().collect(),
            Self::Sequence(values) => values,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueEvaluationErrorKind {
    Template,
    Cardinality,
    TransformTypeMismatch,
    TransformInvalidPercentEncoding,
    TransformInvalidUtf8,
    TypeMismatch,
    RequiredCombinePartMissing,
    CandidateShape,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueEvaluationError {
    pub kind: ValueEvaluationErrorKind,
    pub relative_path: String,
    pub transform_index: Option<usize>,
    pub value_index: Option<usize>,
    pub expected_cardinality: Option<String>,
    pub actual_count: Option<usize>,
    pub message: String,
}

pub fn evaluate_discovery_capture_value<'a, 'doc, 'body>(
    value: &CompiledValue,
    context: &'a DiscoveryCaptureValueContext<'a, 'doc, 'body>,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    evaluate_value_node(
        value,
        &ValueEvaluationContext::DiscoveryCapture(context),
        "",
    )
}

pub fn evaluate_discovery_output_value<'a, 'doc, 'body>(
    value: &CompiledValue,
    context: &'a DiscoveryFilterOutputValueContext<'a, 'doc, 'body>,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    evaluate_value_node(
        value,
        &ValueEvaluationContext::DiscoveryFilterOutput(context),
        "",
    )
}

pub fn evaluate_detail_capture_value<'a>(
    value: &CompiledValue,
    context: &'a DetailCaptureValueContext<'a>,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    evaluate_value_node(value, &ValueEvaluationContext::DetailCapture(context), "")
}

pub fn evaluate_detail_output_value<'a, 'doc, 'body>(
    value: &CompiledValue,
    context: &'a DetailMatchFilterOutputValueContext<'a, 'doc, 'body>,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    evaluate_value_node(
        value,
        &ValueEvaluationContext::DetailMatchFilterOutput(context),
        "",
    )
}

pub(super) fn evaluate_value_node<'a, 'doc, 'body>(
    value: &CompiledValue,
    context: &ValueEvaluationContext<'a, 'doc, 'body>,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    match value {
        CompiledValue::Const {
            value,
            cardinality,
            transforms,
        } => const_value::execute(value, *cardinality, transforms, path),
        CompiledValue::Template {
            template: compiled,
            cardinality,
            transforms,
        } => template::execute(compiled, *cardinality, transforms, context, path),
        CompiledValue::SourceConfig {
            key,
            cardinality,
            transforms,
        } => source_config::execute(key, *cardinality, transforms, context, path),
        CompiledValue::PostingMeta {
            key,
            cardinality,
            transforms,
        } => posting_meta::execute(key, *cardinality, transforms, context, path),
        CompiledValue::Capture {
            key,
            cardinality,
            transforms,
        } => capture::execute(key, *cardinality, transforms, context, path),
        CompiledValue::ItemField {
            key,
            cardinality,
            transforms,
        } => item_field::execute(key, *cardinality, transforms, context, path),
        CompiledValue::JsonPath {
            selector,
            cardinality,
            transforms,
        } => json_path::execute(selector, *cardinality, transforms, context, path),
        CompiledValue::XmlText {
            selector,
            cardinality,
            transforms,
        } => xml_text::execute(selector, *cardinality, transforms, context, path),
        CompiledValue::XmlElement {
            selector,
            cardinality,
            transforms,
        } => xml_element::execute(selector, *cardinality, transforms, context, path),
        CompiledValue::CssText {
            selector,
            cardinality,
            transforms,
        } => css_text::execute(selector, *cardinality, transforms, context, path),
        CompiledValue::CssAttribute {
            selector,
            attribute,
            cardinality,
            transforms,
        } => css_attribute::execute(selector, attribute, *cardinality, transforms, context, path),
        CompiledValue::Combine {
            parts,
            join,
            cardinality,
            transforms,
        } => combine::execute(parts, join, *cardinality, transforms, context, path),
        CompiledValue::FirstNonEmpty {
            candidates,
            transforms,
        } => first_non_empty::execute(candidates, transforms, context, path),
    }
}

fn finish_values<'doc, 'body>(
    values: Vec<TransformValue<'doc, 'body>>,
    cardinality: CompiledCardinality,
    transforms: &CompiledTransformPipeline,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let outcome = cardinality
        .execute(values)
        .map_err(|error| ValueEvaluationError {
            kind: ValueEvaluationErrorKind::Cardinality,
            relative_path: path.to_string(),
            transform_index: None,
            value_index: None,
            expected_cardinality: Some(error.expected.clone()),
            actual_count: Some(error.actual_count),
            message: format!(
                "expected {} value(s), received {}",
                error.expected, error.actual_count
            ),
        })?;
    match outcome {
        CardinalityOutcome::Scalar(None) => Ok(CompiledValueResult::Scalar(None)),
        CardinalityOutcome::Scalar(Some(value)) => {
            apply_pipeline(TransformShape::Scalar(value), transforms, path)
        }
        CardinalityOutcome::Sequence(values) => {
            apply_pipeline(TransformShape::Sequence(values), transforms, path)
        }
    }
}
fn apply_pipeline<'doc, 'body>(
    shape: TransformShape<'doc, 'body>,
    transforms: &CompiledTransformPipeline,
    path: &str,
) -> Result<CompiledValueResult, ValueEvaluationError> {
    let output = transforms
        .execute(shape)
        .map_err(|error| ValueEvaluationError {
            kind: match error.kind {
                TransformErrorKind::TypeMismatch => ValueEvaluationErrorKind::TransformTypeMismatch,
                TransformErrorKind::InvalidPercentEncoding => {
                    ValueEvaluationErrorKind::TransformInvalidPercentEncoding
                }
                TransformErrorKind::InvalidUtf8 => ValueEvaluationErrorKind::TransformInvalidUtf8,
            },
            relative_path: path.to_string(),
            transform_index: Some(error.transform_index),
            value_index: error.value_index,
            expected_cardinality: None,
            actual_count: None,
            message: match error.kind {
                TransformErrorKind::TypeMismatch => "Value transform received an incompatible type",
                TransformErrorKind::InvalidPercentEncoding => {
                    "Value transform received malformed percent encoding"
                }
                TransformErrorKind::InvalidUtf8 => "Value transform decoded invalid UTF-8",
            }
            .to_string(),
        })?;
    let scalar = matches!(output, TransformShape::Scalar(_));
    let mut converted = Vec::new();
    for (index, value) in output.into_values().into_iter().enumerate() {
        let Some(value) = convert_scalar(value).map_err(|()| ValueEvaluationError {
            kind: ValueEvaluationErrorKind::TypeMismatch,
            relative_path: path.to_string(),
            transform_index: None,
            value_index: Some(index),
            expected_cardinality: None,
            actual_count: None,
            message: "Value must resolve to text or a scalar".to_string(),
        })?
        else {
            continue;
        };
        let normalized = normalize_whitespace_text(value.trim());
        if scalar || !normalized.is_empty() {
            converted.push(normalized);
        }
    }
    Ok(if scalar {
        CompiledValueResult::Scalar(converted.pop())
    } else {
        CompiledValueResult::Sequence(converted)
    })
}
fn convert_scalar(value: TransformValue<'_, '_>) -> Result<Option<String>, ()> {
    match value {
        TransformValue::Text(value) => Ok(Some(value)),
        TransformValue::Json(Value::String(value)) => Ok(Some(value)),
        TransformValue::Json(Value::Number(value)) => Ok(Some(value.to_string())),
        TransformValue::Json(Value::Bool(value)) => Ok(Some(value.to_string())),
        TransformValue::Json(Value::Null) => Ok(None),
        TransformValue::Json(Value::Array(_) | Value::Object(_)) => Err(()),
        TransformValue::Xml(node) => Ok(Some(xml_node_text(node))),
        TransformValue::Html(node) => Ok(Some(node.formatted_text().to_string())),
    }
}
fn eval_error(kind: ValueEvaluationErrorKind, path: &str, message: &str) -> ValueEvaluationError {
    ValueEvaluationError {
        kind,
        relative_path: path.to_string(),
        transform_index: None,
        value_index: None,
        expected_cardinality: None,
        actual_count: None,
        message: message.to_string(),
    }
}
