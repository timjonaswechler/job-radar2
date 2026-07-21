use serde::{Deserialize, Serialize};

use super::{SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor =
    super::SelectDescriptor { key: "xml_element" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct XmlElementSelect {
    pub(super) element: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct XmlElementSelectPlan {
    element: String,
}

pub(crate) fn compile(element: &str) -> Result<XmlElementSelectPlan, String> {
    if element.is_empty() {
        return Err("XML element local name must not be empty".to_string());
    }
    Ok(XmlElementSelectPlan {
        element: element.to_string(),
    })
}

pub(crate) fn execute<'doc, 'body>(
    plan: &XmlElementSelectPlan,
    root: roxmltree::Node<'doc, 'body>,
) -> SelectedSequence<'doc, 'body> {
    SelectedSequence::new(
        descendant_elements(root, &plan.element)
            .into_iter()
            .map(SelectedItem::Xml)
            .collect(),
    )
}

pub(crate) fn descendant_elements<'doc, 'body>(
    node: roxmltree::Node<'doc, 'body>,
    element: &str,
) -> Vec<roxmltree::Node<'doc, 'body>> {
    node.descendants()
        .filter(|candidate| candidate.is_element() && candidate.tag_name().name() == element)
        .collect()
}
