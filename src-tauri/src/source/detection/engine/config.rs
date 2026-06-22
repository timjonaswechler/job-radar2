use std::{collections::HashMap, future::Future, path::Path, pin::Pin, time::Duration};

use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    declarative::template::{
        render_template, title_case, to_technical_key, TemplateContext, TemplateError,
    },
    simple_json_path::simple_json_path_exists,
    source::registry::{
        self, AvailabilityBlock, DetectionBlock, DetectionPhase, ProfileAccessPathDefinition,
        RegistrySourceProfile, SourceProfileIdentity, SourceRegistryDiagnostic,
        SourceRegistryDocumentKind,
    },
};

use super::*;

pub(super) fn source_config_satisfies_required_schema(
    source_config: &Value,
    schema: Option<&Value>,
) -> bool {
    let Some(required_fields) = schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
    else {
        return true;
    };

    required_fields
        .iter()
        .filter_map(Value::as_str)
        .all(|field| source_config_value_is_available(source_config.get(field)))
}

pub(super) fn source_config_value_is_available(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Null) | None => false,
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(_) => true,
    }
}

pub(super) fn build_source_config(
    source_config_template: Option<&Value>,
    identity: Option<&SourceProfileIdentity>,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, TemplateError> {
    let mut source_config = if let Some(object) = source_config_template.and_then(Value::as_object)
    {
        let mut rendered = Map::new();
        for (key, value) in object {
            rendered.insert(
                key.clone(),
                render_template_value(value, input_url, captures)?,
            );
        }
        Value::Object(rendered)
    } else {
        json_default_source_config(input_url)
    };

    merge_optional_source_config(&mut source_config, identity, input_url, captures)?;
    Ok(source_config)
}

pub(super) fn merge_optional_source_config(
    source_config: &mut Value,
    identity: Option<&SourceProfileIdentity>,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<(), TemplateError> {
    let Some(optional_config) =
        identity.and_then(|identity| identity.optional_source_config.as_ref())
    else {
        return Ok(());
    };
    let Some(optional_config) = optional_config.as_object() else {
        return Ok(());
    };
    let Some(source_config) = source_config.as_object_mut() else {
        return Ok(());
    };

    for (key, value) in optional_config {
        if let Some(rendered) = render_optional_template_value(value, input_url, captures)? {
            source_config.insert(key.clone(), rendered);
        }
    }

    Ok(())
}

pub(super) fn render_template_value(
    value: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, TemplateError> {
    match value {
        Value::String(template) => Ok(Value::String(render_detection_template(
            template, input_url, captures,
        )?)),
        Value::Array(values) => values
            .iter()
            .map(|value| render_template_value(value, input_url, captures))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    render_template_value(value, input_url, captures)?,
                ))
            })
            .collect::<Result<Map<_, _>, TemplateError>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

pub(super) fn render_optional_template_value(
    value: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Option<Value>, TemplateError> {
    match value {
        Value::String(template) => match render_detection_template(template, input_url, captures) {
            Ok(rendered) => Ok(Some(Value::String(rendered))),
            Err(error) if is_missing_capture(&error) => Ok(None),
            Err(error) => Err(error),
        },
        Value::Array(values) => {
            let mut rendered_values = Vec::new();
            for value in values {
                let Some(rendered_value) =
                    render_optional_template_value(value, input_url, captures)?
                else {
                    return Ok(None);
                };
                rendered_values.push(rendered_value);
            }
            Ok(Some(Value::Array(rendered_values)))
        }
        Value::Object(object) => {
            let mut rendered_object = Map::new();
            for (key, value) in object {
                if let Some(rendered_value) =
                    render_optional_template_value(value, input_url, captures)?
                {
                    rendered_object.insert(key.clone(), rendered_value);
                }
            }
            if rendered_object.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Value::Object(rendered_object)))
            }
        }
        other => Ok(Some(other.clone())),
    }
}

pub(super) fn json_default_source_config(input_url: &Url) -> Value {
    serde_json::json!({ "startUrl": input_url.as_str() })
}
