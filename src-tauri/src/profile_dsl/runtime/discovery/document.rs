use super::*;
pub(super) use crate::profile_dsl::primitives::parse::ParsedDocument;
pub(super) use crate::profile_dsl::primitives::select::SelectedItem as RuntimeItem;
use crate::profile_dsl::primitives::select::{CompiledSelect, SelectKind};

pub(super) fn select_items<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &CompiledSelect,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<RuntimeItem<'doc, 'body>>> {
    let items = select_items_raw(document, select, base_path, strategy_key, diagnostics)?;
    if let [RuntimeItem::Json(Value::Array(values))] = items.as_slice() {
        return Some(values.iter().map(RuntimeItem::Json).collect());
    }
    Some(items)
}

pub(super) fn select_items_raw<'doc, 'body>(
    document: &'doc ParsedDocument<'body>,
    select: &CompiledSelect,
    base_path: &str,
    strategy_key: Option<&str>,
    diagnostics: &mut Diagnostics,
) -> Option<Vec<RuntimeItem<'doc, 'body>>> {
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
    if sequence.is_empty() {
        let (code, message) = match select.kind() {
            SelectKind::JsonPath => (
                "json_path_select_missing",
                "JSONPath selector did not match a posting item collection",
            ),
            SelectKind::XmlElement => (
                "xml_select_missing",
                "XML element selector did not match any posting items",
            ),
            SelectKind::XmlText => (
                "xml_text_select_missing",
                "XML text selector did not match any text values",
            ),
            SelectKind::Css => (
                "css_select_missing",
                "CSS selector did not match any posting items",
            ),
            SelectKind::SitemapUrls => return Some(Vec::new()),
            SelectKind::Document => (
                "document_select_missing",
                "Document selector produced no item",
            ),
        };
        diagnostics.push(runtime_error(
            code,
            message,
            format!("{base_path}/select"),
            strategy_key,
            json!({ "selectType": select.kind().key() }),
        ));
        return None;
    }
    Some(sequence.into_vec())
}
