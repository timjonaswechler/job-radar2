use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::select::{SelectedItem, SelectedSequence};

mod dedupe;
mod html_to_text;
mod join;
mod normalize_whitespace;
mod regex_replace;
mod slug_to_title;
mod split;
mod to_string;
mod trim;
mod url_decode;

pub use dedupe::{Dedupe, DedupePlan};
pub use html_to_text::{HtmlToText, HtmlToTextPlan};
pub use join::{Join, JoinPlan};
pub use normalize_whitespace::normalize as normalize_whitespace_text;
pub use normalize_whitespace::{NormalizeWhitespace, NormalizeWhitespacePlan};
pub use regex_replace::{RegexReplace, RegexReplacePlan};
pub use slug_to_title::{SlugToTitle, SlugToTitlePlan};
pub use split::{Split, SplitPlan};
pub use to_string::{ToStringTransform, ToStringTransformPlan};
pub use trim::{Trim, TrimPlan};
pub use url_decode::{UrlDecode, UrlDecodePlan};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transform {
    Trim(Trim),
    NormalizeWhitespace(NormalizeWhitespace),
    HtmlToText(HtmlToText),
    UrlDecode(UrlDecode),
    SlugToTitle(SlugToTitle),
    Dedupe(Dedupe),
    ToString(ToStringTransform),
    Split(Split),
    Join(Join),
    RegexReplace(RegexReplace),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformKind {
    Trim,
    NormalizeWhitespace,
    HtmlToText,
    UrlDecode,
    SlugToTitle,
    Dedupe,
    ToString,
    Split,
    Join,
    RegexReplace,
}

impl TransformKind {
    pub const ALL: [Self; 10] = [
        Self::Trim,
        Self::NormalizeWhitespace,
        Self::HtmlToText,
        Self::UrlDecode,
        Self::SlugToTitle,
        Self::Dedupe,
        Self::ToString,
        Self::Split,
        Self::Join,
        Self::RegexReplace,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Trim => "trim",
            Self::NormalizeWhitespace => "normalize_whitespace",
            Self::HtmlToText => "html_to_text",
            Self::UrlDecode => "url_decode",
            Self::SlugToTitle => "slug_to_title",
            Self::Dedupe => "dedupe",
            Self::ToString => "to_string",
            Self::Split => "split",
            Self::Join => "join",
            Self::RegexReplace => "regex_replace",
        }
    }
}

impl Transform {
    pub const fn kind(&self) -> TransformKind {
        match self {
            Self::Trim(_) => TransformKind::Trim,
            Self::NormalizeWhitespace(_) => TransformKind::NormalizeWhitespace,
            Self::HtmlToText(_) => TransformKind::HtmlToText,
            Self::UrlDecode(_) => TransformKind::UrlDecode,
            Self::SlugToTitle(_) => TransformKind::SlugToTitle,
            Self::Dedupe(_) => TransformKind::Dedupe,
            Self::ToString(_) => TransformKind::ToString,
            Self::Split(_) => TransformKind::Split,
            Self::Join(_) => TransformKind::Join,
            Self::RegexReplace(_) => TransformKind::RegexReplace,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransformDescriptor {
    pub key: &'static str,
}
const TRANSFORM_DESCRIPTORS: [TransformDescriptor; 10] = [
    trim::DESCRIPTOR,
    normalize_whitespace::DESCRIPTOR,
    html_to_text::DESCRIPTOR,
    url_decode::DESCRIPTOR,
    slug_to_title::DESCRIPTOR,
    dedupe::DESCRIPTOR,
    to_string::DESCRIPTOR,
    split::DESCRIPTOR,
    join::DESCRIPTOR,
    regex_replace::DESCRIPTOR,
];
pub fn transform_descriptors() -> &'static [TransformDescriptor] {
    &TRANSFORM_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransformRegistryError {
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

pub fn validate_transform_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    registration_keys: &[String],
) -> Result<(), TransformRegistryError> {
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
            return Err(TransformRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let schema = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [("serde", serde_keys), ("registration", registration_keys)] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = schema.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(TransformRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&schema).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(TransformRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledTransform {
    Trim(TrimPlan),
    NormalizeWhitespace(NormalizeWhitespacePlan),
    HtmlToText(HtmlToTextPlan),
    UrlDecode(UrlDecodePlan),
    SlugToTitle(SlugToTitlePlan),
    Dedupe(DedupePlan),
    ToString(ToStringTransformPlan),
    Split(SplitPlan),
    Join(JoinPlan),
    RegexReplace(RegexReplacePlan),
}

impl CompiledTransform {
    pub const fn kind(&self) -> TransformKind {
        match self {
            Self::Trim(_) => TransformKind::Trim,
            Self::NormalizeWhitespace(_) => TransformKind::NormalizeWhitespace,
            Self::HtmlToText(_) => TransformKind::HtmlToText,
            Self::UrlDecode(_) => TransformKind::UrlDecode,
            Self::SlugToTitle(_) => TransformKind::SlugToTitle,
            Self::Dedupe(_) => TransformKind::Dedupe,
            Self::ToString(_) => TransformKind::ToString,
            Self::Split(_) => TransformKind::Split,
            Self::Join(_) => TransformKind::Join,
            Self::RegexReplace(_) => TransformKind::RegexReplace,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompiledTransformPipeline(Vec<CompiledTransform>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformShapeKind {
    Scalar,
    Sequence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTransformErrorKind {
    EmptySeparator,
    InvalidRegex,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTransformError {
    pub kind: CompileTransformErrorKind,
    pub transform_index: usize,
    pub message: String,
}
impl fmt::Display for CompileTransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at transform {}", self.message, self.transform_index)
    }
}
impl std::error::Error for CompileTransformError {}

pub fn compile_transform_pipeline(
    authored: &[Transform],
) -> Result<CompiledTransformPipeline, CompileTransformError> {
    let mut plans = Vec::with_capacity(authored.len());
    for (transform_index, transform) in authored.iter().enumerate() {
        let plan = match transform {
            crate::definition::primitives::transform::Transform::Trim(value) => {
                CompiledTransform::Trim(trim::compile(value))
            }
            crate::definition::primitives::transform::Transform::NormalizeWhitespace(value) => {
                CompiledTransform::NormalizeWhitespace(normalize_whitespace::compile(value))
            }
            crate::definition::primitives::transform::Transform::HtmlToText(value) => {
                CompiledTransform::HtmlToText(html_to_text::compile(value))
            }
            crate::definition::primitives::transform::Transform::UrlDecode(value) => {
                CompiledTransform::UrlDecode(url_decode::compile(value))
            }
            crate::definition::primitives::transform::Transform::SlugToTitle(value) => {
                CompiledTransform::SlugToTitle(slug_to_title::compile(value))
            }
            crate::definition::primitives::transform::Transform::Dedupe(value) => {
                CompiledTransform::Dedupe(dedupe::compile(value))
            }
            crate::definition::primitives::transform::Transform::ToString(value) => {
                CompiledTransform::ToString(to_string::compile(value))
            }
            crate::definition::primitives::transform::Transform::Split(value) => {
                CompiledTransform::Split(split::compile(value).map_err(|message| {
                    CompileTransformError {
                        kind: CompileTransformErrorKind::EmptySeparator,
                        transform_index,
                        message,
                    }
                })?)
            }
            crate::definition::primitives::transform::Transform::Join(value) => {
                CompiledTransform::Join(join::compile(value))
            }
            crate::definition::primitives::transform::Transform::RegexReplace(value) => {
                CompiledTransform::RegexReplace(regex_replace::compile(value).map_err(
                    |message| CompileTransformError {
                        kind: CompileTransformErrorKind::InvalidRegex,
                        transform_index,
                        message,
                    },
                )?)
            }
        };
        plans.push(plan);
    }
    Ok(CompiledTransformPipeline(plans))
}

#[derive(Clone)]
pub enum TransformValue<'doc, 'body> {
    Json(Value),
    Xml(roxmltree::Node<'doc, 'body>),
    Html(dom_query::NodeRef<'doc>),
    Text(String),
}
impl fmt::Debug for TransformValue<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(v) => f.debug_tuple("Json").field(v).finish(),
            Self::Xml(v) => f.debug_tuple("Xml").field(&v.tag_name().name()).finish(),
            Self::Html(_) => f.write_str("Html(..)"),
            Self::Text(v) => f.debug_tuple("Text").field(v).finish(),
        }
    }
}
impl<'doc, 'body> From<SelectedItem<'doc, 'body>> for TransformValue<'doc, 'body> {
    fn from(value: SelectedItem<'doc, 'body>) -> Self {
        match value {
            SelectedItem::Json(v) => Self::Json(v.clone()),
            SelectedItem::Xml(v) => Self::Xml(v),
            SelectedItem::Html(v) => Self::Html(v),
            SelectedItem::Text(v) => Self::Text(v),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TransformShape<'doc, 'body> {
    Scalar(TransformValue<'doc, 'body>),
    Sequence(Vec<TransformValue<'doc, 'body>>),
}
impl<'doc, 'body> TransformShape<'doc, 'body> {
    pub fn shape_name(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Sequence(_) => "sequence",
        }
    }
    pub fn into_values(self) -> Vec<TransformValue<'doc, 'body>> {
        match self {
            Self::Scalar(value) => vec![value],
            Self::Sequence(values) => values,
        }
    }
}
impl<'doc, 'body> From<SelectedSequence<'doc, 'body>> for TransformShape<'doc, 'body> {
    fn from(values: SelectedSequence<'doc, 'body>) -> Self {
        Self::Sequence(values.into_vec().into_iter().map(Into::into).collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformErrorKind {
    TypeMismatch,
    InvalidPercentEncoding,
    InvalidUtf8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransformError {
    pub kind: TransformErrorKind,
    pub transform_index: usize,
    pub value_index: Option<usize>,
    pub message: String,
}

impl CompiledTransformPipeline {
    pub fn output_shape(&self, mut shape: TransformShapeKind) -> TransformShapeKind {
        for transform in &self.0 {
            shape = match transform {
                CompiledTransform::Split(_) => TransformShapeKind::Sequence,
                CompiledTransform::Join(_) => TransformShapeKind::Scalar,
                _ => shape,
            };
        }
        shape
    }

    pub fn execute<'doc, 'body>(
        &self,
        mut shape: TransformShape<'doc, 'body>,
    ) -> Result<TransformShape<'doc, 'body>, TransformError> {
        for (transform_index, transform) in self.0.iter().enumerate() {
            shape = match transform {
                CompiledTransform::Trim(plan) => {
                    map_text(shape, transform_index, |v| Ok(trim::execute(plan, v)))?
                }
                CompiledTransform::NormalizeWhitespace(plan) => {
                    map_text(shape, transform_index, |v| {
                        Ok(normalize_whitespace::execute(plan, v))
                    })?
                }
                CompiledTransform::HtmlToText(plan) => map_text(shape, transform_index, |v| {
                    Ok(html_to_text::execute(plan, v))
                })?,
                CompiledTransform::UrlDecode(plan) => {
                    map_text(shape, transform_index, |v| url_decode::execute(plan, v))?
                }
                CompiledTransform::SlugToTitle(plan) => map_text(shape, transform_index, |v| {
                    Ok(slug_to_title::execute(plan, v))
                })?,
                CompiledTransform::Dedupe(plan) => dedupe::execute(plan, shape, transform_index)?,
                CompiledTransform::ToString(plan) => {
                    to_string::execute(plan, shape, transform_index)?
                }
                CompiledTransform::Split(plan) => split::execute(plan, shape, transform_index)?,
                CompiledTransform::Join(plan) => join::execute(plan, shape, transform_index)?,
                CompiledTransform::RegexReplace(plan) => map_text(shape, transform_index, |v| {
                    Ok(regex_replace::execute(plan, v))
                })?,
            };
        }
        Ok(shape)
    }
}

fn map_text<'doc, 'body, F>(
    shape: TransformShape<'doc, 'body>,
    transform_index: usize,
    mut apply: F,
) -> Result<TransformShape<'doc, 'body>, TransformError>
where
    F: FnMut(String) -> Result<String, TransformErrorKind>,
{
    let scalar = matches!(shape, TransformShape::Scalar(_));
    let mut output = Vec::new();
    for (value_index, value) in shape.into_values().into_iter().enumerate() {
        let TransformValue::Text(value) = value else {
            return Err(type_mismatch(transform_index, Some(value_index)));
        };
        let value = apply(value).map_err(|kind| TransformError {
            kind,
            transform_index,
            value_index: Some(value_index),
            message: error_message(kind).to_string(),
        })?;
        output.push(TransformValue::Text(value));
    }
    Ok(if scalar {
        TransformShape::Scalar(output.pop().expect("scalar transform has one value"))
    } else {
        TransformShape::Sequence(output)
    })
}

fn type_mismatch(transform_index: usize, value_index: Option<usize>) -> TransformError {
    TransformError {
        kind: TransformErrorKind::TypeMismatch,
        transform_index,
        value_index,
        message: error_message(TransformErrorKind::TypeMismatch).to_string(),
    }
}
fn error_message(kind: TransformErrorKind) -> &'static str {
    match kind {
        TransformErrorKind::TypeMismatch => "transform received an incompatible scalar type",
        TransformErrorKind::InvalidPercentEncoding => {
            "url_decode received malformed percent encoding"
        }
        TransformErrorKind::InvalidUtf8 => "url_decode decoded bytes are not valid UTF-8",
    }
}

fn authored_transform_shape(value: &Transform) -> (&'static str, &'static [&'static str]) {
    match value {
        crate::definition::primitives::transform::Transform::Trim(Trim {}) => ("trim", &[]),
        crate::definition::primitives::transform::Transform::NormalizeWhitespace(
            NormalizeWhitespace {},
        ) => ("normalize_whitespace", &[]),
        crate::definition::primitives::transform::Transform::HtmlToText(HtmlToText {}) => {
            ("html_to_text", &[])
        }
        crate::definition::primitives::transform::Transform::UrlDecode(UrlDecode {}) => {
            ("url_decode", &[])
        }
        crate::definition::primitives::transform::Transform::SlugToTitle(SlugToTitle {}) => {
            ("slug_to_title", &[])
        }
        crate::definition::primitives::transform::Transform::Dedupe(Dedupe {}) => ("dedupe", &[]),
        crate::definition::primitives::transform::Transform::ToString(ToStringTransform {}) => {
            ("to_string", &[])
        }
        crate::definition::primitives::transform::Transform::Split(Split {
            separator: _,
            trim_parts: _,
            drop_empty: _,
        }) => ("split", &["separator", "trimParts", "dropEmpty"]),
        crate::definition::primitives::transform::Transform::Join(Join { separator: _ }) => {
            ("join", &["separator"])
        }
        crate::definition::primitives::transform::Transform::RegexReplace(RegexReplace {
            pattern: _,
            replacement: _,
        }) => ("regex_replace", &["pattern", "replacement"]),
    }
}

pub fn completeness_serde_shapes() -> Vec<crate::definition::primitives::completeness::SerdeShape> {
    use crate::definition::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::{ParentOption, Tagged},
        Family::Transform,
        PrimitiveContext::{Detail, Discovery},
    };
    let fixtures = [
        crate::definition::primitives::transform::Transform::Trim(Trim {}),
        crate::definition::primitives::transform::Transform::NormalizeWhitespace(
            NormalizeWhitespace {},
        ),
        crate::definition::primitives::transform::Transform::HtmlToText(HtmlToText {}),
        crate::definition::primitives::transform::Transform::UrlDecode(UrlDecode {}),
        crate::definition::primitives::transform::Transform::SlugToTitle(SlugToTitle {}),
        crate::definition::primitives::transform::Transform::Dedupe(Dedupe {}),
        crate::definition::primitives::transform::Transform::ToString(ToStringTransform {}),
        crate::definition::primitives::transform::Transform::Split(Split {
            separator: ",".into(),
            trim_parts: true,
            drop_empty: true,
        }),
        crate::definition::primitives::transform::Transform::Join(Join {
            separator: ",".into(),
        }),
        crate::definition::primitives::transform::Transform::RegexReplace(RegexReplace {
            pattern: "x".into(),
            replacement: "y".into(),
        }),
    ];
    let mut out = Vec::new();
    for value in &fixtures {
        let (key, options) = authored_transform_shape(value);
        out.push(serde_shape(
            Transform,
            key,
            &[Discovery, Detail],
            Tagged,
            "src-tauri/crates/source-engine/src/definition/documents/extract.rs",
        ));
        for option in options {
            out.push(serde_shape(
                Transform,
                format!("{key}.{option}"),
                &[Discovery, Detail],
                ParentOption,
                "src-tauri/crates/source-engine/src/definition/documents/extract.rs",
            ));
        }
    }
    out
}

macro_rules! transform_variant_witness {
    ($name:ident, $variant:ident) => {
        fn $name() {
            fn check(v: &CompiledTransform) {
                if let CompiledTransform::$variant(plan) = v {
                    let _ = plan;
                }
            }
            let _ = check as fn(&CompiledTransform);
        }
    };
}
transform_variant_witness!(witness_transform_trim, Trim);
transform_variant_witness!(witness_transform_normalize, NormalizeWhitespace);
transform_variant_witness!(witness_transform_html, HtmlToText);
transform_variant_witness!(witness_transform_url, UrlDecode);
transform_variant_witness!(witness_transform_slug, SlugToTitle);
transform_variant_witness!(witness_transform_dedupe, Dedupe);
transform_variant_witness!(witness_transform_string, ToString);
transform_variant_witness!(witness_transform_split, Split);
transform_variant_witness!(witness_transform_join, Join);
transform_variant_witness!(witness_transform_regex, RegexReplace);
fn witness_split_separator() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::Split(p) = v {
            let _ = &p.separator;
        }
    }
    let _ = check as fn(&CompiledTransform);
}
fn witness_split_trim() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::Split(p) = v {
            let _ = &p.trim_parts;
        }
    }
    let _ = check as fn(&CompiledTransform);
}
fn witness_split_drop() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::Split(p) = v {
            let _ = &p.drop_empty;
        }
    }
    let _ = check as fn(&CompiledTransform);
}
fn witness_join_separator() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::Join(p) = v {
            let _ = &p.separator;
        }
    }
    let _ = check as fn(&CompiledTransform);
}
fn witness_regex_pattern() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::RegexReplace(p) = v {
            let _ = &p.pattern;
        }
    }
    let _ = check as fn(&CompiledTransform);
}
fn witness_regex_replacement() {
    fn check(v: &CompiledTransform) {
        if let CompiledTransform::RegexReplace(p) = v {
            let _ = &p.replacement;
        }
    }
    let _ = check as fn(&CompiledTransform);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::definition::primitives::completeness::CompiledRegistration> {
    use crate::definition::primitives::completeness::{
        AuthoredShapeKind::{ParentOption, Tagged},
        CompiledRegistration,
        Family::Transform,
        Owner::P05,
        PrimitiveContext::{Detail, Discovery},
    };
    macro_rules! row {
        ($key:literal,$variant:literal,$file:literal,$shape:expr,$witness:expr) => {
            CompiledRegistration {
                family: Transform,
                key: $key,
                contexts: &[Discovery, Detail],
                owner: P05,
                canonical_file: concat!(
                    "src-tauri/crates/source-engine/src/definition/primitives/transform/",
                    $file,
                    ".rs"
                ),
                shape: $shape,
                compiled_identity: concat!("CompiledTransform::", $variant),
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    vec![
        row!("trim", "Trim", "trim", Tagged, witness_transform_trim),
        row!(
            "normalize_whitespace",
            "NormalizeWhitespace",
            "normalize_whitespace",
            Tagged,
            witness_transform_normalize
        ),
        row!(
            "html_to_text",
            "HtmlToText",
            "html_to_text",
            Tagged,
            witness_transform_html
        ),
        row!(
            "url_decode",
            "UrlDecode",
            "url_decode",
            Tagged,
            witness_transform_url
        ),
        row!(
            "slug_to_title",
            "SlugToTitle",
            "slug_to_title",
            Tagged,
            witness_transform_slug
        ),
        row!(
            "dedupe",
            "Dedupe",
            "dedupe",
            Tagged,
            witness_transform_dedupe
        ),
        row!(
            "to_string",
            "ToString",
            "to_string",
            Tagged,
            witness_transform_string
        ),
        row!("split", "Split", "split", Tagged, witness_transform_split),
        row!("join", "Join", "join", Tagged, witness_transform_join),
        row!(
            "regex_replace",
            "RegexReplace",
            "regex_replace",
            Tagged,
            witness_transform_regex
        ),
        row!(
            "split.separator",
            "Split.separator",
            "split",
            ParentOption,
            witness_split_separator
        ),
        row!(
            "split.trimParts",
            "Split.trim_parts",
            "split",
            ParentOption,
            witness_split_trim
        ),
        row!(
            "split.dropEmpty",
            "Split.drop_empty",
            "split",
            ParentOption,
            witness_split_drop
        ),
        row!(
            "join.separator",
            "Join.separator",
            "join",
            ParentOption,
            witness_join_separator
        ),
        row!(
            "regex_replace.pattern",
            "RegexReplace.pattern",
            "regex_replace",
            ParentOption,
            witness_regex_pattern
        ),
        row!(
            "regex_replace.replacement",
            "RegexReplace.replacement",
            "regex_replace",
            ParentOption,
            witness_regex_replacement
        ),
    ]
}
