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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceIdentity {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) key_candidates: Vec<String>,
    pub(super) name_candidates: Vec<String>,
}

pub(super) fn derive_source_identity(
    identity: Option<&SourceProfileIdentity>,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<SourceIdentity, String> {
    let fallback_company_name = derive_company_name(input_url);
    let fallback_key = format!("{}", to_technical_key(&fallback_company_name));
    let fallback_name = format!("{fallback_company_name}");

    let mut key_candidates = render_identity_candidates(
        identity
            .map(|identity| identity.key_candidates.as_slice())
            .unwrap_or(&[]),
        input_url,
        captures,
    )?
    .into_iter()
    .map(|candidate| to_technical_key(&candidate))
    .filter(|candidate| !candidate.is_empty())
    .collect::<Vec<_>>();
    dedupe_preserving_order(&mut key_candidates);
    if key_candidates.is_empty() {
        key_candidates.push(fallback_key);
    }

    let mut name_candidates = render_identity_candidates(
        identity
            .map(|identity| identity.name_candidates.as_slice())
            .unwrap_or(&[]),
        input_url,
        captures,
    )?
    .into_iter()
    .filter(|candidate| !candidate.trim().is_empty())
    .collect::<Vec<_>>();
    dedupe_preserving_order(&mut name_candidates);
    if name_candidates.is_empty() {
        name_candidates.push(fallback_name);
    }

    Ok(SourceIdentity {
        key: key_candidates[0].clone(),
        name: name_candidates[0].clone(),
        key_candidates,
        name_candidates,
    })
}

pub(super) fn render_identity_candidates(
    candidates: &[String],
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let mut rendered_candidates = Vec::new();
    for candidate in candidates {
        match render_detection_template(candidate, input_url, captures) {
            Ok(rendered) => {
                let rendered = rendered.trim();
                if !rendered.is_empty() {
                    rendered_candidates.push(rendered.to_string());
                }
            }
            Err(error) if is_missing_capture(&error) => continue,
            Err(error) => return Err(detection_template_error_message(error)),
        }
    }

    Ok(rendered_candidates)
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

pub(super) fn derive_company_name(url: &Url) -> String {
    let host = normalized_host(url);
    let label = host
        .split('.')
        .find(|label| !is_generic_host_label(label))
        .unwrap_or("neue_quelle");
    title_case(label)
}
