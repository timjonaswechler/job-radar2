use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::{documents::ParseType, primitives::parse::ParsedDocument};

pub mod css;
mod document;
pub mod json_path;
mod sitemap_urls;
pub mod xml_element;
pub mod xml_text;

pub use css::{CssSelect, CssSelectPlan};
pub use document::{DocumentSelect, DocumentSelectPlan};
pub use json_path::{JsonPathSelect, JsonPathSelectPlan};

pub fn compile_json_path(path: &str) -> Result<JsonPathSelectPlan, String> {
    json_path::compile(path)
}

pub fn resolve_compiled_json_path<'a>(
    plan: &JsonPathSelectPlan,
    root: &'a Value,
) -> Option<&'a Value> {
    json_path::resolve_compiled(plan, root)
}
pub use sitemap_urls::{SitemapUrlsSelect, SitemapUrlsSelectPlan};
pub use xml_element::{XmlElementSelect, XmlElementSelectPlan};
pub use xml_text::{XmlTextSelect, XmlTextSelectPlan};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Select {
    Document(DocumentSelect),
    JsonPath(JsonPathSelect),
    XmlElement(XmlElementSelect),
    XmlText(XmlTextSelect),
    Css(CssSelect),
    SitemapUrls(SitemapUrlsSelect),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectPhase {
    Discovery,
    Detail,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectPlacement {
    Strategy,
    SitemapChild,
    SitemapPosting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectCompileContext {
    pub document_type: ParseType,
    pub phase: SelectPhase,
    pub placement: SelectPlacement,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectKind {
    Document,
    JsonPath,
    XmlElement,
    XmlText,
    Css,
    SitemapUrls,
}

impl Select {
    pub const fn kind(&self) -> SelectKind {
        select_kind(self)
    }
}

impl SelectKind {
    pub const ALL: [Self; 6] = [
        Self::Document,
        Self::JsonPath,
        Self::XmlElement,
        Self::XmlText,
        Self::Css,
        Self::SitemapUrls,
    ];
    pub const fn key(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::JsonPath => "json_path",
            Self::XmlElement => "xml_element",
            Self::XmlText => "xml_text",
            Self::Css => "css",
            Self::SitemapUrls => "sitemap_urls",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectDescriptor {
    pub key: &'static str,
}
const SELECT_DESCRIPTORS: [SelectDescriptor; 6] = [
    document::DESCRIPTOR,
    json_path::DESCRIPTOR,
    xml_element::DESCRIPTOR,
    xml_text::DESCRIPTOR,
    css::DESCRIPTOR,
    sitemap_urls::DESCRIPTOR,
];
pub fn select_descriptors() -> &'static [SelectDescriptor] {
    &SELECT_DESCRIPTORS
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectRegistryError {
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

pub fn validate_select_registration_keys(
    schema_keys: &[String],
    serde_keys: &[String],
    fragment_keys: &[String],
    registration_keys: &[String],
) -> Result<(), SelectRegistryError> {
    for (layer, keys) in [
        ("schema", schema_keys),
        ("serde", serde_keys),
        ("fragment", fragment_keys),
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
            return Err(SelectRegistryError::Duplicate {
                layer,
                keys: duplicates,
            });
        }
    }
    let schema = schema_keys.iter().cloned().collect::<BTreeSet<_>>();
    for (layer, keys) in [
        ("serde", serde_keys),
        ("fragment", fragment_keys),
        ("registration", registration_keys),
    ] {
        let actual = keys.iter().cloned().collect::<BTreeSet<_>>();
        let missing = schema.difference(&actual).cloned().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(SelectRegistryError::Missing {
                layer,
                keys: missing,
            });
        }
        let extra = actual.difference(&schema).cloned().collect::<Vec<_>>();
        if !extra.is_empty() {
            return Err(SelectRegistryError::Extra { layer, keys: extra });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompiledSelect {
    Document(DocumentSelectPlan),
    JsonPath(JsonPathSelectPlan),
    XmlElement(XmlElementSelectPlan),
    XmlText(XmlTextSelectPlan),
    Css(CssSelectPlan),
    SitemapUrls(SitemapUrlsSelectPlan),
}

impl CompiledSelect {
    pub const fn kind(&self) -> SelectKind {
        match self {
            Self::Document(_) => SelectKind::Document,
            Self::JsonPath(_) => SelectKind::JsonPath,
            Self::XmlElement(_) => SelectKind::XmlElement,
            Self::XmlText(_) => SelectKind::XmlText,
            Self::Css(_) => SelectKind::Css,
            Self::SitemapUrls(_) => SelectKind::SitemapUrls,
        }
    }

    pub fn select<'doc, 'body>(
        &self,
        document: &'doc ParsedDocument<'body>,
    ) -> Result<SelectedSequence<'doc, 'body>, SelectExecutionError> {
        match (self, document) {
            (Self::Document(plan), document) => Ok(document::execute(plan, document)),
            (Self::JsonPath(plan), ParsedDocument::Json(value)) => {
                Ok(json_path::execute(plan, value))
            }
            (Self::XmlElement(plan), ParsedDocument::Xml(value)) => {
                Ok(xml_element::execute(plan, value.root_element()))
            }
            (Self::XmlText(plan), ParsedDocument::Xml(value)) => {
                Ok(xml_text::execute(plan, value.root_element()))
            }
            (Self::Css(plan), ParsedDocument::Html(value)) => Ok(css::execute(plan, value)),
            (Self::SitemapUrls(plan), ParsedDocument::Xml(value)) => {
                Ok(sitemap_urls::execute(plan, value.root_element()))
            }
            _ => Err(SelectExecutionError {
                message: "compiled Select received an incompatible Parsed Document".to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileSelectErrorKind {
    Syntax,
    DocumentIncompatible,
    Placement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileSelectError {
    pub kind: CompileSelectErrorKind,
    pub message: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectExecutionError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedDocumentType {
    Any,
    Json,
    Xml,
    Html,
}

pub const fn selected_document_is_compatible(
    document_type: ParseType,
    required: SelectedDocumentType,
) -> bool {
    matches!(
        (document_type, required),
        (_, SelectedDocumentType::Any)
            | (ParseType::Json, SelectedDocumentType::Json)
            | (ParseType::Xml, SelectedDocumentType::Xml)
            | (ParseType::Html, SelectedDocumentType::Html)
    )
}

pub fn compile_select(
    authored: &Select,
    context: SelectCompileContext,
) -> Result<CompiledSelect, CompileSelectError> {
    let kind = select_kind(authored);
    let required_document = match kind {
        SelectKind::Document => SelectedDocumentType::Any,
        SelectKind::JsonPath => SelectedDocumentType::Json,
        SelectKind::XmlElement | SelectKind::XmlText | SelectKind::SitemapUrls => {
            SelectedDocumentType::Xml
        }
        SelectKind::Css => SelectedDocumentType::Html,
    };
    let compatible = selected_document_is_compatible(context.document_type, required_document);
    if !compatible {
        return Err(error(
            CompileSelectErrorKind::DocumentIncompatible,
            format!(
                "parse type `{}` is not compatible with select type `{}`",
                context.document_type.key(),
                kind.key()
            ),
        ));
    }
    match (kind, context.phase, context.placement) {
        (SelectKind::SitemapUrls, SelectPhase::Discovery, SelectPlacement::SitemapChild | SelectPlacement::SitemapPosting) => {}
        (SelectKind::SitemapUrls, _, _) => return Err(error(CompileSelectErrorKind::Placement, "sitemap_urls is valid only in XML Discovery sitemap child or posting selector placement")),
        (_, _, SelectPlacement::Strategy) => {}
        (_, _, _) => return Err(error(CompileSelectErrorKind::Placement, "only sitemap_urls is valid in a sitemap selector placement")),
    }
    let compiled = match authored {
        crate::profile_dsl::primitives::select::Select::Document(_) => {
            CompiledSelect::Document(DocumentSelectPlan)
        }
        crate::profile_dsl::primitives::select::Select::JsonPath(authored) => {
            CompiledSelect::JsonPath(json_path::compile(&authored.json_path).map_err(syntax_error)?)
        }
        crate::profile_dsl::primitives::select::Select::XmlElement(authored) => {
            CompiledSelect::XmlElement(
                xml_element::compile(&authored.element).map_err(syntax_error)?,
            )
        }
        crate::profile_dsl::primitives::select::Select::XmlText(authored) => {
            CompiledSelect::XmlText(xml_text::compile(&authored.text_path).map_err(syntax_error)?)
        }
        crate::profile_dsl::primitives::select::Select::Css(authored) => {
            CompiledSelect::Css(css::compile(&authored.selector).map_err(syntax_error)?)
        }
        crate::profile_dsl::primitives::select::Select::SitemapUrls(authored) => {
            CompiledSelect::SitemapUrls(
                sitemap_urls::compile(authored.url_pattern.as_deref()).map_err(syntax_error)?,
            )
        }
    };
    Ok(compiled)
}

fn syntax_error(message: impl Into<String>) -> CompileSelectError {
    error(CompileSelectErrorKind::Syntax, message)
}

fn error(kind: CompileSelectErrorKind, message: impl Into<String>) -> CompileSelectError {
    CompileSelectError {
        kind,
        message: message.into(),
    }
}
const fn select_kind(select: &Select) -> SelectKind {
    match select {
        crate::profile_dsl::primitives::select::Select::Document(_) => SelectKind::Document,
        crate::profile_dsl::primitives::select::Select::JsonPath(_) => SelectKind::JsonPath,
        crate::profile_dsl::primitives::select::Select::XmlElement(_) => SelectKind::XmlElement,
        crate::profile_dsl::primitives::select::Select::XmlText(_) => SelectKind::XmlText,
        crate::profile_dsl::primitives::select::Select::Css(_) => SelectKind::Css,
        crate::profile_dsl::primitives::select::Select::SitemapUrls(_) => SelectKind::SitemapUrls,
    }
}

#[derive(Clone)]
pub enum SelectedItem<'doc, 'body> {
    Json(&'doc Value),
    Xml(roxmltree::Node<'doc, 'body>),
    Html(dom_query::NodeRef<'doc>),
    Text(String),
}

impl std::fmt::Debug for SelectedItem<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(v) => f.debug_tuple("Json").field(v).finish(),
            Self::Xml(v) => f.debug_tuple("Xml").field(&v.tag_name().name()).finish(),
            Self::Html(_) => f.write_str("Html(..)"),
            Self::Text(v) => f.debug_tuple("Text").field(v).finish(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SelectedSequence<'doc, 'body>(Vec<SelectedItem<'doc, 'body>>);
impl<'doc, 'body> SelectedSequence<'doc, 'body> {
    pub fn new(items: Vec<SelectedItem<'doc, 'body>>) -> Self {
        Self(items)
    }
    pub fn one(item: SelectedItem<'doc, 'body>) -> Self {
        Self(vec![item])
    }
    pub fn as_slice(&self) -> &[SelectedItem<'doc, 'body>] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn into_vec(self) -> Vec<SelectedItem<'doc, 'body>> {
        self.0
    }
}

fn authored_select_shape(value: &Select) -> (&'static str, &'static [&'static str]) {
    match value {
        crate::profile_dsl::primitives::select::Select::Document(DocumentSelect {}) => {
            ("document", &[])
        }
        crate::profile_dsl::primitives::select::Select::JsonPath(JsonPathSelect {
            json_path: _,
        }) => ("json_path", &["jsonPath"]),
        crate::profile_dsl::primitives::select::Select::XmlElement(XmlElementSelect {
            element: _,
        }) => ("xml_element", &["element"]),
        crate::profile_dsl::primitives::select::Select::XmlText(XmlTextSelect { text_path: _ }) => {
            ("xml_text", &["textPath"])
        }
        crate::profile_dsl::primitives::select::Select::Css(CssSelect { selector: _ }) => {
            ("css", &["selector"])
        }
        crate::profile_dsl::primitives::select::Select::SitemapUrls(SitemapUrlsSelect {
            url_pattern: _,
        }) => ("sitemap_urls", &["urlPattern"]),
    }
}

pub fn completeness_serde_shapes() -> Vec<crate::profile_dsl::primitives::completeness::SerdeShape>
{
    use crate::profile_dsl::primitives::completeness::{
        serde_shape,
        AuthoredShapeKind::Tagged,
        Family::Select,
        PrimitiveContext::{Detail, Discovery},
    };
    let fixtures = [
        crate::profile_dsl::primitives::select::Select::Document(DocumentSelect {}),
        crate::profile_dsl::primitives::select::Select::JsonPath(JsonPathSelect {
            json_path: "x".into(),
        }),
        crate::profile_dsl::primitives::select::Select::XmlElement(XmlElementSelect {
            element: "x".into(),
        }),
        crate::profile_dsl::primitives::select::Select::XmlText(XmlTextSelect {
            text_path: "x".into(),
        }),
        crate::profile_dsl::primitives::select::Select::Css(CssSelect {
            selector: "x".into(),
        }),
        crate::profile_dsl::primitives::select::Select::SitemapUrls(SitemapUrlsSelect {
            url_pattern: Some("x".into()),
        }),
    ];
    let mut out = Vec::new();
    for value in &fixtures {
        let (key, options) = authored_select_shape(value);
        out.push(serde_shape(
            Select,
            key,
            &[Discovery, Detail],
            Tagged,
            "src-tauri/crates/source-profile-dsl/src/profile_dsl/documents/select.rs",
        ));
        for option in options {
            out.push(serde_shape(
                Select,
                format!("{key}.{option}"),
                &[Discovery, Detail],
                crate::profile_dsl::primitives::completeness::AuthoredShapeKind::ParentOption,
                "src-tauri/crates/source-profile-dsl/src/profile_dsl/documents/select.rs",
            ));
        }
    }
    out
}

fn witness_select_document() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::Document(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_json_path() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::JsonPath(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_json_path_path() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::JsonPath(plan) = v {
            let _ = &plan.segments;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_xml_element() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::XmlElement(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_xml_element_element() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::XmlElement(plan) = v {
            let _ = &plan.element;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_xml_text() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::XmlText(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_xml_text_path() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::XmlText(plan) = v {
            let _ = (&plan.current, &plan.segments);
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_css() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::Css(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_css_selector() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::Css(plan) = v {
            let _ = &plan.selector;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_sitemap() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::SitemapUrls(plan) = v {
            let _ = plan;
        }
    }
    let _ = check as fn(&CompiledSelect);
}
fn witness_select_sitemap_pattern() {
    fn check(v: &CompiledSelect) {
        if let CompiledSelect::SitemapUrls(plan) = v {
            let _ = &plan.url_pattern;
        }
    }
    let _ = check as fn(&CompiledSelect);
}

pub fn completeness_compiled_registrations(
) -> Vec<crate::profile_dsl::primitives::completeness::CompiledRegistration> {
    use crate::profile_dsl::primitives::completeness::{
        AuthoredShapeKind::Tagged,
        CompiledRegistration,
        Family::Select,
        Owner::P03,
        PrimitiveContext::{Detail, Discovery},
    };
    macro_rules! row {
        ($key:literal,$variant:literal,$file:literal,$witness:expr) => {
            CompiledRegistration {
                family: Select,
                key: $key,
                contexts: &[Discovery, Detail],
                owner: P03,
                canonical_file: concat!(
                    "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/select/",
                    $file,
                    ".rs"
                ),
                shape: Tagged,
                compiled_identity: concat!("CompiledSelect::", $variant),
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    let mut out = vec![
        row!("document", "Document", "document", witness_select_document),
        row!(
            "json_path",
            "JsonPath",
            "json_path",
            witness_select_json_path
        ),
        row!(
            "xml_element",
            "XmlElement",
            "xml_element",
            witness_select_xml_element
        ),
        row!("xml_text", "XmlText", "xml_text", witness_select_xml_text),
        row!("css", "Css", "css", witness_select_css),
        row!(
            "sitemap_urls",
            "SitemapUrls",
            "sitemap_urls",
            witness_select_sitemap
        ),
    ];
    macro_rules! option {
        ($key:literal,$identity:literal,$file:literal,$witness:expr) => {
            CompiledRegistration {
                family: Select,
                key: $key,
                contexts: &[Discovery, Detail],
                owner: P03,
                canonical_file: concat!(
                    "src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/select/",
                    $file,
                    ".rs"
                ),
                shape:
                    crate::profile_dsl::primitives::completeness::AuthoredShapeKind::ParentOption,
                compiled_identity: $identity,
                witness: $witness,
                behavior_bearing: false,
            }
        };
    }
    out.extend([
        option!(
            "json_path.jsonPath",
            "CompiledSelect::JsonPath.segments",
            "json_path",
            witness_select_json_path_path
        ),
        option!(
            "xml_element.element",
            "CompiledSelect::XmlElement.element",
            "xml_element",
            witness_select_xml_element_element
        ),
        option!(
            "xml_text.textPath",
            "CompiledSelect::XmlText.{current,segments}",
            "xml_text",
            witness_select_xml_text_path
        ),
        option!(
            "css.selector",
            "CompiledSelect::Css.selector",
            "css",
            witness_select_css_selector
        ),
        option!(
            "sitemap_urls.urlPattern",
            "CompiledSelect::SitemapUrls.url_pattern",
            "sitemap_urls",
            witness_select_sitemap_pattern
        ),
    ]);
    out
}
