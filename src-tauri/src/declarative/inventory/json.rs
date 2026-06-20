use serde_json::Value;

use crate::{
    search_run_model::SourceExecutionError,
    simple_json_path::{resolve_simple_json_path, SimpleJsonPathError},
};

use super::*;

#[derive(Clone, Debug)]
pub(super) enum InventoryItem {
    Text(String),
    Json(Value),
}

impl InventoryItem {
    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Json(_) => None,
        }
    }

    pub(super) fn json(&self) -> Option<&Value> {
        match self {
            Self::Text(_) => None,
            Self::Json(value) => Some(value),
        }
    }
}

pub(super) fn select_json_items(
    json_text: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    select_json_items_with_root(json_text, items).map(|(items, _root)| items)
}

pub(super) fn select_json_items_with_root(
    json_text: &str,
    items: &serde_json::Map<String, Value>,
) -> Result<(Vec<InventoryItem>, Value), SourceExecutionError> {
    let root = serde_json::from_str::<Value>(json_text).map_err(|error| {
        SourceExecutionError::Failed(format!("could not parse inventory JSON: {error}"))
    })?;
    let inventory_items = select_json_items_from_root(&root, items)?;
    Ok((inventory_items, root))
}

pub(super) fn select_json_items_from_root(
    root: &Value,
    items: &serde_json::Map<String, Value>,
) -> Result<Vec<InventoryItem>, SourceExecutionError> {
    let select = required_object_value(items, "select", "executionPlan.inventory.items.select")?;
    let json_path = required_string(
        select,
        "jsonPath",
        "executionPlan.inventory.items.select.jsonPath",
    )?;
    let selected = resolve_simple_json_path(root, json_path)
        .map_err(|error| simple_json_path_execution_error("executionPlan.inventory.items.select.jsonPath", error))?
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "executionPlan.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but no value was found"
            ))
        })?;
    let array = selected.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "executionPlan.inventory.items.select.jsonPath `{json_path}` must resolve to an array, but resolved to {}",
            json_type_label(selected)
        ))
    })?;

    Ok(array.iter().cloned().map(InventoryItem::Json).collect())
}

pub(super) fn simple_json_path_execution_error(
    path: &str,
    error: SimpleJsonPathError,
) -> SourceExecutionError {
    SourceExecutionError::Failed(format!("{path} {error}"))
}

pub(super) fn json_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
