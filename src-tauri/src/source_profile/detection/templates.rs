use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::profile_dsl::diagnostics::Diagnostic;
use crate::profile_dsl::template::{render_template, TemplateContext, TemplateError};

use super::detection_error;

struct DetectionTemplateContext<'a> {
    input_url: &'a str,
    captures: &'a BTreeMap<String, String>,
    source_config: Option<&'a Map<String, Value>>,
}

impl TemplateContext for DetectionTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "inputUrl" {
            return Ok(Some(self.input_url.to_string()));
        }
        if let Some(capture_key) = variable.strip_prefix("capture:") {
            return Ok(self.captures.get(capture_key).cloned());
        }
        if let Some(source_config_key) = variable
            .strip_prefix("sourceConfig:")
            .or_else(|| variable.strip_prefix("sourceConfig."))
        {
            return Ok(self
                .source_config
                .and_then(|source_config| source_config.get(source_config_key))
                .and_then(json_scalar_as_string));
        }
        Ok(None)
    }
}

pub(super) fn render_detection_template(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    render_detection_template_with_source_config(template, input_url, captures, None)
}

pub(super) fn render_detection_template_with_source_config(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    source_config: Option<&Map<String, Value>>,
) -> Result<String, TemplateError> {
    render_template(
        template,
        &DetectionTemplateContext {
            input_url,
            captures,
            source_config,
        },
    )
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(super) fn template_diagnostic(
    error: TemplateError,
    path: &str,
    probe_key: Option<&str>,
) -> Diagnostic {
    let code = match error {
        TemplateError::MissingVariable(_) => "missing_detection_template_variable",
        TemplateError::Invalid(_) => "invalid_detection_template",
    };
    detection_error(
        code,
        format!("Profile Detection template could not be rendered: {error}"),
        path,
        probe_key,
        serde_json::json!({ "error": error.to_string() }),
    )
}
