use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::profile_dsl::documents::{DetectionDocument, ReusableAccessPathDocument};
use crate::profile_dsl::source_config::{compile_contract, ContractViolation, SchemaLocation};
use crate::profile_dsl::template::{title_case, to_technical_key, TemplateError};
use crate::source_profile::documents::SourceProfileDocument;

use super::{
    detection_error, render_detection_template, template_diagnostic, SourceProposal,
    SourceProposalEvidence,
};

pub(super) fn build_source_proposal(
    input_url: &str,
    profile: &SourceProfileDocument,
    detection: &DetectionDocument,
    captures: BTreeMap<String, String>,
    evidence: Vec<SourceProposalEvidence>,
    base_path: &str,
) -> Result<SourceProposal, Vec<Diagnostic>> {
    let access_path = recommended_access_path(profile, detection).ok_or_else(|| {
        vec![detection_error(
            "recommended_access_path_not_found",
            format!(
                "Source Profile `{}` does not define the recommended Access Path",
                profile.key
            ),
            format!("{base_path}/recommendedAccessPathKey"),
            None,
            serde_json::json!({
                "sourceProfileKey": profile.key,
                "recommendedAccessPathKey": detection.recommended_access_path_key,
            }),
        )]
    })?;

    let source_config =
        build_source_config(input_url, profile, Some(access_path), detection, &captures).map_err(
            |error| {
                vec![template_diagnostic(
                    error,
                    &format!("{base_path}/sourceConfig"),
                    None,
                )]
            },
        )?;
    validate_detection_source_config_values(
        &source_config,
        profile,
        access_path,
        base_path,
        ValidationCompleteness::Complete,
    )?;
    let key_candidates = render_key_candidate_templates(
        detection.key_candidates.as_deref(),
        || default_key_candidates(&captures, &profile.key),
        input_url,
        &captures,
        &format!("{base_path}/keyCandidates"),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let name_candidates = render_candidate_templates(
        detection.name_candidates.as_deref(),
        || default_name_candidates(&captures, &profile.name),
        input_url,
        &captures,
        &format!("{base_path}/nameCandidates"),
    )
    .map_err(|diagnostic| vec![diagnostic])?;

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

pub(super) fn recommended_access_path<'a>(
    profile: &'a SourceProfileDocument,
    detection: &DetectionDocument,
) -> Option<&'a ReusableAccessPathDocument> {
    if let Some(key) = &detection.recommended_access_path_key {
        profile.access_paths.iter().find(|path| path.key == *key)
    } else if profile.access_paths.len() == 1 {
        profile.access_paths.first()
    } else {
        None
    }
}

#[derive(Clone, Copy)]
pub(super) enum ValidationCompleteness {
    Incremental,
    Complete,
}

pub(super) fn validate_detection_source_config_values(
    source_config: &Value,
    profile: &SourceProfileDocument,
    access_path: &ReusableAccessPathDocument,
    base_path: &str,
    completeness: ValidationCompleteness,
) -> Result<(), Vec<Diagnostic>> {
    let Some(source_config) = source_config.as_object() else {
        return Err(vec![detection_error(
            "invalid_source_config_proposal",
            "Profile Detection produced a Source Config proposal that is not an object",
            format!("{base_path}/sourceConfig"),
            None,
            serde_json::json!({ "expectedType": "object" }),
        )]);
    };
    let profile_base_path = base_path.strip_suffix("/detect").unwrap_or(base_path);
    let profile_schema_path = format!("{profile_base_path}/sourceConfigSchema");
    let access_path_schema_path = format!("{base_path}/recommendedAccessPath/sourceConfigSchema");
    let contract = compile_contract(&[
        SchemaLocation {
            schema: profile.source_config_schema.as_ref(),
            path: &profile_schema_path,
            title_allowed: true,
        },
        SchemaLocation {
            schema: access_path.source_config_schema.as_ref(),
            path: &access_path_schema_path,
            title_allowed: true,
        },
    ])
    .map_err(|violations| {
        violations
            .into_iter()
            .map(compiler_definition_diagnostic)
            .collect::<Vec<_>>()
    })?;
    let violations = match completeness {
        ValidationCompleteness::Incremental => contract.validate_incremental(source_config),
        ValidationCompleteness::Complete => contract.validate_complete(source_config),
    };
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations
            .into_iter()
            .map(|violation| detection_value_diagnostic(violation, base_path))
            .collect())
    }
}

pub(super) fn compiler_definition_diagnostic(violation: ContractViolation) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Compiler,
        code: violation.code.to_string(),
        message: violation.message,
        severity: DiagnosticSeverity::Error,
        path: violation.path,
        strategy_key: None,
        details: Some(violation.details),
    }
}

fn detection_value_diagnostic(violation: ContractViolation, base_path: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: violation.code.to_string(),
        message: violation.message,
        severity: DiagnosticSeverity::Error,
        path: format!("{base_path}/sourceConfig{}", violation.path),
        strategy_key: None,
        details: Some(violation.details),
    }
}

pub(super) fn build_source_config(
    input_url: &str,
    profile: &SourceProfileDocument,
    access_path: Option<&ReusableAccessPathDocument>,
    detection: &DetectionDocument,
    captures: &BTreeMap<String, String>,
) -> Result<Value, TemplateError> {
    let Some(template) = &detection.source_config else {
        return Ok(Value::Object(default_source_config(
            profile,
            access_path,
            input_url,
            captures,
        )));
    };
    render_json_object_templates(template, input_url, captures).map(Value::Object)
}

fn default_source_config(
    profile: &SourceProfileDocument,
    access_path: Option<&ReusableAccessPathDocument>,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Map<String, Value> {
    let mut source_config = Map::new();
    insert_default_source_config_values(
        &mut source_config,
        profile.source_config_schema.as_ref(),
        input_url,
        captures,
    );
    if let Some(access_path) = access_path {
        insert_default_source_config_values(
            &mut source_config,
            access_path.source_config_schema.as_ref(),
            input_url,
            captures,
        );
    }
    source_config
}

fn insert_default_source_config_values(
    source_config: &mut Map<String, Value>,
    schema: Option<&Map<String, Value>>,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) {
    if let Some(properties) = schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
    {
        for key in properties.keys() {
            if source_config.contains_key(key) {
                continue;
            }
            if let Some(value) = captures.get(key) {
                source_config.insert(key.clone(), Value::String(value.clone()));
            } else if key == "startUrl" {
                source_config.insert(key.clone(), Value::String(input_url.to_string()));
            }
        }
    }
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

fn render_key_candidate_templates<F>(
    templates: Option<&[String]>,
    default: F,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    path: &str,
) -> Result<Vec<String>, Diagnostic>
where
    F: FnOnce() -> Vec<String>,
{
    let rendered_candidates =
        render_candidate_templates(templates, default, input_url, captures, path)?;
    let mut key_candidates = Vec::new();
    for candidate in rendered_candidates {
        let key_candidate = to_technical_key(&candidate);
        if !key_candidates
            .iter()
            .any(|candidate| candidate == &key_candidate)
        {
            key_candidates.push(key_candidate);
        }
    }
    Ok(key_candidates)
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
    detection: &DetectionDocument,
) -> Vec<SourceProposalEvidence> {
    detection
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
