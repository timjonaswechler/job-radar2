use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::profile_dsl::diagnostics::Diagnostic;
use crate::profile_dsl::documents::ReusableAccessPathDocument;
use crate::profile_dsl::template::{title_case, to_technical_key, TemplateError};
use crate::source_profile::documents::{ProfileDetectionDocument, SourceProfileDocument};

use super::{
    detection_error, render_detection_template, template_diagnostic, SourceProposal,
    SourceProposalEvidence,
};

pub(super) fn build_source_proposal(
    input_url: &str,
    profile: &SourceProfileDocument,
    detect: &ProfileDetectionDocument,
    captures: BTreeMap<String, String>,
    evidence: Vec<SourceProposalEvidence>,
    base_path: &str,
) -> Result<SourceProposal, Diagnostic> {
    let access_path = recommended_access_path(profile, detect).ok_or_else(|| {
        detection_error(
            "recommended_access_path_not_found",
            format!(
                "Source Profile `{}` does not define the recommended Access Path",
                profile.key
            ),
            format!("{base_path}/recommendedAccessPathKey"),
            None,
            serde_json::json!({
                "sourceProfileKey": profile.key,
                "recommendedAccessPathKey": detect.recommended_access_path_key,
            }),
        )
    })?;

    let source_config = build_source_config(input_url, profile, detect, &captures)
        .map_err(|error| template_diagnostic(error, &format!("{base_path}/sourceConfig"), None))?;
    validate_source_config_for_detection(&source_config, profile, access_path, base_path)?;
    let key_candidates = render_candidate_templates(
        detect.key_candidates.as_deref(),
        || default_key_candidates(&captures, &profile.key),
        input_url,
        &captures,
        &format!("{base_path}/keyCandidates"),
    )?;
    let name_candidates = render_candidate_templates(
        detect.name_candidates.as_deref(),
        || default_name_candidates(&captures, &profile.name),
        input_url,
        &captures,
        &format!("{base_path}/nameCandidates"),
    )?;

    Ok(SourceProposal {
        profile_key: profile.key.clone(),
        profile_name: profile.name.clone(),
        recommended_access_path_key: access_path.key.clone(),
        recommended_access_path_name: access_path.name.clone(),
        source_config,
        key_candidates,
        name_candidates,
        captures,
        evidence,
        support_level: profile.support.level,
    })
}

fn recommended_access_path<'a>(
    profile: &'a SourceProfileDocument,
    detect: &ProfileDetectionDocument,
) -> Option<&'a ReusableAccessPathDocument> {
    if let Some(key) = &detect.recommended_access_path_key {
        profile.access_paths.iter().find(|path| path.key == *key)
    } else if profile.access_paths.len() == 1 {
        profile.access_paths.first()
    } else {
        None
    }
}

fn validate_source_config_for_detection(
    source_config: &Value,
    profile: &SourceProfileDocument,
    access_path: &ReusableAccessPathDocument,
    base_path: &str,
) -> Result<(), Diagnostic> {
    let Some(source_config) = source_config.as_object() else {
        return Err(detection_error(
            "invalid_source_config_proposal",
            "Profile Detection produced a Source Config proposal that is not an object",
            format!("{base_path}/sourceConfig"),
            None,
            serde_json::json!({ "expectedType": "object" }),
        ));
    };

    for key in source_config.keys() {
        if is_search_request_criteria_key(key) {
            return Err(detection_error(
                "forbidden_search_criteria_in_source_config",
                format!(
                    "Source Config proposal property `{key}` looks like Search Request criteria"
                ),
                format!("{base_path}/sourceConfig/{key}"),
                None,
                serde_json::json!({ "property": key }),
            ));
        }
    }

    for key in required_schema_keys(profile.source_config_schema.as_ref())
        .into_iter()
        .chain(required_schema_keys(
            access_path.source_config_schema.as_ref(),
        ))
    {
        if !source_config.contains_key(&key) {
            return Err(detection_error(
                "missing_source_config_required_property",
                format!("Source Config proposal is missing required property `{key}`"),
                format!("{base_path}/sourceConfig/{key}"),
                None,
                serde_json::json!({ "property": key }),
            ));
        }
    }

    Ok(())
}

fn required_schema_keys(schema: Option<&Map<String, Value>>) -> Vec<String> {
    schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .map(|required| {
            required
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn is_search_request_criteria_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "keyword"
            | "keywords"
            | "role"
            | "roles"
            | "preferredlocation"
            | "preferredlocations"
            | "country"
            | "countries"
            | "radius"
            | "includerule"
            | "includerules"
            | "excluderule"
            | "excluderules"
            | "matchrule"
            | "matchrules"
            | "exclusionrule"
            | "exclusionrules"
    )
}

pub(super) fn build_source_config(
    input_url: &str,
    profile: &SourceProfileDocument,
    detect: &ProfileDetectionDocument,
    captures: &BTreeMap<String, String>,
) -> Result<Value, TemplateError> {
    let Some(template) = &detect.source_config else {
        return Ok(Value::Object(default_source_config(
            profile, input_url, captures,
        )));
    };
    render_json_object_templates(template, input_url, captures).map(Value::Object)
}

fn default_source_config(
    profile: &SourceProfileDocument,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Map<String, Value> {
    let mut source_config = Map::new();
    if let Some(schema) = &profile.source_config_schema {
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for key in properties.keys() {
                if let Some(value) = captures.get(key) {
                    source_config.insert(key.clone(), Value::String(value.clone()));
                } else if key == "startUrl" {
                    source_config.insert(key.clone(), Value::String(input_url.to_string()));
                }
            }
        }
    }
    source_config
}

fn render_json_object_templates(
    template: &Map<String, Value>,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<Map<String, Value>, TemplateError> {
    let mut rendered = Map::new();
    for (key, value) in template {
        rendered.insert(
            key.clone(),
            render_json_value_templates(value, input_url, captures)?,
        );
    }
    Ok(rendered)
}

fn render_json_value_templates(
    value: &Value,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<Value, TemplateError> {
    match value {
        Value::String(value) => {
            render_detection_template(value, input_url, captures).map(Value::String)
        }
        Value::Array(values) => values
            .iter()
            .map(|value| render_json_value_templates(value, input_url, captures))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(values) => {
            render_json_object_templates(values, input_url, captures).map(Value::Object)
        }
        other => Ok(other.clone()),
    }
}

fn render_candidate_templates<F>(
    templates: Option<&[String]>,
    default: F,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    path: &str,
) -> Result<Vec<String>, Diagnostic>
where
    F: FnOnce() -> Vec<String>,
{
    let Some(templates) = templates else {
        return Ok(default());
    };
    let mut candidates = Vec::new();
    for (index, template) in templates.iter().enumerate() {
        let rendered = render_detection_template(template, input_url, captures)
            .map_err(|error| template_diagnostic(error, &format!("{path}/{index}"), None))?;
        let trimmed = rendered.trim();
        if !trimmed.is_empty() && !candidates.iter().any(|candidate| candidate == trimmed) {
            candidates.push(trimmed.to_string());
        }
    }
    Ok(candidates)
}

fn default_key_candidates(captures: &BTreeMap<String, String>, profile_key: &str) -> Vec<String> {
    captures
        .values()
        .next()
        .map(|value| vec![to_technical_key(value)])
        .unwrap_or_else(|| vec![profile_key.to_string()])
}

fn default_name_candidates(captures: &BTreeMap<String, String>, profile_name: &str) -> Vec<String> {
    captures
        .values()
        .next()
        .map(|value| vec![title_case(value)])
        .unwrap_or_else(|| vec![profile_name.to_string()])
}

pub(super) fn detection_document_evidence(
    detect: &ProfileDetectionDocument,
) -> Vec<SourceProposalEvidence> {
    detect
        .evidence
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|evidence| SourceProposalEvidence {
            kind: evidence.kind,
            message: evidence.message.clone(),
            path: evidence.path.clone(),
            probe_key: None,
        })
        .collect()
}
