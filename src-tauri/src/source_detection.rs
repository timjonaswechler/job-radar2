use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::source_model::{get_system_profile, list_system_profiles, SourceStatus, SystemProfile};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDetectionStatus {
    Detected,
    Ambiguous,
    Unsupported,
    BuiltInSource,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetectionResult {
    pub status: SourceDetectionStatus,
    pub adapter_key: Option<String>,
    pub system_profile_id: Option<i64>,
    pub system_profile_key: Option<String>,
    pub key: Option<String>,
    pub name: Option<String>,
    pub source_config: Option<Value>,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
    pub matches: Vec<SourceDetectionMatch>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetectionMatch {
    pub adapter_key: String,
    pub system_profile_id: i64,
    pub system_profile_key: String,
    pub system_profile_name: String,
    pub key: String,
    pub name: String,
    pub source_config: Value,
    pub evidence: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemProfileTestStatus {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemProfileTestCheckStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProfileTestResult {
    pub status: SystemProfileTestStatus,
    pub adapter_key: String,
    pub system_profile_id: i64,
    pub system_profile_key: String,
    pub system_profile_name: String,
    pub key: Option<String>,
    pub name: Option<String>,
    pub source_config: Option<Value>,
    pub checks: Vec<SystemProfileTestCheckResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProfileTestCheckResult {
    pub index: usize,
    pub check: Value,
    pub status: SystemProfileTestCheckStatus,
    pub evidence: Option<String>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct DetectionCheckOutcome {
    status: SystemProfileTestCheckStatus,
    evidence: Option<String>,
    diagnostic: Option<String>,
}

impl DetectionCheckOutcome {
    fn passed(evidence: impl Into<String>) -> Self {
        Self {
            status: SystemProfileTestCheckStatus::Passed,
            evidence: Some(evidence.into()),
            diagnostic: None,
        }
    }

    fn failed(diagnostic: impl Into<String>) -> Self {
        Self {
            status: SystemProfileTestCheckStatus::Failed,
            evidence: None,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TemplateRenderError {
    MissingCapture(String),
    Invalid(String),
}

impl TemplateRenderError {
    fn message(&self) -> String {
        match self {
            Self::MissingCapture(capture_key) => {
                format!("sourceConfig references missing capture `{capture_key}`")
            }
            Self::Invalid(message) => message.clone(),
        }
    }
}

pub async fn detect_source_from_url(
    pool: &SqlitePool,
    input: &str,
) -> Result<SourceDetectionResult, String> {
    let input_url = parse_http_url(input)?;
    if let Some(message) = built_in_source_message(&input_url) {
        return Ok(SourceDetectionResult {
            status: SourceDetectionStatus::BuiltInSource,
            adapter_key: None,
            system_profile_id: None,
            system_profile_key: None,
            key: None,
            name: None,
            source_config: None,
            evidence: vec![message],
            warnings: Vec::new(),
            matches: Vec::new(),
        });
    }

    let profiles = list_system_profiles(pool)
        .await?
        .into_iter()
        .filter(|profile| profile.status == SourceStatus::Active)
        .collect::<Vec<_>>();
    let client = ReqwestDetectionHttpClient::new()?;
    detect_with_profiles(&client, &input_url, &profiles).await
}

pub async fn test_url_against_system_profile(
    pool: &SqlitePool,
    input: &str,
    system_profile_id: i64,
) -> Result<SystemProfileTestResult, String> {
    let client = ReqwestDetectionHttpClient::new()?;
    test_url_against_system_profile_with_client(pool, &client, input, system_profile_id).await
}

async fn test_url_against_system_profile_with_client<C: DetectionHttpClient + Sync>(
    pool: &SqlitePool,
    client: &C,
    input: &str,
    system_profile_id: i64,
) -> Result<SystemProfileTestResult, String> {
    let input_url = parse_http_url(input)?;
    let profile = get_system_profile(pool, system_profile_id).await?;
    test_system_profile_with_client(client, &input_url, &profile).await
}

async fn detect_with_profiles<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    profiles: &[SystemProfile],
) -> Result<SourceDetectionResult, String> {
    let html = client.get_text(input_url.clone()).await?;
    let mut matches = Vec::new();
    let mut warnings = Vec::new();

    for profile in profiles {
        match evaluate_profile(client, input_url, &html, profile).await {
            Ok(Some(candidate)) => matches.push(candidate),
            Ok(None) => {}
            Err(error) => warnings.push(format!("{}: {error}", profile.key)),
        }
    }

    if matches.is_empty() {
        return Ok(SourceDetectionResult {
            status: SourceDetectionStatus::Unsupported,
            adapter_key: None,
            system_profile_id: None,
            system_profile_key: None,
            key: None,
            name: None,
            source_config: None,
            evidence: Vec::new(),
            warnings,
            matches,
        });
    }

    if matches.len() > 1 {
        return Ok(SourceDetectionResult {
            status: SourceDetectionStatus::Ambiguous,
            adapter_key: None,
            system_profile_id: None,
            system_profile_key: None,
            key: None,
            name: None,
            source_config: None,
            evidence: Vec::new(),
            warnings,
            matches,
        });
    }

    let detected = matches[0].clone();
    Ok(SourceDetectionResult {
        status: SourceDetectionStatus::Detected,
        adapter_key: Some(detected.adapter_key.clone()),
        system_profile_id: Some(detected.system_profile_id),
        system_profile_key: Some(detected.system_profile_key.clone()),
        key: Some(detected.key.clone()),
        name: Some(detected.name.clone()),
        source_config: Some(detected.source_config.clone()),
        evidence: detected.evidence.clone(),
        warnings,
        matches,
    })
}

async fn evaluate_profile<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    profile: &SystemProfile,
) -> Result<Option<SourceDetectionMatch>, String> {
    let required = required_detection_checks(profile)?;

    let mut captures = HashMap::new();
    let mut evidence = Vec::new();

    for check in required {
        let Some(check_evidence) =
            evaluate_check(client, input_url, html, check, &mut captures).await?
        else {
            return Ok(None);
        };
        evidence.push(check_evidence);
    }

    evidence.extend(
        enrich_identity_captures(client, input_url, html, &profile.definition, &mut captures)
            .await?,
    );
    let source_config = build_source_config(&profile.definition, input_url, &captures)?;
    let identity = derive_source_identity(&profile.definition, input_url, &captures)?;
    Ok(Some(SourceDetectionMatch {
        adapter_key: profile.adapter_key.clone(),
        system_profile_id: profile.id,
        system_profile_key: profile.key.clone(),
        system_profile_name: profile.name.clone(),
        key: identity.key,
        name: identity.name,
        source_config,
        evidence,
    }))
}

async fn test_system_profile_with_client<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    profile: &SystemProfile,
) -> Result<SystemProfileTestResult, String> {
    let html = client.get_text(input_url.clone()).await?;
    let required = required_detection_checks(profile)?;
    let mut captures = HashMap::new();
    let mut checks = Vec::new();

    for (index, check) in required.iter().enumerate() {
        let outcome = evaluate_check_outcome(client, input_url, &html, check, &mut captures)
            .await
            .unwrap_or_else(DetectionCheckOutcome::failed);
        checks.push(SystemProfileTestCheckResult {
            index: index + 1,
            check: check.clone(),
            status: outcome.status,
            evidence: outcome.evidence,
            diagnostic: outcome.diagnostic,
        });
    }

    let status = if checks
        .iter()
        .all(|check| check.status == SystemProfileTestCheckStatus::Passed)
    {
        SystemProfileTestStatus::Passed
    } else {
        SystemProfileTestStatus::Failed
    };

    let (key, name, source_config) = if status == SystemProfileTestStatus::Passed {
        enrich_identity_captures(client, input_url, &html, &profile.definition, &mut captures)
            .await?;
        let identity = derive_source_identity(&profile.definition, input_url, &captures)?;
        (
            Some(identity.key),
            Some(identity.name),
            Some(build_source_config(
                &profile.definition,
                input_url,
                &captures,
            )?),
        )
    } else {
        (None, None, None)
    };

    Ok(SystemProfileTestResult {
        status,
        adapter_key: profile.adapter_key.clone(),
        system_profile_id: profile.id,
        system_profile_key: profile.key.clone(),
        system_profile_name: profile.name.clone(),
        key,
        name,
        source_config,
        checks,
    })
}

fn required_detection_checks(profile: &SystemProfile) -> Result<&[Value], String> {
    profile
        .definition
        .pointer("/detect/required")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "definition.detect.required must be an array".to_string())
}

async fn evaluate_check<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    check: &Value,
    captures: &mut HashMap<String, String>,
) -> Result<Option<String>, String> {
    let outcome = evaluate_check_outcome(client, input_url, html, check, captures).await?;
    if outcome.status == SystemProfileTestCheckStatus::Passed {
        Ok(outcome.evidence)
    } else {
        Ok(None)
    }
}

async fn evaluate_check_outcome<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    check: &Value,
    captures: &mut HashMap<String, String>,
) -> Result<DetectionCheckOutcome, String> {
    if let Some(needle) = check.get("htmlContains").and_then(Value::as_str) {
        return Ok(if contains_text(html, needle) {
            DetectionCheckOutcome::passed(format!("HTML enthält `{needle}`"))
        } else {
            DetectionCheckOutcome::failed(format!("HTML enthält `{needle}` nicht"))
        });
    }

    if let Some(pattern) = check.get("htmlRegex").and_then(Value::as_str) {
        return evaluate_regex_outcome("HTML", html, pattern, check.get("captureAs"), captures);
    }

    if let Some(fetch_text) = check.get("fetchText").and_then(Value::as_object) {
        let url = required_string(fetch_text, "url")?;
        let fetched_url = input_url
            .join(url)
            .map_err(|error| format!("fetchText.url is invalid: {error}"))?;
        let text = match client.get_text(fetched_url.clone()).await {
            Ok(text) => text,
            Err(error) => {
                return Ok(DetectionCheckOutcome::failed(format!(
                    "{} konnte nicht geladen werden: {error}",
                    fetched_url.as_str()
                )))
            }
        };
        if let Some(needle) = fetch_text.get("contains").and_then(Value::as_str) {
            return Ok(if contains_text(&text, needle) {
                DetectionCheckOutcome::passed(format!(
                    "{} enthält `{needle}`",
                    fetched_url.as_str()
                ))
            } else {
                DetectionCheckOutcome::failed(format!(
                    "{} enthält `{needle}` nicht",
                    fetched_url.as_str()
                ))
            });
        }
        if let Some(pattern) = fetch_text.get("regex").and_then(Value::as_str) {
            return evaluate_regex_outcome(
                fetched_url.as_str(),
                &text,
                pattern,
                fetch_text.get("captureAs"),
                captures,
            );
        }
        return Ok(DetectionCheckOutcome::passed(format!(
            "{} wurde erfolgreich geladen",
            fetched_url.as_str()
        )));
    }

    if let Some(fetch_json) = check.get("fetchJson").and_then(Value::as_object) {
        let url = required_string(fetch_json, "url")?;
        let fetched_url = input_url
            .join(url)
            .map_err(|error| format!("fetchJson.url is invalid: {error}"))?;
        let text = match client.get_text(fetched_url.clone()).await {
            Ok(text) => text,
            Err(error) => {
                return Ok(DetectionCheckOutcome::failed(format!(
                    "{} konnte nicht geladen werden: {error}",
                    fetched_url.as_str()
                )))
            }
        };
        let json: Value = match serde_json::from_str(&text) {
            Ok(json) => json,
            Err(error) => {
                return Ok(DetectionCheckOutcome::failed(format!(
                    "{} lieferte kein gültiges JSON: {error}",
                    fetched_url.as_str()
                )))
            }
        };
        if let Some(path) = fetch_json.get("pathExists").and_then(Value::as_str) {
            return Ok(if json_path_exists(&json, path) {
                DetectionCheckOutcome::passed(format!(
                    "{} enthält JSON-Pfad `{path}`",
                    fetched_url.as_str()
                ))
            } else {
                DetectionCheckOutcome::failed(format!(
                    "{} enthält JSON-Pfad `{path}` nicht",
                    fetched_url.as_str()
                ))
            });
        }
        return Ok(DetectionCheckOutcome::passed(format!(
            "{} lieferte gültiges JSON",
            fetched_url.as_str()
        )));
    }

    if let Some(fetch_script) = check.get("fetchScript").and_then(Value::as_object) {
        return evaluate_fetch_script(client, input_url, html, fetch_script, captures).await;
    }

    Err("unsupported detection check".to_string())
}

async fn evaluate_fetch_script<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    fetch_script: &Map<String, Value>,
    captures: &mut HashMap<String, String>,
) -> Result<DetectionCheckOutcome, String> {
    let src_contains = fetch_script.get("srcContains").and_then(Value::as_str);
    let src_regex = match fetch_script.get("srcRegex").and_then(Value::as_str) {
        Some(pattern) => Some(
            Regex::new(pattern)
                .map_err(|error| format!("invalid fetchScript.srcRegex `{pattern}`: {error}"))?,
        ),
        None => None,
    };
    let script_urls = extract_script_srcs(html)
        .into_iter()
        .filter(|src| {
            src_contains
                .map(|needle| contains_text(src, needle))
                .unwrap_or(true)
        })
        .filter(|src| {
            src_regex
                .as_ref()
                .map(|regex| regex.is_match(src))
                .unwrap_or(true)
        })
        .filter_map(|src| input_url.join(&src).ok())
        .collect::<Vec<_>>();

    if script_urls.is_empty() {
        return Ok(DetectionCheckOutcome::failed(
            "Kein Script-src passte zu den fetchScript-Kriterien",
        ));
    }

    let mut fetch_errors = Vec::new();
    let mut loaded_any = false;
    for script_url in script_urls {
        let script_text = match client.get_text(script_url.clone()).await {
            Ok(script_text) => {
                loaded_any = true;
                script_text
            }
            Err(error) => {
                fetch_errors.push(format!("{}: {error}", script_url.as_str()));
                continue;
            }
        };
        if let Some(needle) = fetch_script.get("contains").and_then(Value::as_str) {
            if contains_text(&script_text, needle) {
                return Ok(DetectionCheckOutcome::passed(format!(
                    "Script {} enthält `{needle}`",
                    script_url.as_str()
                )));
            }
        }
        if let Some(pattern) = fetch_script.get("regex").and_then(Value::as_str) {
            if let Some(evidence) = evaluate_regex(
                script_url.as_str(),
                &script_text,
                pattern,
                fetch_script.get("captureAs"),
                captures,
            )? {
                return Ok(DetectionCheckOutcome::passed(format!("Script {evidence}")));
            }
        }
        if fetch_script.get("contains").is_none() && fetch_script.get("regex").is_none() {
            return Ok(DetectionCheckOutcome::passed(format!(
                "Script {} wurde erfolgreich geladen",
                script_url.as_str()
            )));
        }
    }

    if !loaded_any && !fetch_errors.is_empty() {
        return Ok(DetectionCheckOutcome::failed(format!(
            "Passende Scripts konnten nicht geladen werden: {}",
            fetch_errors.join("; ")
        )));
    }

    Ok(DetectionCheckOutcome::failed(
        "Keines der passenden Scripts erfüllte die fetchScript-Inhaltsprüfung",
    ))
}

fn extract_script_srcs(html: &str) -> Vec<String> {
    let script_src_regex = Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#).unwrap();
    script_src_regex
        .captures_iter(html)
        .filter_map(|captures| captures.get(1).map(|src| src.as_str().to_string()))
        .collect()
}

fn contains_text(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(&needle.to_lowercase())
}

fn evaluate_regex(
    label: &str,
    text: &str,
    pattern: &str,
    capture_as: Option<&Value>,
    captures: &mut HashMap<String, String>,
) -> Result<Option<String>, String> {
    let regex =
        Regex::new(pattern).map_err(|error| format!("invalid regex `{pattern}`: {error}"))?;
    let Some(regex_captures) = regex.captures(text) else {
        return Ok(None);
    };

    if let Some(capture_key) = capture_as.and_then(Value::as_str) {
        if let Some(value) = regex_captures.get(1) {
            captures.insert(capture_key.to_string(), value.as_str().to_string());
        }
    }

    Ok(Some(format!("{label} erfüllt Regex `{pattern}`")))
}

fn evaluate_regex_outcome(
    label: &str,
    text: &str,
    pattern: &str,
    capture_as: Option<&Value>,
    captures: &mut HashMap<String, String>,
) -> Result<DetectionCheckOutcome, String> {
    Ok(
        match evaluate_regex(label, text, pattern, capture_as, captures)? {
            Some(evidence) => DetectionCheckOutcome::passed(evidence),
            None => {
                DetectionCheckOutcome::failed(format!("{label} erfüllt Regex `{pattern}` nicht"))
            }
        },
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceIdentity {
    key: String,
    name: String,
}

async fn enrich_identity_captures<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    definition: &Value,
    captures: &mut HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let Some(extracts) = definition
        .pointer("/identity/extract")
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    let mut evidence = Vec::new();
    for extract in extracts {
        let outcome = evaluate_check_outcome(client, input_url, html, extract, captures).await?;
        if outcome.status == SystemProfileTestCheckStatus::Passed {
            if let Some(extract_evidence) = outcome.evidence {
                evidence.push(format!("Identität: {extract_evidence}"));
            }
        }
    }

    Ok(evidence)
}

fn derive_source_identity(
    definition: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<SourceIdentity, String> {
    let fallback_company_name = derive_company_name(input_url);
    let fallback_key = format!("{}_careers", to_technical_key(&fallback_company_name));
    let fallback_name = format!("{fallback_company_name} Karriere");

    let key = render_first_identity_candidate(
        definition.pointer("/identity/keyCandidates"),
        input_url,
        captures,
    )?
    .map(|candidate| to_technical_key(&candidate))
    .filter(|candidate| !candidate.is_empty())
    .unwrap_or(fallback_key);

    let name = render_first_identity_candidate(
        definition.pointer("/identity/nameCandidates"),
        input_url,
        captures,
    )?
    .filter(|candidate| !candidate.trim().is_empty())
    .unwrap_or(fallback_name);

    Ok(SourceIdentity { key, name })
}

fn render_first_identity_candidate(
    candidates_value: Option<&Value>,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Option<String>, String> {
    let Some(candidates) = candidates_value.and_then(Value::as_array) else {
        return Ok(None);
    };

    for candidate in candidates.iter().filter_map(Value::as_str) {
        match render_template(candidate, input_url, captures) {
            Ok(rendered) => {
                let rendered = rendered.trim();
                if !rendered.is_empty() {
                    return Ok(Some(rendered.to_string()));
                }
            }
            Err(TemplateRenderError::MissingCapture(_)) => continue,
            Err(error) => return Err(error.message()),
        }
    }

    Ok(None)
}

fn build_source_config(
    definition: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, String> {
    let template = definition.get("sourceConfig").unwrap_or(&Value::Null);
    let mut source_config = if let Some(object) = template.as_object() {
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

    merge_optional_source_config(&mut source_config, definition, input_url, captures)?;
    Ok(source_config)
}

fn merge_optional_source_config(
    source_config: &mut Value,
    definition: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<(), String> {
    let Some(optional_config) = definition.pointer("/identity/optionalSourceConfig") else {
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

fn render_template_value(
    value: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, String> {
    match value {
        Value::String(template) => Ok(Value::String(
            render_template(template, input_url, captures).map_err(|error| error.message())?,
        )),
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
            .collect::<Result<Map<_, _>, String>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

fn render_optional_template_value(
    value: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Option<Value>, String> {
    match value {
        Value::String(template) => match render_template(template, input_url, captures) {
            Ok(rendered) => Ok(Some(Value::String(rendered))),
            Err(TemplateRenderError::MissingCapture(_)) => Ok(None),
            Err(error) => Err(error.message()),
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

fn render_template(
    template: &str,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<String, TemplateRenderError> {
    let placeholder_regex = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut first_error = None;
    let rendered =
        placeholder_regex
            .replace_all(template, |placeholder: &regex::Captures<'_>| {
                match render_template_expression(&placeholder[1], input_url, captures) {
                    Ok(value) => value,
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                        String::new()
                    }
                }
            })
            .to_string();

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(rendered)
    }
}

fn render_template_expression(
    expression: &str,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<String, TemplateRenderError> {
    let mut parts = expression.split('|').map(str::trim);
    let Some(variable) = parts.next().filter(|variable| !variable.is_empty()) else {
        return Err(TemplateRenderError::Invalid(
            "template expression must not be empty".to_string(),
        ));
    };

    let mut value = if variable == "inputUrl" {
        input_url.as_str().to_string()
    } else if variable == "origin" {
        origin(input_url)
    } else if let Some(capture_key) = variable.strip_prefix("capture:") {
        if capture_key.is_empty() {
            return Err(TemplateRenderError::Invalid(
                "capture template variable must include a capture name".to_string(),
            ));
        }
        captures
            .get(capture_key)
            .cloned()
            .ok_or_else(|| TemplateRenderError::MissingCapture(capture_key.to_string()))?
    } else {
        return Err(TemplateRenderError::Invalid(format!(
            "unsupported template variable `{variable}`"
        )));
    };

    for filter in parts {
        if filter.is_empty() {
            return Err(TemplateRenderError::Invalid(
                "template filter must not be empty".to_string(),
            ));
        }
        value = apply_template_filter(filter, &value)?;
    }

    Ok(value)
}

fn apply_template_filter(filter: &str, value: &str) -> Result<String, TemplateRenderError> {
    match filter {
        "technicalKey" => Ok(to_technical_key(value)),
        "titleCase" => Ok(title_case(value)),
        "domainKey" => Ok(to_technical_key(&company_domain_label(value)?)),
        "domainTitle" => Ok(title_case(&company_domain_label(value)?)),
        _ => Err(TemplateRenderError::Invalid(format!(
            "unsupported template filter `{filter}`"
        ))),
    }
}

fn company_domain_label(value: &str) -> Result<String, TemplateRenderError> {
    let url = parse_http_url(value).map_err(|error| {
        TemplateRenderError::Invalid(format!(
            "template domain filter requires an HTTP(S) URL: {error}"
        ))
    })?;
    let host = normalized_host(&url);
    let label = host
        .split('.')
        .find(|label| !is_generic_host_label(label))
        .or_else(|| host.split('.').next())
        .unwrap_or_default();

    if label.is_empty() {
        Err(TemplateRenderError::Invalid(
            "template domain filter could not derive a domain label".to_string(),
        ))
    } else {
        Ok(label.to_string())
    }
}

fn json_default_source_config(input_url: &Url) -> Value {
    serde_json::json!({ "startUrl": input_url.as_str() })
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} is required"))
}

fn json_path_exists(value: &Value, path: &str) -> bool {
    let Some(path) = path.strip_prefix("$.") else {
        return false;
    };

    let mut current = value;
    for segment in path.split('.') {
        let Some(next) = current.get(segment) else {
            return false;
        };
        current = next;
    }
    true
}

fn parse_http_url(input: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string());
    }
    let with_protocol = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let url = Url::parse(&with_protocol)
        .map_err(|_| "Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        Ok(url)
    } else {
        Err("Bitte eine absolute HTTP- oder HTTPS-URL einfügen.".to_string())
    }
}

fn built_in_source_message(url: &Url) -> Option<String> {
    let host = normalized_host(url);
    if host.contains("stepstone") || host.contains("indeed") {
        Some("StepStone und Indeed sind bereits als eingebaute Quellen vorhanden.".to_string())
    } else {
        None
    }
}

fn origin(url: &Url) -> String {
    match url.port() {
        Some(port) => format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            port
        ),
        None => format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default()),
    }
}

fn derive_company_name(url: &Url) -> String {
    let host = normalized_host(url);
    let label = host
        .split('.')
        .find(|label| !is_generic_host_label(label))
        .unwrap_or("neue_quelle");
    title_case(label)
}

fn is_generic_host_label(label: &str) -> bool {
    matches!(
        label,
        "www"
            | "app"
            | "api"
            | "jobs"
            | "job"
            | "careers"
            | "career"
            | "join"
            | "boards"
            | "job-boards"
    )
}

fn normalized_host(url: &Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .to_lowercase()
        .strip_prefix("www.")
        .unwrap_or_else(|| url.host_str().unwrap_or_default())
        .to_string()
}

fn title_case(value: &str) -> String {
    let words = value
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();

    if words.is_empty() {
        "Neue Quelle".to_string()
    } else {
        words.join(" ")
    }
}

fn to_technical_key(value: &str) -> String {
    let mut key = String::new();
    let mut last_was_separator = false;
    for ch in value.to_lowercase().chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'ä' => Some('a'),
            'ö' => Some('o'),
            'ü' => Some('u'),
            'ß' => {
                key.push_str("ss");
                last_was_separator = false;
                None
            }
            _ => None,
        };

        if let Some(ch) = mapped {
            key.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !key.is_empty() {
            key.push('_');
            last_was_separator = true;
        }
    }

    let key = key.trim_matches('_').to_string();
    if key.is_empty() {
        "quelle".to_string()
    } else {
        key
    }
}

type BoxedTextFuture<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

trait DetectionHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_>;
}

struct ReqwestDetectionHttpClient {
    client: reqwest::Client,
}

impl ReqwestDetectionHttpClient {
    fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("JobRadarSourceDetection/0.1")
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

impl DetectionHttpClient for ReqwestDetectionHttpClient {
    fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
        Box::pin(async move {
            let mut last_error = None;
            for attempt in 0..3 {
                match self.client.get(url.clone()).send().await {
                    Ok(response) if response.status().is_success() => {
                        return response.text().await.map_err(|error| error.to_string());
                    }
                    Ok(response) => {
                        last_error = Some(format!(
                            "{} returned HTTP {}",
                            url.as_str(),
                            response.status()
                        ));
                    }
                    Err(error) => {
                        last_error =
                            Some(format!("{} could not be fetched: {error}", url.as_str()));
                    }
                }

                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }

            Err(last_error.unwrap_or_else(|| format!("{} could not be fetched", url.as_str())))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_model::{
        create_source, create_system_profile, list_sources, CreateSourceInput,
        CreateSystemProfileInput,
    };
    use serde::Deserialize;
    use serde_json::json;
    use sqlx::{
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
        SqlitePool,
    };

    struct FixtureHttpClient {
        responses: HashMap<String, String>,
    }

    impl FixtureHttpClient {
        fn new(responses: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
            Self {
                responses: responses
                    .into_iter()
                    .map(|(url, body)| (url.to_string(), body.to_string()))
                    .collect(),
            }
        }
    }

    impl DetectionHttpClient for FixtureHttpClient {
        fn get_text(&self, url: Url) -> BoxedTextFuture<'_> {
            Box::pin(async move {
                self.responses
                    .get(url.as_str())
                    .cloned()
                    .ok_or_else(|| format!("{} not found", url.as_str()))
            })
        }
    }

    #[test]
    fn tests_selected_system_profile_successfully_without_persisting_a_source() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let profile = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "example_board".to_string(),
                    name: "Example Board".to_string(),
                    description: None,
                    adapter_key: "declarative_http_jobboard".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [
                            { "htmlContains": "example-board-root" },
                            { "fetchText": {
                                "url": "/assets/board.js",
                                "regex": "apiBase\\s*=\\s*\"([^\"]+)\"",
                                "captureAs": "apiBaseUrl"
                            }}
                        ]},
                        "sourceConfig": {
                            "startUrl": "{{inputUrl}}",
                            "apiBaseUrl": "{{capture:apiBaseUrl}}"
                        }
                    }),
                    source_config_schema: json!({}),
                    status: SourceStatus::Draft,
                    validation_error: None,
                },
            )
            .await
            .unwrap();
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/jobs",
                    r#"<main id="example-board-root"></main>"#,
                ),
                (
                    "https://example.com/assets/board.js",
                    r#"window.apiBase = "https://api.example.com/jobs";"#,
                ),
            ]);

            let result = test_url_against_system_profile_with_client(
                &pool,
                &client,
                "https://example.com/jobs",
                profile.id,
            )
            .await
            .unwrap();

            assert_eq!(result.status, SystemProfileTestStatus::Passed);
            assert_eq!(result.system_profile_key, "example_board");
            assert_eq!(result.checks.len(), 2);
            assert!(result
                .checks
                .iter()
                .all(|check| check.status == SystemProfileTestCheckStatus::Passed));
            assert_eq!(
                result.checks[0].evidence.as_deref(),
                Some("HTML enthält `example-board-root`")
            );
            assert_eq!(result.checks[0].diagnostic, None);
            assert_eq!(
                result.source_config.unwrap()["apiBaseUrl"],
                "https://api.example.com/jobs"
            );
            assert!(list_sources(&pool).await.unwrap().is_empty());
        });
    }

    #[test]
    fn tests_selected_system_profile_reports_failed_required_check_without_source_config() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let profile = create_system_profile(
                &pool,
                CreateSystemProfileInput {
                    key: "example_board".to_string(),
                    name: "Example Board".to_string(),
                    description: None,
                    adapter_key: "declarative_http_jobboard".to_string(),
                    definition_schema_version: 1,
                    definition: json!({
                        "detect": { "required": [
                            { "htmlContains": "example-board-root" },
                            { "fetchText": {
                                "url": "/assets/board.js",
                                "contains": "requiredApiToken"
                            }}
                        ]},
                        "sourceConfig": { "startUrl": "{{inputUrl}}" }
                    }),
                    source_config_schema: json!({}),
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/jobs",
                    r#"<main id="example-board-root"></main>"#,
                ),
                (
                    "https://example.com/assets/board.js",
                    "console.log('different token')",
                ),
            ]);

            let result = test_url_against_system_profile_with_client(
                &pool,
                &client,
                "https://example.com/jobs",
                profile.id,
            )
            .await
            .unwrap();

            assert_eq!(result.status, SystemProfileTestStatus::Failed);
            assert_eq!(result.source_config, None);
            assert_eq!(
                result.checks[0].status,
                SystemProfileTestCheckStatus::Passed
            );
            assert_eq!(
                result.checks[1].status,
                SystemProfileTestCheckStatus::Failed
            );
            assert_eq!(
                result.checks[0].evidence.as_deref(),
                Some("HTML enthält `example-board-root`")
            );
            assert!(result.checks[1]
                .diagnostic
                .as_deref()
                .unwrap()
                .contains("requiredApiToken` nicht"));
            assert!(list_sources(&pool).await.unwrap().is_empty());
        });
    }

    #[test]
    fn detects_greenhouse_ashby_and_lever_with_vendor_board_or_api_evidence_and_creatable_config() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let scenarios = [
                (
                    create_builtin_system_profile(
                        &pool,
                        include_str!("../../system-profiles/builtin/greenhouse.json"),
                    )
                    .await,
                    "https://openai.com/careers",
                    r#"
                    <html>
                      <body>
                        <h1>OpenAI Careers</h1>
                        <script src="https://boards.greenhouse.io/embed/job_board/js?for=openai"></script>
                        <a href="https://boards.greenhouse.io/openai">Job board</a>
                      </body>
                    </html>
                    "#,
                    "\\.greenhouse\\.io",
                    "https://openai.com/careers",
                ),
                (
                    create_builtin_system_profile(
                        &pool,
                        include_str!("../../system-profiles/builtin/ashby.json"),
                    )
                    .await,
                    "https://ashby-fixture.test/careers",
                    r#"
                    <html>
                      <body>
                        <h1>Example Careers</h1>
                        <iframe src="https://jobs.ashbyhq.com/example"></iframe>
                      </body>
                    </html>
                    "#,
                    "\\.ashbyhq\\.com",
                    "https://api.ashbyhq.com/posting-api/job-board/example?includeCompensation=true",
                ),
                (
                    create_builtin_system_profile(
                        &pool,
                        include_str!("../../system-profiles/builtin/lever.json"),
                    )
                    .await,
                    "https://lever-fixture.test/jobs",
                    r#"
                    <html>
                      <body>
                        <h1>Example Careers</h1>
                        <a href="https://jobs.lever.co/example/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">
                          Senior Rust Engineer
                        </a>
                      </body>
                    </html>
                    "#,
                    "jobs\\.lever\\.co",
                    "https://lever-fixture.test/jobs",
                ),
            ];

            for (profile, input_url, html, expected_evidence_marker, expected_start_url) in
                scenarios
            {
                let client = FixtureHttpClient::new([(input_url, html)]);

                let result = detect_with_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &[profile.clone()],
                )
                .await
                .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Detected);
                assert_eq!(
                    result.system_profile_key.as_deref(),
                    Some(profile.key.as_str())
                );
                assert!(result
                    .evidence
                    .join("\n")
                    .contains(expected_evidence_marker));

                let source = create_source(
                    &pool,
                    CreateSourceInput {
                        key: result.key.unwrap(),
                        adapter_key: result.adapter_key.unwrap(),
                        system_profile_id: result.system_profile_id,
                        browser_profile_id: None,
                        name: result.name.unwrap(),
                        description: None,
                        source_config: result.source_config.unwrap(),
                        status: SourceStatus::Active,
                        validation_error: None,
                    },
                )
                .await
                .unwrap();

                assert_eq!(source.system_profile_id, Some(profile.id));
                assert_eq!(source.adapter_key, "declarative_http_jobboard");
                assert_eq!(source.source_config["startUrl"], expected_start_url);
            }
        });
    }

    #[test]
    fn ashby_identity_uses_public_website_when_hosted_board_exposes_it() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let profile = create_builtin_system_profile(
                &pool,
                include_str!("../../system-profiles/builtin/ashby.json"),
            )
            .await;
            let input_url = "https://jobs.ashbyhq.com/focused";
            let client = FixtureHttpClient::new([(
                input_url,
                r#"
                <html>
                  <head>
                    <meta property="og:url" content="https://jobs.ashbyhq.com/focused" />
                  </head>
                  <body>
                    <script>
                      window.__appData = {"organization":{"name":"Focused","publicWebsite":"https://focused-energy.co","hostedJobsPageSlug":"focused"}};
                    </script>
                  </body>
                </html>
                "#,
            )]);

            let result = detect_with_profiles(&client, &Url::parse(input_url).unwrap(), &[profile])
                .await
                .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.key.as_deref(), Some("focused_energy_careers"));
            assert_eq!(result.name.as_deref(), Some("Focused Energy Karriere"));
            let source_config = result.source_config.unwrap();
            assert_eq!(
                source_config["startUrl"],
                "https://api.ashbyhq.com/posting-api/job-board/focused?includeCompensation=true"
            );
            assert_eq!(source_config["companyWebsite"], "https://focused-energy.co");
            assert!(result.evidence.join("\n").contains("Identität:"));
        });
    }

    #[test]
    fn greenhouse_ashby_and_lever_ignore_generic_vendor_mentions_without_board_or_api_evidence() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/greenhouse-mention",
                    r#"
                    <html>
                      <body>
                        <p>We use a vendor listed at https://www.greenhouse.io/.</p>
                      </body>
                    </html>
                    "#,
                ),
                (
                    "https://example.com/ashby-mention",
                    r#"
                    <html>
                      <body>
                        <p>Read about recruiting tools at https://www.ashbyhq.com/.</p>
                      </body>
                    </html>
                    "#,
                ),
                (
                    "https://example.com/lever-mention",
                    r#"
                    <html>
                      <body>
                        <p>Our old provider lived under https://jobs.lever.co/.</p>
                      </body>
                    </html>
                    "#,
                ),
            ]);
            let profiles = vec![
                builtin_greenhouse_profile(51),
                builtin_ashby_profile(52),
                builtin_lever_profile(53),
            ];

            for input_url in [
                "https://example.com/greenhouse-mention",
                "https://example.com/ashby-mention",
                "https://example.com/lever-mention",
            ] {
                let result =
                    detect_with_profiles(&client, &Url::parse(input_url).unwrap(), &profiles)
                        .await
                        .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Unsupported);
                assert!(result.matches.is_empty());
            }
        });
    }

    #[test]
    fn greenhouse_ashby_and_lever_do_not_detect_company_domain_only_pages() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://openai.com/careers",
                    r#"<html><body><h1>OpenAI Careers</h1><p>Come build with us.</p></body></html>"#,
                ),
                (
                    "https://helsing.ai/careers",
                    r#"<html><body><h1>Helsing Careers</h1><p>Open roles.</p></body></html>"#,
                ),
            ]);
            let profiles = vec![
                builtin_greenhouse_profile(54),
                builtin_ashby_profile(55),
                builtin_lever_profile(56),
            ];

            for input_url in ["https://openai.com/careers", "https://helsing.ai/careers"] {
                let result =
                    detect_with_profiles(&client, &Url::parse(input_url).unwrap(), &profiles)
                        .await
                        .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Unsupported);
                assert!(result.matches.is_empty());
            }
        });
    }

    #[test]
    fn detects_successfactors_with_sap_rmk_html_and_sitemap_evidence() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://careers.example.com/search/",
                    r#"
                    <html>
                      <head>
                        <meta name="generator" content="SAP SuccessFactors Recruiting Marketing">
                        <script src="/platform/js/sap-rmk-careersite.js"></script>
                      </head>
                      <body>
                        <div id="rmk-career-site">Aktuelle Stellen</div>
                      </body>
                    </html>
                    "#,
                ),
                (
                    "https://careers.example.com/sitemap.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                      <url>
                        <loc>https://careers.example.com/job/Berlin-Senior-Rust-Engineer-12345/</loc>
                      </url>
                    </urlset>"#,
                ),
            ]);
            let profile = builtin_successfactors_profile(44);

            let result = detect_with_profiles(
                &client,
                &Url::parse("https://careers.example.com/search/").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.system_profile_key.as_deref(), Some("successfactors"));
            assert_eq!(
                result.adapter_key.as_deref(),
                Some("declarative_sitemap_jobboard")
            );
            assert_eq!(result.evidence.len(), 2);
            assert!(result.evidence[0].contains("HTML erfüllt Regex"));
            assert!(result.evidence[0].contains("SuccessFactors"));
            assert!(result.evidence[1].contains("https://careers.example.com/sitemap.xml"));
            assert!(result.evidence[1].contains("<urlset"));

            let source_config = result.source_config.unwrap();
            assert_eq!(
                source_config["url"],
                "https://careers.example.com/sitemap.xml"
            );
            assert_eq!(source_config["recursive"], false);
        });
    }

    #[test]
    fn successfactors_detection_fails_for_matching_hostname_without_technical_evidence() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://successfactors.example.com/jobs",
                    r#"
                    <html>
                      <body>
                        <h1>Careers</h1>
                        <p>Current openings from our recruiting team.</p>
                      </body>
                    </html>
                    "#,
                ),
                (
                    "https://successfactors.example.com/sitemap.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                      <url>
                        <loc>https://successfactors.example.com/job/Generic-Role-1/</loc>
                      </url>
                    </urlset>"#,
                ),
            ]);
            let profile = builtin_successfactors_profile(45);

            let result = detect_with_profiles(
                &client,
                &Url::parse("https://successfactors.example.com/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
            assert!(result.evidence.is_empty());
        });
    }

    #[test]
    fn detects_commerzbank_muz_with_bundled_profile_and_creates_source() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let profile = create_builtin_muz_system_profile(&pool).await;
            let client = FixtureHttpClient::new([
                (
                    "https://jobs.commerzbank.com/index.php?ac=search_result",
                    r#"
                    <html>
                      <body>
                        <form class="jobboard-container js-job-search-form" method="get">
                          <div class="jobboard-datatable jobboard-widget"
                               data-widget="jobboardDatatable"
                               data-widget-config="configWidgetDataTable"></div>
                        </form>
                        <script src="/script/gjb_scripts.js"></script>
                      </body>
                    </html>
                    "#,
                ),
                (
                    "https://jobs.commerzbank.com/script/gjb_scripts.js",
                    r#"
                    var gjb_apiTokenPayload = "";
                    var gjbAddress = "https://api-jobs.commerzbank.com/";
                    "#,
                ),
                (
                    "https://jobs.commerzbank.com/assets/js/jobboard.config.json",
                    r#"{"configWidgetContainer":{"search":{"endpoint":"placeholder"}}}"#,
                ),
            ]);

            let result = detect_with_profiles(
                &client,
                &Url::parse("https://jobs.commerzbank.com/index.php?ac=search_result").unwrap(),
                &[profile.clone()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(
                result.system_profile_key.as_deref(),
                Some("muz_global_jobboard")
            );
            assert_eq!(
                result.adapter_key.as_deref(),
                Some("declarative_http_jobboard")
            );
            assert_eq!(result.system_profile_id, Some(profile.id));
            assert_eq!(result.evidence.len(), 3);
            let evidence = result.evidence.join("\n");
            assert!(evidence.contains("HTML"));
            assert!(evidence.contains("Script"));
            assert!(evidence.contains("JSON-Pfad"));

            let source_config = result.source_config.clone().unwrap();
            assert_eq!(
                source_config["startUrl"],
                "https://jobs.commerzbank.com/index.php?ac=search_result"
            );
            assert_eq!(
                source_config["apiBaseUrl"],
                "https://api-jobs.commerzbank.com/"
            );
            assert_eq!(
                source_config["configUrl"],
                "https://jobs.commerzbank.com/assets/js/jobboard.config.json"
            );

            let source = create_source(
                &pool,
                CreateSourceInput {
                    key: result.key.unwrap(),
                    adapter_key: result.adapter_key.unwrap(),
                    system_profile_id: result.system_profile_id,
                    browser_profile_id: None,
                    name: result.name.unwrap(),
                    description: None,
                    source_config,
                    status: SourceStatus::Active,
                    validation_error: None,
                },
            )
            .await
            .unwrap();

            assert_eq!(source.system_profile_id, Some(profile.id));
            assert_eq!(source.adapter_key, "declarative_http_jobboard");
            assert_eq!(
                source.source_config["apiBaseUrl"],
                "https://api-jobs.commerzbank.com/"
            );
        });
    }

    #[test]
    fn muz_detection_fails_for_generic_jobs_page_without_technical_evidence() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://jobs.commerzbank.com/generic-careers",
                r#"
                <html>
                  <body>
                    <h1>Jobs und Karriere</h1>
                    <p>Unsere aktuellen Stellenangebote.</p>
                  </body>
                </html>
                "#,
            )]);
            let profile = builtin_muz_profile(42);

            let result = detect_with_profiles(
                &client,
                &Url::parse("https://jobs.commerzbank.com/generic-careers").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
            assert!(result.evidence.is_empty());
        });
    }

    #[test]
    fn muz_source_config_requires_stable_api_and_config_values() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            let profile = create_builtin_muz_system_profile(&pool).await;

            let error = create_source(
                &pool,
                CreateSourceInput {
                    key: "commerzbank_careers".to_string(),
                    adapter_key: "declarative_http_jobboard".to_string(),
                    system_profile_id: Some(profile.id),
                    browser_profile_id: None,
                    name: "Commerzbank Karriere".to_string(),
                    description: None,
                    source_config: json!({
                        "startUrl": "https://jobs.commerzbank.com/index.php?ac=search_result"
                    }),
                    status: SourceStatus::Draft,
                    validation_error: None,
                },
            )
            .await
            .unwrap_err();

            assert!(error.contains("sourceConfig.apiBaseUrl is required"));
        });
    }

    #[test]
    fn detects_magnolia_esmp_job_search_through_script_and_json_endpoint() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://www.ruv.de/karriere/jobsuche?reqPlace=&reqUmkreis=&jobSearchText=",
                    r#"<script type="module" src="/.resources/ruv-magnolia-presse/webresources/js/script.js"></script>"#,
                ),
                (
                    "https://www.ruv.de/.resources/ruv-magnolia-presse/webresources/js/script.js",
                    r#"fetch(window.location.origin+"/.search?index=job",{method:"GET"})"#,
                ),
                (
                    "https://www.ruv.de/.search?index=job&size=1&page=1",
                    r#"{"searchResults":[{"title":"Software Engineer","url":"/karriere/stellenanzeigen/ref1"}],"total":1}"#,
                ),
            ]);
            let profile = SystemProfile {
                id: 43,
                key: "magnolia_esmp_job_search".to_string(),
                name: "Magnolia ESMP Jobsuche".to_string(),
                description: None,
                adapter_key: "declarative_http_jobboard".to_string(),
                definition_schema_version: 1,
                definition: json!({
                    "detect": { "required": [
                        { "fetchScript": {
                            "srcRegex": "/webresources/js/.*script\\.js",
                            "contains": "/.search?index=job"
                        }},
                        { "fetchJson": {
                            "url": "/.search?index=job&size=1&page=1",
                            "pathExists": "$.searchResults"
                        }}
                    ]},
                    "sourceConfig": {
                        "startUrl": "{{inputUrl}}",
                        "endpointUrl": "{{origin}}/.search?index=job",
                        "itemsPath": "$.searchResults",
                        "titlePath": "$.title",
                        "urlPath": "$.url"
                    }
                }),
                source_config_schema: json!({}),
                built_in: true,
                status: SourceStatus::Active,
                validation_error: None,
                created_at: String::new(),
                updated_at: String::new(),
            };

            let result = detect_with_profiles(
                &client,
                &Url::parse(
                    "https://www.ruv.de/karriere/jobsuche?reqPlace=&reqUmkreis=&jobSearchText=",
                )
                .unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(
                result.system_profile_key.as_deref(),
                Some("magnolia_esmp_job_search")
            );
            assert_eq!(
                result.source_config.unwrap()["endpointUrl"],
                "https://www.ruv.de/.search?index=job"
            );
        });
    }

    #[test]
    fn unsupported_when_required_evidence_is_missing() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body>No known system</body></html>"#,
            )]);
            let profile = SystemProfile {
                id: 1,
                key: "example".to_string(),
                name: "Example".to_string(),
                description: None,
                adapter_key: "declarative_http_jobboard".to_string(),
                definition_schema_version: 1,
                definition: json!({
                    "detect": { "required": [{ "htmlContains": "jobboard-widget" }] },
                    "sourceConfig": { "startUrl": "{{inputUrl}}" }
                }),
                source_config_schema: json!({}),
                built_in: false,
                status: SourceStatus::Active,
                validation_error: None,
                created_at: String::new(),
                updated_at: String::new(),
            };
            let result = detect_with_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
        });
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BuiltinSystemProfileSeed {
        key: String,
        name: String,
        description: Option<String>,
        adapter_key: String,
        definition_schema_version: i64,
        definition: Value,
        source_config_schema: Value,
    }

    fn builtin_greenhouse_profile(id: i64) -> SystemProfile {
        builtin_profile_from_json(
            id,
            include_str!("../../system-profiles/builtin/greenhouse.json"),
        )
    }

    fn builtin_ashby_profile(id: i64) -> SystemProfile {
        builtin_profile_from_json(id, include_str!("../../system-profiles/builtin/ashby.json"))
    }

    fn builtin_lever_profile(id: i64) -> SystemProfile {
        builtin_profile_from_json(id, include_str!("../../system-profiles/builtin/lever.json"))
    }

    fn builtin_muz_profile(id: i64) -> SystemProfile {
        builtin_profile_from_json(
            id,
            include_str!("../../system-profiles/builtin/muz_global_jobboard.json"),
        )
    }

    fn builtin_successfactors_profile(id: i64) -> SystemProfile {
        builtin_profile_from_json(
            id,
            include_str!("../../system-profiles/builtin/successfactors.json"),
        )
    }

    fn builtin_profile_from_json(id: i64, contents: &str) -> SystemProfile {
        let seed: BuiltinSystemProfileSeed = serde_json::from_str(contents).unwrap();

        SystemProfile {
            id,
            key: seed.key,
            name: seed.name,
            description: seed.description,
            adapter_key: seed.adapter_key,
            definition_schema_version: seed.definition_schema_version,
            definition: seed.definition,
            source_config_schema: seed.source_config_schema,
            built_in: true,
            status: SourceStatus::Active,
            validation_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    async fn create_builtin_system_profile(pool: &SqlitePool, contents: &str) -> SystemProfile {
        let profile = builtin_profile_from_json(0, contents);
        create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: profile.key,
                name: profile.name,
                description: profile.description,
                adapter_key: profile.adapter_key,
                definition_schema_version: profile.definition_schema_version,
                definition: profile.definition,
                source_config_schema: profile.source_config_schema,
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap()
    }

    async fn create_builtin_muz_system_profile(pool: &SqlitePool) -> SystemProfile {
        create_builtin_system_profile(
            pool,
            include_str!("../../system-profiles/builtin/muz_global_jobboard.json"),
        )
        .await
    }

    async fn migrated_pool() -> SqlitePool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        pool
    }
}
