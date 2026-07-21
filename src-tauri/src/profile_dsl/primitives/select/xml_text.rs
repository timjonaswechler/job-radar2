use serde::{Deserialize, Serialize};

use super::{SelectedItem, SelectedSequence};

pub(super) const DESCRIPTOR: super::SelectDescriptor = super::SelectDescriptor { key: "xml_text" };

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XmlTextSelect {
    pub(super) text_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct XmlTextSelectPlan {
    current: bool,
    segments: Vec<String>,
}

pub(super) fn compile(text_path: &str) -> Result<XmlTextSelectPlan, String> {
    let trimmed = text_path.trim();
    let segments = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    Ok(XmlTextSelectPlan {
        current: trimmed.is_empty() || trimmed == "." || segments.is_empty(),
        segments,
    })
}

pub(super) fn execute<'doc, 'body>(
    plan: &XmlTextSelectPlan,
    root: roxmltree::Node<'doc, 'body>,
) -> SelectedSequence<'doc, 'body> {
    SelectedSequence::new(
        nodes_for_plan(root, plan)
            .into_iter()
            .map(node_text)
            .map(SelectedItem::Text)
            .collect(),
    )
}

pub(crate) fn path_texts(node: roxmltree::Node<'_, '_>, text_path: &str) -> Vec<String> {
    let plan = compile(text_path).expect("XML text paths are literal and always compile");
    nodes_for_plan(node, &plan)
        .into_iter()
        .map(node_text)
        .collect()
}

fn nodes_for_plan<'doc, 'body>(
    node: roxmltree::Node<'doc, 'body>,
    plan: &XmlTextSelectPlan,
) -> Vec<roxmltree::Node<'doc, 'body>> {
    if plan.current {
        return vec![node];
    }
    let mut current = vec![node];
    for (index, segment) in plan.segments.iter().enumerate() {
        let mut next = Vec::new();
        for candidate in current {
            if index == 0 && candidate.is_element() && candidate.tag_name().name() == segment {
                next.push(candidate);
            }
            next.extend(
                candidate
                    .children()
                    .filter(|child| child.is_element() && child.tag_name().name() == segment),
            );
        }
        current = next;
        if current.is_empty() {
            break;
        }
    }
    current
}

pub(crate) fn node_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
