use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::{documents::ParseType, primitives::parse::ParsedDocument};

mod css;
mod document;
mod json_path;
mod sitemap_urls;
mod xml_element;
mod xml_text;

pub use css::{CssSelect, CssSelectPlan};
pub use document::{DocumentSelect, DocumentSelectPlan};
pub(crate) use json_path::resolve_authored_json_path;
pub use json_path::{JsonPathSelect, JsonPathSelectPlan};
pub use sitemap_urls::{SitemapUrlsSelect, SitemapUrlsSelectPlan};
pub(crate) use xml_element::descendant_elements as xml_descendant_elements;
pub use xml_element::{XmlElementSelect, XmlElementSelectPlan};
pub(crate) use xml_text::{node_text as xml_node_text, path_texts as xml_path_texts};
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

pub fn compile_select(
    authored: &Select,
    context: SelectCompileContext,
) -> Result<CompiledSelect, CompileSelectError> {
    let kind = select_kind(authored);
    let compatible = matches!(
        (context.document_type, kind),
        (_, SelectKind::Document)
            | (ParseType::Json, SelectKind::JsonPath)
            | (
                ParseType::Xml,
                SelectKind::XmlElement | SelectKind::XmlText | SelectKind::SitemapUrls
            )
            | (ParseType::Html, SelectKind::Css)
    );
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
        Select::Document(_) => CompiledSelect::Document(DocumentSelectPlan),
        Select::JsonPath(authored) => {
            CompiledSelect::JsonPath(json_path::compile(&authored.json_path).map_err(syntax_error)?)
        }
        Select::XmlElement(authored) => CompiledSelect::XmlElement(
            xml_element::compile(&authored.element).map_err(syntax_error)?,
        ),
        Select::XmlText(authored) => {
            CompiledSelect::XmlText(xml_text::compile(&authored.text_path).map_err(syntax_error)?)
        }
        Select::Css(authored) => {
            CompiledSelect::Css(css::compile(&authored.selector).map_err(syntax_error)?)
        }
        Select::SitemapUrls(authored) => CompiledSelect::SitemapUrls(
            sitemap_urls::compile(authored.url_pattern.as_deref()).map_err(syntax_error)?,
        ),
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
        Select::Document(_) => SelectKind::Document,
        Select::JsonPath(_) => SelectKind::JsonPath,
        Select::XmlElement(_) => SelectKind::XmlElement,
        Select::XmlText(_) => SelectKind::XmlText,
        Select::Css(_) => SelectKind::Css,
        Select::SitemapUrls(_) => SelectKind::SitemapUrls,
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
