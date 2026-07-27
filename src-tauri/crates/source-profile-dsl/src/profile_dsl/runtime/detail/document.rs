use super::*;
use crate::profile_dsl::primitives::{
    parse::ParsedDocument,
    select::{CompiledSelect, SelectKind, SelectedItem},
};

#[derive(Clone)]
pub(super) enum RuntimeItem<'doc, 'body> {
    Json(&'doc Value),
    Xml(roxmltree::Node<'doc, 'body>),
    XmlCollection(Vec<roxmltree::Node<'doc, 'body>>),
    Html(NodeRef<'doc>),
    Text(String),
}

pub(super) fn select_detail_document<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &CompiledSelect,
    allow_collection: bool,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<RuntimeItem<'doc, 'body>> {
    let sequence = match select.select(document) {
        Ok(sequence) => sequence,
        Err(error) => {
            diagnostics.push(runtime_error(
                "compiled_select_context_mismatch",
                error.message,
                format!("{base_path}/select"),
                strategy_key,
                json!({ "selectType": select.kind().key() }),
            ));
            return None;
        }
    };
    let mut items = sequence.into_vec();
    if allow_collection && select.kind() == SelectKind::XmlElement && !items.is_empty() {
        let nodes = items
            .into_iter()
            .filter_map(|item| match item {
                SelectedItem::Xml(node) => Some(node),
                _ => None,
            })
            .collect();
        return Some(RuntimeItem::XmlCollection(nodes));
    }
    match items.len() {
        0 => {
            let (code, message) = match select.kind() {
                SelectKind::JsonPath => (
                    "json_path_select_missing",
                    "JSONPath selector did not match a posting detail document",
                ),
                SelectKind::XmlElement => (
                    "xml_select_missing",
                    "XML element selector did not match a posting detail document",
                ),
                SelectKind::XmlText => (
                    "xml_text_select_missing",
                    "XML text selector did not match posting detail text",
                ),
                SelectKind::Css => (
                    "css_select_missing",
                    "CSS selector did not match a posting detail document",
                ),
                SelectKind::Document => (
                    "document_select_missing",
                    "Document selector produced no posting detail document",
                ),
                SelectKind::SitemapUrls => (
                    "sitemap_select_invalid",
                    "Sitemap selection is unavailable in Detail",
                ),
            };
            diagnostics.push(runtime_error(
                code,
                message,
                format!("{base_path}/select"),
                strategy_key,
                json!({ "selectType": select.kind().key() }),
            ));
            None
        }
        1 => match items.remove(0) {
            SelectedItem::Json(value) => Some(RuntimeItem::Json(value)),
            SelectedItem::Xml(value) => Some(RuntimeItem::Xml(value)),
            SelectedItem::Html(value) => Some(RuntimeItem::Html(value)),
            SelectedItem::Text(value) => Some(RuntimeItem::Text(value)),
        },
        count => {
            let (code, message) = match select.kind() {
                SelectKind::XmlElement => (
                    "xml_select_multiple",
                    "XML element selector matched multiple posting detail documents",
                ),
                SelectKind::XmlText => (
                    "xml_text_select_multiple",
                    "XML text selector matched multiple posting detail text values",
                ),
                SelectKind::Css => (
                    "css_select_multiple",
                    "CSS selector matched multiple posting detail documents",
                ),
                _ => (
                    "select_multiple",
                    "Selector matched multiple posting detail documents",
                ),
            };
            diagnostics.push(runtime_error(
                code,
                message,
                format!("{base_path}/select"),
                strategy_key,
                json!({ "selectType": select.kind().key(), "actualCount": count }),
            ));
            None
        }
    }
}
