use roxmltree::{Document, Node};
use serde_json::Value;

use crate::search_run_model::SourceExecutionError;

use super::*;

pub(super) fn select_xml_items(
    xml: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    let select = required_object_value(items, "select", "executionPlan.inventory.items.select")?;
    if let Some(element_name) = select.get("xmlText") {
        let element_name = element_name.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(
                "executionPlan.inventory.items.select.xmlText must be a string".to_string(),
            )
        })?;
        if element_name.trim().is_empty() {
            return Err(SourceExecutionError::Failed(
                "executionPlan.inventory.items.select.xmlText must not be empty".to_string(),
            ));
        }

        return parse_xml_text_values(xml, element_name)
            .map(|values| values.into_iter().map(InventoryItem::Text).collect())
            .map_err(|error| {
                SourceExecutionError::Failed(format!("could not parse inventory XML: {error}"))
            });
    }

    if let Some(element_name) = select.get("xmlElement") {
        let element_name = element_name.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(
                "executionPlan.inventory.items.select.xmlElement must be a string".to_string(),
            )
        })?;
        if element_name.trim().is_empty() {
            return Err(SourceExecutionError::Failed(
                "executionPlan.inventory.items.select.xmlElement must not be empty".to_string(),
            ));
        }

        return parse_xml_element_values(xml, element_name)
            .map(|values| values.into_iter().map(InventoryItem::Json).collect())
            .map_err(|error| {
                SourceExecutionError::Failed(format!("could not parse inventory XML: {error}"))
            });
    }

    Err(SourceExecutionError::Failed(
        "executionPlan.inventory.items.select must contain xmlText or xmlElement".to_string(),
    ))
}

pub(super) fn parse_xml_text_values(xml: &str, element_name: &str) -> Result<Vec<String>, String> {
    let document = Document::parse(xml).map_err(|error| error.to_string())?;

    Ok(document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == element_name)
        .map(xml_text_content)
        .collect())
}

pub(super) fn parse_xml_element_values(
    xml: &str,
    element_name: &str,
) -> Result<Vec<Value>, String> {
    let document = Document::parse(xml).map_err(|error| error.to_string())?;

    Ok(document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == element_name)
        .map(xml_element_to_json_value)
        .collect())
}

pub(super) fn xml_element_to_json_value(node: Node<'_, '_>) -> Value {
    let element_children = node
        .children()
        .filter(|child| child.is_element())
        .collect::<Vec<_>>();

    if element_children.is_empty() {
        return Value::String(xml_text_content(node));
    }

    let mut object = serde_json::Map::new();
    for child in element_children {
        insert_xml_json_child(
            &mut object,
            child.tag_name().name().to_string(),
            xml_element_to_json_value(child),
        );
    }

    Value::Object(object)
}

pub(super) fn xml_text_content(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|text| text.text())
        .collect::<String>()
        .trim()
        .to_string()
}

pub(super) fn insert_xml_json_child(
    object: &mut serde_json::Map<String, Value>,
    name: String,
    value: Value,
) {
    match object.get_mut(&name) {
        None => {
            object.insert(name, value);
        }
        Some(Value::Array(values)) => values.push(value),
        Some(existing) => {
            let previous = std::mem::take(existing);
            *existing = Value::Array(vec![previous, value]);
        }
    }
}
