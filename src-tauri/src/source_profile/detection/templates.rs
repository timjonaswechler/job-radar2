use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

use super::detection_error;
use crate::profile_dsl::diagnostics::Diagnostic;
use crate::profile_dsl::template::{
    compile_template, descriptor_for_placement, render_template, TemplateAdmissionKeys,
    TemplateCompileError, TemplatePlacement, TemplateReference, TemplateRenderError,
    TemplateValueView,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DetectionTemplateError {
    Compile(TemplateCompileError),
    Missing(TemplateReference),
}
impl fmt::Display for DetectionTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => error.fmt(f),
            Self::Missing(reference) => write!(
                f,
                "template variable `{}:{}` is not available",
                reference.namespace.as_deref().unwrap_or(""),
                reference.key
            ),
        }
    }
}
impl From<TemplateCompileError> for DetectionTemplateError {
    fn from(value: TemplateCompileError) -> Self {
        Self::Compile(value)
    }
}
impl From<TemplateRenderError> for DetectionTemplateError {
    fn from(value: TemplateRenderError) -> Self {
        Self::Missing(value.reference)
    }
}

struct DetectionValues<'a> {
    input_url: &'a str,
    captures: &'a BTreeMap<String, String>,
    source_config: Option<&'a Map<String, Value>>,
}
impl TemplateValueView for DetectionValues<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            None if reference.key == "inputUrl" => Some(self.input_url.to_string()),
            Some("capture") => self.captures.get(&reference.key).cloned(),
            Some("sourceConfig") => self
                .source_config
                .and_then(|values| values.get(&reference.key))
                .and_then(json_scalar_as_string),
            _ => None,
        }
    }
}

// Productive Detection evaluation remains A02-owned residue. It deliberately
// compiles and renders through the canonical Template owner rather than carrying
// a Detection-local parser.
pub(super) fn render_detection_template(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<String, DetectionTemplateError> {
    render_detection_template_at(
        template,
        input_url,
        captures,
        None,
        TemplatePlacement::DetectionProposal,
    )
}
pub(super) fn render_detection_http_template(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<String, DetectionTemplateError> {
    render_detection_template_at(
        template,
        input_url,
        captures,
        None,
        TemplatePlacement::DetectionHttpUrl,
    )
}
pub(super) fn render_detection_template_with_source_config(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    source_config: Option<&Map<String, Value>>,
) -> Result<String, DetectionTemplateError> {
    render_detection_template_at(
        template,
        input_url,
        captures,
        source_config,
        TemplatePlacement::DetectionBrowserUrl,
    )
}
fn render_detection_template_at(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    source_config: Option<&Map<String, Value>>,
    placement: TemplatePlacement,
) -> Result<String, DetectionTemplateError> {
    let descriptor = descriptor_for_placement(
        placement,
        &TemplateAdmissionKeys {
            source_config: source_config
                .into_iter()
                .flat_map(|values| values.keys().cloned())
                .collect(),
            captures: captures.keys().cloned().collect(),
            posting_meta: Default::default(),
        },
    );
    let compiled = compile_template(template, &descriptor)?;
    Ok(render_template(
        &compiled,
        &DetectionValues {
            input_url,
            captures,
            source_config,
        },
    )?)
}
fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn template_diagnostic(
    error: DetectionTemplateError,
    path: &str,
    probe_key: Option<&str>,
) -> Diagnostic {
    let code = match error {
        DetectionTemplateError::Missing(_) => "missing_detection_template_variable",
        DetectionTemplateError::Compile(_) => "invalid_detection_template",
    };
    detection_error(
        code,
        format!("Profile Detection template could not be rendered: {error}"),
        path,
        probe_key,
        serde_json::json!({ "kind": code }),
    )
}
