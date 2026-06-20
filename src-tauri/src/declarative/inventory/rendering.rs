use regex::Regex;
use reqwest::Url;
use serde_json::Value;
use std::collections::HashMap;

use crate::{
    declarative::template::{render_template, TemplateContext, TemplateError},
    search_run_model::{SourceExecutionError, SourceExecutionSource},
    simple_json_path::resolve_simple_json_path,
};

use super::*;

pub(super) fn compile_regex_list(
    value: Option<&Value>,
    path: &str,
) -> Result<Vec<Regex>, SourceExecutionError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be an array")))?;

    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry_path = format!("{path}[{index}]");
            let object = entry.as_object().ok_or_else(|| {
                SourceExecutionError::Failed(format!("{entry_path} must be a JSON object"))
            })?;
            let pattern = required_string(object, "regex", &format!("{entry_path}.regex"))?;
            Regex::new(pattern).map_err(|error| {
                SourceExecutionError::Failed(format!("{entry_path}.regex is invalid: {error}"))
            })
        })
        .collect()
}

pub(super) fn capture_item(regexes: &[Regex], item_text: &str) -> Option<HashMap<String, String>> {
    let mut values = HashMap::new();
    for regex in regexes {
        let captures = regex.captures(item_text)?;
        for capture_name in regex.capture_names().flatten() {
            if let Some(value) = captures.name(capture_name) {
                values.insert(capture_name.to_string(), value.as_str().to_string());
            }
        }
    }
    Some(values)
}

pub(super) fn render_required_field(
    fields: &serde_json::Map<String, Value>,
    field_name: &str,
    context: &InventoryTemplateContext<'_>,
) -> Result<String, SourceExecutionError> {
    let field = fields.get(field_name).ok_or_else(|| {
        SourceExecutionError::Failed(format!(
            "executionPlan.inventory.fields.{field_name} is required"
        ))
    })?;
    render_field_expression(
        field,
        context,
        &format!("executionPlan.inventory.fields.{field_name}"),
    )
}

pub(super) fn render_locations(
    fields: &serde_json::Map<String, Value>,
    context: &InventoryTemplateContext<'_>,
) -> Result<Vec<String>, SourceExecutionError> {
    let locations = fields.get("locations").ok_or_else(|| {
        SourceExecutionError::Failed(
            "executionPlan.inventory.fields.locations is required".to_string(),
        )
    })?;
    let locations = locations.as_array().ok_or_else(|| {
        SourceExecutionError::Failed(
            "executionPlan.inventory.fields.locations must be an array".to_string(),
        )
    })?;

    let mut rendered_locations = Vec::new();
    for (index, location) in locations.iter().enumerate() {
        rendered_locations.extend(render_location_expression(
            location,
            context,
            &format!("executionPlan.inventory.fields.locations[{index}]"),
        )?);
    }
    dedupe_preserving_order(&mut rendered_locations);
    Ok(rendered_locations)
}

pub(super) fn render_location_expression(
    value: &Value,
    context: &InventoryTemplateContext<'_>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    let split = optional_location_split(object, path)?;

    if let Some(template) = object.get("template").and_then(Value::as_str) {
        let rendered = render_template(template, context).map_err(|error| {
            SourceExecutionError::Failed(format!("{path}.template is invalid: {error}"))
        })?;
        return Ok(location_string_values(&rendered, split));
    }

    if let Some(json_path) = object.get("jsonPath") {
        let json_path = json_path.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(format!("{path}.jsonPath must be a non-empty string"))
        })?;
        if json_path.trim().is_empty() {
            return Err(SourceExecutionError::Failed(format!(
                "{path}.jsonPath must be a non-empty string"
            )));
        }
        let item = context.item.and_then(InventoryItem::json).ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path}.jsonPath is only available for JSON inventory items"
            ))
        })?;
        let value = resolve_simple_json_path(item, json_path).map_err(|error| {
            simple_json_path_execution_error(&format!("{path}.jsonPath"), error)
        })?;
        if object.contains_key("objectFields") {
            let object_fields = required_location_object_fields(object, path)?;
            return json_location_object_fields_to_strings(value, &object_fields, path);
        }
        return json_location_value_to_strings(value, split, path);
    }

    Err(SourceExecutionError::Failed(format!(
        "{path} must contain a template or jsonPath expression"
    )))
}

pub(super) fn optional_location_split<'a>(
    object: &'a serde_json::Map<String, Value>,
    path: &str,
) -> Result<Option<&'a str>, SourceExecutionError> {
    let Some(split) = object.get("split") else {
        return Ok(None);
    };
    let delimiter = split
        .as_str()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path}.split must be a string")))?;
    if delimiter.is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.split must not be empty"
        )));
    }
    Ok(Some(delimiter))
}

pub(super) fn required_location_object_fields(
    object: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    let fields = object
        .get("objectFields")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            SourceExecutionError::Failed(format!("{path}.objectFields must be an array"))
        })?;
    if fields.is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "{path}.objectFields must not be empty"
        )));
    }

    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let field = field.as_str().ok_or_else(|| {
                SourceExecutionError::Failed(format!(
                    "{path}.objectFields[{index}] must be a non-empty string"
                ))
            })?;
            if field.trim().is_empty() {
                return Err(SourceExecutionError::Failed(format!(
                    "{path}.objectFields[{index}] must be a non-empty string"
                )));
            }
            Ok(field.to_string())
        })
        .collect()
}

pub(super) fn json_location_object_fields_to_strings(
    value: Option<&Value>,
    object_fields: &[String],
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(object)) => location_object_fields_to_string(object, object_fields, path)
            .map(|location| location.into_iter().collect()),
        Some(Value::Array(values)) => {
            let mut locations = Vec::new();
            for (index, value) in values.iter().enumerate() {
                let object = value.as_object().ok_or_else(|| {
                    SourceExecutionError::Failed(format!(
                        "{path}.jsonPath array item {index} must resolve to an object when objectFields is set"
                    ))
                })?;
                if let Some(location) = location_object_fields_to_string(
                    object,
                    object_fields,
                    &format!("{path}.jsonPath array item {index}"),
                )? {
                    locations.push(location);
                }
            }
            Ok(locations)
        }
        Some(_) => Err(SourceExecutionError::Failed(format!(
            "{path}.jsonPath must resolve to an object, null, or an array of objects when objectFields is set"
        ))),
    }
}

pub(super) fn location_object_fields_to_string(
    object: &serde_json::Map<String, Value>,
    object_fields: &[String],
    path: &str,
) -> Result<Option<String>, SourceExecutionError> {
    let mut parts = Vec::new();
    for field in object_fields {
        match object.get(field) {
            None | Some(Value::Null) => {}
            Some(Value::String(value)) => {
                let value = value.trim();
                if !value.is_empty() {
                    parts.push(value.to_string());
                }
            }
            Some(Value::Bool(value)) => parts.push(value.to_string()),
            Some(Value::Number(value)) => parts.push(value.to_string()),
            Some(Value::Array(_)) | Some(Value::Object(_)) => {
                return Err(SourceExecutionError::Failed(format!(
                    "{path}.{field} must resolve to a string, number, boolean, or null"
                )));
            }
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join(", ")))
    }
}

pub(super) fn json_location_value_to_strings(
    value: Option<&Value>,
    split: Option<&str>,
    path: &str,
) -> Result<Vec<String>, SourceExecutionError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(value)) => Ok(location_string_values(value, split)),
        Some(Value::Bool(value)) => Ok(location_string_values(&value.to_string(), None)),
        Some(Value::Number(value)) => Ok(location_string_values(&value.to_string(), None)),
        Some(Value::Array(values)) => {
            let mut locations = Vec::new();
            for (index, value) in values.iter().enumerate() {
                match value {
                    Value::Null => {}
                    Value::String(value) => locations.extend(location_string_values(value, split)),
                    Value::Bool(value) => locations.extend(location_string_values(&value.to_string(), None)),
                    Value::Number(value) => {
                        locations.extend(location_string_values(&value.to_string(), None))
                    }
                    Value::Array(_) | Value::Object(_) => {
                        return Err(SourceExecutionError::Failed(format!(
                            "{path}.jsonPath array item {index} must resolve to a string, number, boolean, or null"
                        )));
                    }
                }
            }
            Ok(locations)
        }
        Some(Value::Object(_)) => Err(SourceExecutionError::Failed(format!(
            "{path}.jsonPath must resolve to a string, number, boolean, null, or an array of those values"
        ))),
    }
}

pub(super) fn location_string_values(value: &str, split: Option<&str>) -> Vec<String> {
    match split {
        Some(delimiter) => value
            .split(delimiter)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        None => {
            let value = value.trim();
            if value.is_empty() {
                Vec::new()
            } else {
                vec![value.to_string()]
            }
        }
    }
}

pub(super) fn dedupe_preserving_order(values: &mut Vec<String>) {
    let mut seen = Vec::<String>::new();
    values.retain(|value| {
        if seen.iter().any(|seen_value| seen_value == value) {
            false
        } else {
            seen.push(value.clone());
            true
        }
    });
}

pub(super) fn render_field_expression(
    value: &Value,
    context: &InventoryTemplateContext<'_>,
    path: &str,
) -> Result<String, SourceExecutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))?;
    if let Some(template) = object.get("template").and_then(Value::as_str) {
        return render_template(template, context).map_err(|error| {
            SourceExecutionError::Failed(format!("{path}.template is invalid: {error}"))
        });
    }

    if let Some(json_path) = object.get("jsonPath") {
        let json_path = json_path.as_str().ok_or_else(|| {
            SourceExecutionError::Failed(format!("{path}.jsonPath must be a non-empty string"))
        })?;
        if json_path.trim().is_empty() {
            return Err(SourceExecutionError::Failed(format!(
                "{path}.jsonPath must be a non-empty string"
            )));
        }
        let item = context.item.and_then(InventoryItem::json).ok_or_else(|| {
            SourceExecutionError::Failed(format!(
                "{path}.jsonPath is only available for JSON inventory items"
            ))
        })?;
        let value = resolve_simple_json_path(item, json_path).map_err(|error| {
            simple_json_path_execution_error(&format!("{path}.jsonPath"), error)
        })?;
        return json_field_value_to_string(value, path);
    }

    Err(SourceExecutionError::Failed(format!(
        "{path} must contain a template or jsonPath expression"
    )))
}

pub(super) fn json_field_value_to_string(
    value: Option<&Value>,
    path: &str,
) -> Result<String, SourceExecutionError> {
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Array(_) | Value::Object(_)) => Err(SourceExecutionError::Failed(format!(
            "{path}.jsonPath must resolve to a string, number, boolean, or null"
        ))),
    }
}

pub(super) struct InventoryTemplateContext<'a> {
    pub(super) source: &'a SourceExecutionSource,
    pub(super) item: Option<&'a InventoryItem>,
    pub(super) captures: &'a HashMap<String, String>,
}

impl TemplateContext for InventoryTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "sourceName" {
            Ok(Some(self.source.name.clone()))
        } else if variable == "sourceKey" {
            Ok(Some(self.source.key.clone()))
        } else if variable == "itemText" {
            self.item
                .and_then(InventoryItem::text)
                .map(str::to_string)
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(
                        "itemText is not available in this template context".to_string(),
                    )
                })
        } else if let Some(config_key) = variable.strip_prefix("sourceConfig:") {
            if config_key.is_empty() {
                return Err(TemplateError::Invalid(
                    "sourceConfig template variable must include a key".to_string(),
                ));
            }
            source_config_value_as_string(&self.source.source_config, config_key)
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(format!("sourceConfig.{config_key} is not available"))
                })
        } else if let Some(json_path) = variable.strip_prefix("itemJson:") {
            if json_path.trim().is_empty() {
                return Err(TemplateError::Invalid(
                    "itemJson template variable must include a JSONPath".to_string(),
                ));
            }
            let item = self.item.and_then(InventoryItem::json).ok_or_else(|| {
                TemplateError::Invalid(
                    "itemJson is not available in this template context".to_string(),
                )
            })?;
            item_json_value_as_string(item, json_path).map(Some)
        } else if let Some(capture_key) = variable.strip_prefix("capture:") {
            if capture_key.is_empty() {
                return Err(TemplateError::Invalid(
                    "capture template variable must include a capture name".to_string(),
                ));
            }
            self.captures
                .get(capture_key)
                .cloned()
                .map(Some)
                .ok_or_else(|| {
                    TemplateError::Invalid(format!("capture `{capture_key}` is not available"))
                })
        } else {
            Err(TemplateError::Invalid(format!(
                "unsupported template variable `{variable}`"
            )))
        }
    }
}

pub(super) fn source_config_value_as_string(source_config: &Value, key: &str) -> Option<String> {
    let value = source_config.get(key)?;
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn item_json_value_as_string(
    item: &Value,
    json_path: &str,
) -> Result<String, TemplateError> {
    let value = resolve_simple_json_path(item, json_path)
        .map_err(|error| TemplateError::Invalid(format!("itemJson path {error}")))?;
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Bool(value)) => Ok(value.to_string()),
        Some(Value::Number(value)) => Ok(value.to_string()),
        Some(Value::Array(_) | Value::Object(_)) => Err(TemplateError::Invalid(format!(
            "itemJson path `{json_path}` must resolve to a string, number, boolean, or null"
        ))),
    }
}

pub(super) fn required_object_value<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a serde_json::Map<String, Value>, SourceExecutionError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| SourceExecutionError::Failed(format!("{path} must be a JSON object")))
}

pub(super) fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, SourceExecutionError> {
    let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-empty string"))
    })?;
    if value.trim().is_empty() {
        return Err(SourceExecutionError::Failed(format!(
            "{path} must be a non-empty string"
        )));
    }
    Ok(value)
}

pub(super) fn required_u64(
    object: &serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<u64, SourceExecutionError> {
    object.get(key).and_then(Value::as_u64).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-negative integer"))
    })
}

pub(super) fn optional_u64(
    object: &serde_json::Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<u64>, SourceExecutionError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    value.as_u64().map(Some).ok_or_else(|| {
        SourceExecutionError::Failed(format!("{path} must be a non-negative integer"))
    })
}

pub(super) fn resolve_http_candidate_url(raw_url: &str, base_url: &Url) -> Option<String> {
    let raw_url = raw_url.trim();
    if raw_url.is_empty() {
        return None;
    }
    let url = base_url.join(raw_url).ok()?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Some(url.to_string())
    } else {
        None
    }
}

pub(super) fn parse_http_url(value: &str, field: &str) -> Result<Url, SourceExecutionError> {
    let url = Url::parse(value.trim()).map_err(|error| {
        SourceExecutionError::Failed(format!(
            "{field} must be an absolute http or https URL: {error}"
        ))
    })?;

    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err(SourceExecutionError::Failed(format!(
            "{field} must be an absolute http or https URL"
        )))
    }
}
