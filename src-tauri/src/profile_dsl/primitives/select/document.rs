use serde::{Deserialize, Serialize};

use super::{ParsedDocument, SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor = super::SelectDescriptor { key: "document" };

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSelect {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct DocumentSelectPlan;

pub(super) fn execute<'doc, 'body>(
    _plan: &DocumentSelectPlan,
    document: &'doc ParsedDocument<'body>,
) -> SelectedSequence<'doc, 'body> {
    let item = match document {
        ParsedDocument::Json(value) => SelectedItem::Json(value),
        ParsedDocument::Xml(value) => SelectedItem::Xml(value.root_element()),
        ParsedDocument::Html(value) => SelectedItem::Html(value.tree.root()),
    };
    SelectedSequence::one(item)
}
