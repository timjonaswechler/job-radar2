use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    documents::{FieldExpression, ParseType},
    primitives::{
        cardinality::Cardinality,
        select::{
            selected_document_is_compatible, SelectedDocumentType, SelectedItem, SelectedSequence,
        },
        transform::{compile_transform_pipeline, CompileTransformErrorKind},
    },
    template::{compile_template_all, TemplateCompileErrorKind, TemplateDescriptor},
};

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

    const fn admits_selected(self) -> bool {
        self.descriptor().selected_item
    }

    const fn admits_posting(self) -> bool {
        self.descriptor().posting
    }

    const fn admits_captures(self) -> bool {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledValueFoundation {
    pub placement: ValuePlacement,
    pub shape: ValueShape,
    pub node_count: usize,
    pub max_depth: usize,
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

pub fn compile_value_foundation(
    expression: &FieldExpression,
    context: &ValueCompileContext,
) -> Result<CompiledValueFoundation, ValueCompileError> {
    let mut stats = Stats::default();
    compile_node(expression, context, "", 1, &mut stats)?;
    Ok(CompiledValueFoundation {
        placement: context.placement,
        shape: expression_shape(expression),
        node_count: stats.nodes,
        max_depth: stats.max_depth,
    })
}

#[derive(Default)]
struct Stats {
    nodes: usize,
    max_depth: usize,
}

fn compile_node(
    expression: &FieldExpression,
    context: &ValueCompileContext,
    path: &str,
    depth: usize,
    stats: &mut Stats,
) -> Result<(), ValueCompileError> {
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
    if let Some(transforms) = expression.transforms() {
        if let Err(transform_error) = compile_transform_pipeline(transforms) {
            let kind = match transform_error.kind {
                CompileTransformErrorKind::EmptySeparator => {
                    ValueCompileErrorKind::TransformEmptySeparator
                }
                CompileTransformErrorKind::InvalidRegex => {
                    ValueCompileErrorKind::TransformInvalidRegex
                }
            };
            return Err(error(
                kind,
                &format!("{path}/transforms/{}", transform_error.transform_index),
                &transform_error.message,
            ));
        }
    }

    match expression {
        FieldExpression::Const { .. } => {}
        FieldExpression::Template { template, .. } => {
            let descriptor = template_descriptor(context);
            if let Err(errors) = compile_template_all(template, &descriptor) {
                let transform_pipe = errors
                    .iter()
                    .any(|error| error.kind == TemplateCompileErrorKind::TransformPipeUnsupported);
                return Err(error(
                    if transform_pipe {
                        ValueCompileErrorKind::TemplateTransformPipe
                    } else {
                        ValueCompileErrorKind::Template
                    },
                    &member_path(path, "template"),
                    if transform_pipe {
                        "Template transform pipes are unsupported; use transforms[]"
                    } else {
                        "Value template references unavailable context"
                    },
                ));
            }
        }
        FieldExpression::SourceConfig { key, .. } => {
            if !context.source_config_keys.contains(key) {
                return Err(error(
                    ValueCompileErrorKind::UnknownSourceConfigKey,
                    path,
                    "Source Config key is not declared by the Effective Source Profile",
                ));
            }
        }
        FieldExpression::PostingMeta { key, .. } => {
            if !context.placement.admits_posting() {
                return Err(error(
                    ValueCompileErrorKind::PostingMetaUnavailable,
                    path,
                    "postingMeta is unavailable at this Value placement",
                ));
            }
            if !context.posting_meta_keys.contains(key) {
                return Err(error(
                    ValueCompileErrorKind::UnknownPostingMetaKey,
                    path,
                    "postingMeta key is not declared by Discovery",
                ));
            }
        }
        FieldExpression::Capture { key, .. } => {
            if !context.placement.admits_captures() {
                return Err(error(
                    ValueCompileErrorKind::CaptureUnavailable,
                    path,
                    "captures are unavailable at this Value placement",
                ));
            }
            if !context.capture_keys.contains(key) {
                return Err(error(
                    ValueCompileErrorKind::UnknownCaptureKey,
                    path,
                    "capture key is not declared by the Strategy",
                ));
            }
        }
        FieldExpression::ItemField { .. } => require_selected(context, path)?,
        FieldExpression::JsonPath { .. } => {
            require_document(context, path, ParseType::Json, "json_path")?
        }
        FieldExpression::XmlText { .. } | FieldExpression::XmlElement { .. } => {
            require_document(context, path, ParseType::Xml, "xml")?
        }
        FieldExpression::CssText { .. } | FieldExpression::CssAttribute { .. } => {
            require_document(context, path, ParseType::Html, "css")?
        }
        FieldExpression::Combine { parts, .. } => {
            for (index, part) in parts.iter().enumerate() {
                compile_node(
                    &part.value,
                    context,
                    &format!("{path}/parts/{index}/value"),
                    depth + 1,
                    stats,
                )?;
            }
        }
        FieldExpression::FirstNonEmpty { candidates, .. } => {
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
            for (index, candidate) in candidates.iter().enumerate() {
                compile_node(
                    candidate,
                    context,
                    &format!("{path}/candidates/{index}"),
                    depth + 1,
                    stats,
                )?;
            }
        }
    }
    Ok(())
}

fn require_selected(context: &ValueCompileContext, path: &str) -> Result<(), ValueCompileError> {
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

fn require_document(
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

fn template_descriptor(context: &ValueCompileContext) -> TemplateDescriptor {
    let mut descriptor = TemplateDescriptor::new()
        .allow_namespace("sourceConfig", context.source_config_keys.iter().cloned())
        .allow_namespace("source", ["name"]);
    if context.placement.admits_posting() {
        descriptor = descriptor
            .allow_namespace("postingMeta", context.posting_meta_keys.iter().cloned())
            .allow_namespace(
                "posting",
                ["title", "company", "url", "locations", "descriptionText"],
            );
    }
    if context.placement.admits_captures() {
        descriptor = descriptor.allow_namespace("captures", context.capture_keys.iter().cloned());
    }
    descriptor
}

fn expression_shape(expression: &FieldExpression) -> ValueShape {
    let cardinality = match expression {
        FieldExpression::Const { cardinality, .. }
        | FieldExpression::Template { cardinality, .. }
        | FieldExpression::SourceConfig { cardinality, .. }
        | FieldExpression::PostingMeta { cardinality, .. }
        | FieldExpression::Capture { cardinality, .. }
        | FieldExpression::ItemField { cardinality, .. }
        | FieldExpression::JsonPath { cardinality, .. }
        | FieldExpression::XmlText { cardinality, .. }
        | FieldExpression::XmlElement { cardinality, .. }
        | FieldExpression::CssText { cardinality, .. }
        | FieldExpression::CssAttribute { cardinality, .. }
        | FieldExpression::Combine { cardinality, .. }
        | FieldExpression::FirstNonEmpty { cardinality, .. } => cardinality.unwrap_or_default(),
    };
    if matches!(cardinality, Cardinality::All(_)) {
        ValueShape::Sequence
    } else {
        ValueShape::Scalar
    }
}

fn member_path(path: &str, member: &str) -> String {
    if path.is_empty() {
        format!("/{member}")
    } else {
        format!("{path}/{member}")
    }
}

fn error(kind: ValueCompileErrorKind, path: &str, message: &str) -> ValueCompileError {
    ValueCompileError {
        kind,
        path: path.to_string(),
        actual: None,
        maximum: None,
        message: message.to_string(),
    }
}

fn limit_error(
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
    pub source_config: &'a BTreeMap<String, serde_json::Value>,
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
