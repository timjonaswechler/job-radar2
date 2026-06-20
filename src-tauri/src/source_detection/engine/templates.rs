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
    source_registry::{
        self, AvailabilityBlock, DetectionBlock, DetectionPhase, ProfileAccessPathDefinition,
        RegistrySourceProfile, SourceProfileIdentity, SourceRegistryDiagnostic,
        SourceRegistryDocumentKind,
    },
};

use super::*;

pub(in crate::source_detection) struct DetectionTemplateContext<'a> {
    pub(in crate::source_detection) input_url: &'a Url,
    pub(in crate::source_detection) captures: &'a HashMap<String, String>,
}

impl TemplateContext for DetectionTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "inputUrl" {
            Ok(Some(self.input_url.as_str().to_string()))
        } else if variable == "origin" {
            Ok(Some(origin(self.input_url)))
        } else if let Some(capture_key) = variable.strip_prefix("capture:") {
            if capture_key.is_empty() {
                return Err(TemplateError::Invalid(
                    "capture template variable must include a capture name".to_string(),
                ));
            }
            Ok(self.captures.get(capture_key).cloned())
        } else {
            Err(TemplateError::Invalid(format!(
                "unsupported template variable `{variable}`"
            )))
        }
    }
}

pub(super) fn render_detection_template(
    template: &str,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<String, TemplateError> {
    let context = DetectionTemplateContext {
        input_url,
        captures,
    };
    render_template(template, &context)
}

pub(super) fn is_missing_capture(error: &TemplateError) -> bool {
    error
        .missing_variable()
        .and_then(|variable| variable.strip_prefix("capture:"))
        .is_some()
}

pub(super) fn detection_template_error_message(error: TemplateError) -> String {
    match error {
        TemplateError::MissingVariable(variable) => {
            if let Some(capture_key) = variable.strip_prefix("capture:") {
                format!("sourceConfig references missing capture `{capture_key}`")
            } else {
                format!("template variable `{variable}` is not available")
            }
        }
        TemplateError::Invalid(message) => message,
    }
}
