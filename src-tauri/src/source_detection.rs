use std::{collections::HashMap, future::Future, path::Path, pin::Pin, time::Duration};

use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    declarative_template::{
        render_template, title_case, to_technical_key, TemplateContext, TemplateError,
    },
    simple_json_path::simple_json_path_exists,
    source_registry::{
        self, AvailabilityBlock, DetectionBlock, DetectionPhase, ProfileAccessPathDefinition,
        RegistrySourceProfile, SourceProfileIdentity, SourceRegistryDiagnostic,
        SourceRegistryDocumentKind,
    },
};

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
    pub profile_key: Option<String>,
    pub profile_name: Option<String>,
    pub path_key: Option<String>,
    pub path_name: Option<String>,
    pub key: Option<String>,
    pub name: Option<String>,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub source_config: Option<Value>,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
    pub matches: Vec<SourceDetectionMatch>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetectionMatch {
    pub adapter_key: String,
    pub profile_key: String,
    pub profile_name: String,
    pub path_key: String,
    pub path_name: Option<String>,
    pub key: String,
    pub name: String,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub source_config: Value,
    pub evidence: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DetectionCheckStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
struct DetectionCheckOutcome {
    status: DetectionCheckStatus,
    evidence: Option<String>,
    diagnostic: Option<String>,
}

impl DetectionCheckOutcome {
    fn passed(evidence: impl Into<String>) -> Self {
        Self {
            status: DetectionCheckStatus::Passed,
            evidence: Some(evidence.into()),
            diagnostic: None,
        }
    }

    fn failed(diagnostic: impl Into<String>) -> Self {
        Self {
            status: DetectionCheckStatus::Failed,
            evidence: None,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

#[derive(Default)]
struct ProfileEvaluation {
    matches: Vec<SourceDetectionMatch>,
    warnings: Vec<String>,
}

struct DetectionTemplateContext<'a> {
    input_url: &'a Url,
    captures: &'a HashMap<String, String>,
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

pub async fn detect_source_from_url(
    app_data_dir: impl AsRef<Path>,
    input: &str,
) -> Result<SourceDetectionResult, String> {
    let input_url = parse_http_url(input)?;
    if let Some(message) = built_in_source_message(&input_url) {
        return Ok(SourceDetectionResult {
            status: SourceDetectionStatus::BuiltInSource,
            adapter_key: None,
            profile_key: None,
            profile_name: None,
            path_key: None,
            path_name: None,
            key: None,
            name: None,
            key_candidates: Vec::new(),
            name_candidates: Vec::new(),
            source_config: None,
            evidence: vec![message],
            warnings: Vec::new(),
            matches: Vec::new(),
        });
    }

    let snapshot = source_registry::load_snapshot(app_data_dir);
    let mut registry_warnings = source_profile_registry_warnings(&snapshot.diagnostics);
    let client = ReqwestDetectionHttpClient::new()?;
    let mut result =
        detect_with_source_profiles(&client, &input_url, &snapshot.valid_profiles).await?;
    registry_warnings.append(&mut result.warnings);
    result.warnings = registry_warnings;
    Ok(result)
}

fn source_profile_registry_warnings(diagnostics: &[SourceRegistryDiagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.document_kind == SourceRegistryDocumentKind::SourceProfile)
        .map(|diagnostic| {
            format!(
                "source profile registry diagnostic at {}: {}",
                diagnostic.path, diagnostic.message
            )
        })
        .collect()
}

async fn detect_with_source_profiles<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    profiles: &[RegistrySourceProfile],
) -> Result<SourceDetectionResult, String> {
    let mut warnings = Vec::new();
    let html = match client.get_text(input_url.clone()).await {
        Ok(html) => html,
        Err(error) => {
            warnings.push(format!(
                "initial source URL {} could not be fetched during profile detection: {error}",
                input_url.as_str()
            ));
            String::new()
        }
    };
    let mut matches = Vec::new();

    for profile in profiles {
        match evaluate_source_profile(client, input_url, &html, profile).await {
            Ok(evaluation) => {
                matches.extend(evaluation.matches);
                warnings.extend(evaluation.warnings);
            }
            Err(error) => warnings.push(format!(
                "source profile `{}`: {error}",
                profile.document.key
            )),
        }
    }

    if matches.is_empty() {
        return Ok(SourceDetectionResult {
            status: SourceDetectionStatus::Unsupported,
            adapter_key: None,
            profile_key: None,
            profile_name: None,
            path_key: None,
            path_name: None,
            key: None,
            name: None,
            key_candidates: Vec::new(),
            name_candidates: Vec::new(),
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
            profile_key: None,
            profile_name: None,
            path_key: None,
            path_name: None,
            key: None,
            name: None,
            key_candidates: Vec::new(),
            name_candidates: Vec::new(),
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
        profile_key: Some(detected.profile_key.clone()),
        profile_name: Some(detected.profile_name.clone()),
        path_key: Some(detected.path_key.clone()),
        path_name: detected.path_name.clone(),
        key: Some(detected.key.clone()),
        name: Some(detected.name.clone()),
        key_candidates: detected.key_candidates.clone(),
        name_candidates: detected.name_candidates.clone(),
        source_config: Some(detected.source_config.clone()),
        evidence: detected.evidence.clone(),
        warnings,
        matches,
    })
}

async fn evaluate_source_profile<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    profile: &RegistrySourceProfile,
) -> Result<ProfileEvaluation, String> {
    let document = &profile.document;
    let Some(detect) = &document.detect else {
        return Ok(ProfileEvaluation::default());
    };

    if detect.required.is_empty() && detect.any_of.is_none() {
        return Ok(ProfileEvaluation::default());
    }

    if !detect_supports_http(detect) {
        return Ok(ProfileEvaluation {
            matches: Vec::new(),
            warnings: vec![format!(
                "source profile `{}` declares no HTTP detection phase; browser-assisted profile detection is not implemented yet",
                document.key
            )],
        });
    }

    let mut required_captures = HashMap::new();
    let mut required_evidence = Vec::new();

    for check in &detect.required {
        let Some(check_evidence) =
            evaluate_check(client, input_url, html, check, &mut required_captures).await?
        else {
            return Ok(ProfileEvaluation::default());
        };
        required_evidence.push(check_evidence);
    }

    let Some((captures, evidence)) = evaluate_detection_any_of(
        client,
        input_url,
        html,
        detect.any_of.as_deref(),
        &required_captures,
        &required_evidence,
    )
    .await?
    else {
        return Ok(ProfileEvaluation::default());
    };

    let mut evaluation = ProfileEvaluation::default();
    for access_path in &document.access_paths {
        match evaluate_access_path_availability(
            client,
            input_url,
            html,
            profile,
            access_path,
            &captures,
            &evidence,
        )
        .await
        {
            Ok(Some(candidate)) => evaluation.matches.push(candidate),
            Ok(None) => {}
            Err(error) => evaluation.warnings.push(format!(
                "source profile `{}` access path `{}`: {error}",
                document.key, access_path.key
            )),
        }
    }

    Ok(evaluation)
}

fn detect_supports_http(detect: &DetectionBlock) -> bool {
    detect.phases.is_empty() || detect.phases.contains(&DetectionPhase::Http)
}

async fn evaluate_detection_any_of<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    alternatives: Option<&[Vec<Value>]>,
    required_captures: &HashMap<String, String>,
    required_evidence: &[String],
) -> Result<Option<(HashMap<String, String>, Vec<String>)>, String> {
    let Some(alternatives) = alternatives else {
        return Ok(Some((
            required_captures.clone(),
            required_evidence.to_vec(),
        )));
    };

    for alternative in alternatives {
        let mut captures = required_captures.clone();
        let mut evidence = required_evidence.to_vec();
        let mut passed = true;

        for check in alternative {
            let Some(check_evidence) =
                evaluate_check(client, input_url, html, check, &mut captures).await?
            else {
                passed = false;
                break;
            };
            evidence.push(check_evidence);
        }

        if passed {
            return Ok(Some((captures, evidence)));
        }
    }

    Ok(None)
}

async fn evaluate_access_path_availability<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    profile: &RegistrySourceProfile,
    access_path: &ProfileAccessPathDefinition,
    profile_captures: &HashMap<String, String>,
    profile_evidence: &[String],
) -> Result<Option<SourceDetectionMatch>, String> {
    let mut captures = profile_captures.clone();
    let mut evidence = profile_evidence.to_vec();

    if let Some(availability) = &access_path.availability {
        if !evaluate_availability_checks(
            client,
            input_url,
            html,
            availability,
            &mut captures,
            &mut evidence,
        )
        .await?
        {
            return Ok(None);
        }

        if !required_captures_available(availability, &captures) {
            return Ok(None);
        }
    }

    let source_config_template = access_path
        .availability
        .as_ref()
        .and_then(|availability| availability.source_config.as_ref());
    let source_config = match build_source_config(
        source_config_template,
        profile.document.identity.as_ref(),
        input_url,
        &captures,
    ) {
        Ok(source_config) => source_config,
        Err(error) if is_missing_capture(&error) => return Ok(None),
        Err(error) => return Err(detection_template_error_message(error)),
    };
    if !source_config_satisfies_required_schema(
        &source_config,
        profile.document.source_config_schema.as_ref(),
    ) || !source_config_satisfies_required_schema(
        &source_config,
        access_path.source_config_schema.as_ref(),
    ) {
        return Ok(None);
    }
    let identity =
        derive_source_identity(profile.document.identity.as_ref(), input_url, &captures)?;

    Ok(Some(SourceDetectionMatch {
        adapter_key: access_path.adapter_key.clone(),
        profile_key: profile.document.key.clone(),
        profile_name: profile.document.name.clone(),
        path_key: access_path.key.clone(),
        path_name: access_path.name.clone(),
        key: identity.key,
        name: identity.name,
        key_candidates: identity.key_candidates,
        name_candidates: identity.name_candidates,
        source_config,
        evidence,
    }))
}

async fn evaluate_availability_checks<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    availability: &AvailabilityBlock,
    captures: &mut HashMap<String, String>,
    evidence: &mut Vec<String>,
) -> Result<bool, String> {
    for check in &availability.checks {
        let Some(check_evidence) = evaluate_check(client, input_url, html, check, captures).await?
        else {
            return Ok(false);
        };
        evidence.push(check_evidence);
    }

    Ok(true)
}

fn required_captures_available(
    availability: &AvailabilityBlock,
    captures: &HashMap<String, String>,
) -> bool {
    availability.required_captures.iter().all(|capture_key| {
        captures
            .get(capture_key)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn source_config_satisfies_required_schema(source_config: &Value, schema: Option<&Value>) -> bool {
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

fn source_config_value_is_available(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Null) | None => false,
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(_) => true,
    }
}

async fn evaluate_check<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    check: &Value,
    captures: &mut HashMap<String, String>,
) -> Result<Option<String>, String> {
    let outcome = evaluate_check_outcome(client, input_url, html, check, captures).await?;
    if outcome.status == DetectionCheckStatus::Passed {
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
    if let Some(pattern) = check.get("inputUrlRegex").and_then(Value::as_str) {
        return evaluate_regex_outcome(
            "Eingabe-URL",
            input_url.as_str(),
            pattern,
            check.get("captureAs"),
            captures,
        );
    }

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
            return Ok(if simple_json_path_exists(&json, path) {
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

    Err("unsupported source-profile detection check".to_string())
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
        if let Some(value) = regex_captures
            .iter()
            .skip(1)
            .flatten()
            .find(|value| !value.as_str().trim().is_empty())
        {
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
    key_candidates: Vec<String>,
    name_candidates: Vec<String>,
}

fn derive_source_identity(
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

fn render_identity_candidates(
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

fn dedupe_preserving_order(values: &mut Vec<String>) {
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

fn build_source_config(
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

fn merge_optional_source_config(
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

fn render_template_value(
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

fn render_optional_template_value(
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

fn render_detection_template(
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

fn is_missing_capture(error: &TemplateError) -> bool {
    error
        .missing_variable()
        .and_then(|variable| variable.strip_prefix("capture:"))
        .is_some()
}

fn detection_template_error_message(error: TemplateError) -> String {
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

fn json_default_source_config(input_url: &Url) -> Value {
    serde_json::json!({ "startUrl": input_url.as_str() })
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} is required"))
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
    let host = url.host_str().unwrap_or_default().to_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
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
    use crate::source_registry::{
        RegistrySourceProfile, SourceProfileDocument, SourceRegistryDocumentOrigin,
    };
    use serde_json::json;

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
    fn source_profile_detection_does_not_recommend_access_path_without_required_capture() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><main id="example-board-root"></main></body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{ "htmlContains": "example-board-root" }]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "requiredCaptures": ["tenant"],
                        "sourceConfig": {
                            "tenant": "{{capture:tenant}}",
                            "startUrl": "{{inputUrl}}"
                        }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
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

    #[test]
    fn source_profile_detection_recommends_path_after_required_capture_and_availability_checks_pass(
    ) {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/jobs",
                    r#"<html><body><script>window.tenant = "acme";</script></body></html>"#,
                ),
                (
                    "https://example.com/api/status.json",
                    r#"{"jobs":[{"title":"Engineer"}]}"#,
                ),
            ]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{
                        "htmlRegex": "tenant\\s*=\\s*\"([^\"]+)\"",
                        "captureAs": "tenant"
                    }]
                },
                "identity": {
                    "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                    "nameCandidates": ["{{capture:tenant|titleCase}}"]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "requiredCaptures": ["tenant"],
                        "checks": [{
                            "fetchJson": {
                                "url": "/api/status.json",
                                "pathExists": "$.jobs"
                            }
                        }],
                        "sourceConfig": {
                            "tenant": "{{capture:tenant}}",
                            "startUrl": "{{origin}}/api/jobs/{{capture:tenant}}"
                        }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("example_board"));
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
            assert_eq!(result.key.as_deref(), Some("acme"));
            assert_eq!(result.name.as_deref(), Some("Acme"));
            assert_eq!(result.key_candidates, vec!["acme"]);
            assert_eq!(result.name_candidates, vec!["Acme"]);
            assert_eq!(result.matches[0].key_candidates, vec!["acme"]);
            assert_eq!(result.matches[0].name_candidates, vec!["Acme"]);
            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["tenant"], "acme");
            assert_eq!(
                source_config["startUrl"],
                "https://example.com/api/jobs/acme"
            );
            assert!(result
                .evidence
                .join("\n")
                .contains("https://example.com/api/status.json"));
        });
    }

    #[test]
    fn source_profile_detection_can_capture_from_input_url() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://acme.example-board.test/jobs",
                r#"<html><body><main>Jobs</main></body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{
                        "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                        "captureAs": "tenant"
                    }]
                },
                "identity": {
                    "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                    "nameCandidates": ["{{capture:tenant|titleCase}}"]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "requiredCaptures": ["tenant"],
                        "sourceConfig": { "tenant": "{{capture:tenant}}" }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://acme.example-board.test/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.key.as_deref(), Some("acme"));
            assert_eq!(result.name.as_deref(), Some("Acme"));
            assert_eq!(result.source_config.unwrap()["tenant"], "acme");
            assert!(result.evidence.join("\n").contains("Eingabe-URL"));
        });
    }

    #[test]
    fn source_profile_detection_can_continue_after_initial_fetch_failure() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://acme.example-board.test/xml?language=en",
                r#"<?xml version="1.0" encoding="UTF-8"?><example-jobs></example-jobs>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [
                        {
                            "inputUrlRegex": "(?i)^https?://([a-z0-9-]+)\\.example-board\\.test(?:[/:?#]|$)",
                            "captureAs": "tenant"
                        },
                        {
                            "fetchText": {
                                "url": "/xml?language=en",
                                "contains": "<example-jobs"
                            }
                        }
                    ]
                },
                "identity": {
                    "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                    "nameCandidates": ["{{capture:tenant|titleCase}}"]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "requiredCaptures": ["tenant"],
                        "sourceConfig": { "tenant": "{{capture:tenant}}" }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://acme.example-board.test/").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.key.as_deref(), Some("acme"));
            assert_eq!(result.source_config.unwrap()["tenant"], "acme");
            let warnings = result.warnings.join("\n");
            assert!(warnings.contains("https://acme.example-board.test/"));
            assert!(warnings.contains("not found"));
        });
    }

    #[test]
    fn source_profile_detection_any_of_first_matching_alternative_wins() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body>
                    <main id="example-board-root"></main>
                    <script>
                      window.firstTenant = "alpha";
                      window.secondTenant = "bravo";
                    </script>
                    <span>first-alt-ready</span>
                </body></html>"#,
            )]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[any_of_profile()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
            assert_eq!(result.key.as_deref(), Some("alpha"));
            assert_eq!(result.name.as_deref(), Some("Alpha"));
            assert_eq!(result.source_config.unwrap()["tenant"], "alpha");
        });
    }

    #[test]
    fn source_profile_detection_any_of_later_alternative_can_match() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body>
                    <main id="example-board-root"></main>
                    <script>
                      window.firstTenant = "alpha";
                      window.secondTenant = "bravo";
                    </script>
                </body></html>"#,
            )]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[any_of_profile()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.key.as_deref(), Some("bravo"));
            assert_eq!(result.name.as_deref(), Some("Bravo"));
            assert_eq!(result.source_config.unwrap()["tenant"], "bravo");
        });
    }

    #[test]
    fn source_profile_detection_any_of_without_matching_alternative_is_unsupported() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><main id="example-board-root"></main></body></html>"#,
            )]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[any_of_profile()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
        });
    }

    #[test]
    fn source_profile_detection_any_of_does_not_bypass_required_checks() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><script>window.firstTenant = "alpha";</script></body></html>"#,
            )]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[any_of_profile()],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
        });
    }

    #[test]
    fn source_profile_detection_captures_first_non_empty_regex_group() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><a href="https://second.example/bravo">Jobs</a></body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{
                        "htmlRegex": "https://(?:first\\.example/([a-z]+)|second\\.example/([a-z]+))",
                        "captureAs": "tenant"
                    }]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "requiredCaptures": ["tenant"],
                        "sourceConfig": {
                            "tenant": "{{capture:tenant}}"
                        }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.source_config.unwrap()["tenant"], "bravo");
        });
    }

    #[test]
    fn source_profile_detection_does_not_recommend_path_when_required_schema_config_is_missing() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><main id="example-board-root"></main></body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{ "htmlContains": "example-board-root" }]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "sourceConfig": { "startUrl": "{{inputUrl}}" }
                    },
                    "sourceConfigSchema": {
                        "type": "object",
                        "required": ["startUrl", "apiBaseUrl"],
                        "properties": {
                            "startUrl": { "type": "string" },
                            "apiBaseUrl": { "type": "string" }
                        }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
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

    #[test]
    fn source_profile_detection_does_not_recommend_path_when_availability_check_fails() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/jobs",
                    r#"<html><body><main id="example-board-root"></main></body></html>"#,
                ),
                ("https://example.com/health.txt", "different token"),
            ]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example_board",
                "name": "Example Board",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{ "htmlContains": "example-board-root" }]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": {
                        "checks": [{
                            "fetchText": {
                                "url": "/health.txt",
                                "contains": "requiredApiToken"
                            }
                        }],
                        "sourceConfig": { "startUrl": "{{inputUrl}}" }
                    }
                }]
            }));

            let result = detect_with_source_profiles(
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

    #[test]
    fn detection_template_context_uses_shared_renderer_and_filters() {
        let input_url = Url::parse("https://jobs.ashbyhq.com/focused").unwrap();
        let captures = HashMap::from([
            ("boardSlug".to_string(), "focused-energy".to_string()),
            (
                "companyWebsite".to_string(),
                "https://focused-energy.co".to_string(),
            ),
        ]);
        let context = DetectionTemplateContext {
            input_url: &input_url,
            captures: &captures,
        };

        let rendered = render_template(
            "{{origin}}|{{capture:companyWebsite|domainKey}}|{{capture:boardSlug|titleCase}}",
            &context,
        )
        .unwrap();

        assert_eq!(
            rendered,
            "https://jobs.ashbyhq.com|focused_energy|Focused Energy"
        );
    }

    #[test]
    fn detects_greenhouse_ashby_and_lever_with_profile_path_and_creatable_config() {
        tauri::async_runtime::block_on(async {
            let scenarios = [
                (
                    builtin_profile("greenhouse"),
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
                    "greenhouse",
                    "endpoint_inventory",
                    "declarative_endpoint_inventory",
                    "\\.greenhouse\\.io",
                    "boardSlug",
                    "openai",
                ),
                (
                    builtin_profile("ashby"),
                    "https://ashby-fixture.test/careers",
                    r#"
                    <html>
                      <body>
                        <h1>Example Careers</h1>
                        <iframe src="https://jobs.ashbyhq.com/example"></iframe>
                      </body>
                    </html>
                    "#,
                    "ashby",
                    "endpoint_inventory",
                    "declarative_endpoint_inventory",
                    "\\.ashbyhq\\.com",
                    "boardSlug",
                    "example",
                ),
                (
                    builtin_profile("lever"),
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
                    "lever",
                    "endpoint_inventory",
                    "declarative_endpoint_inventory",
                    "jobs\\.lever\\.co",
                    "boardSlug",
                    "example",
                ),
            ];

            for (
                profile,
                input_url,
                html,
                expected_profile_key,
                expected_path_key,
                expected_adapter_key,
                expected_evidence_marker,
                expected_source_config_key,
                expected_source_config_value,
            ) in scenarios
            {
                let client = FixtureHttpClient::new([(input_url, html)]);

                let result = detect_with_source_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &[profile],
                )
                .await
                .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Detected);
                assert_eq!(result.profile_key.as_deref(), Some(expected_profile_key));
                assert_eq!(result.path_key.as_deref(), Some(expected_path_key));
                assert_eq!(result.adapter_key.as_deref(), Some(expected_adapter_key));
                assert!(result
                    .evidence
                    .join("\n")
                    .contains(expected_evidence_marker));
                let source_config = result.source_config.unwrap();
                assert_eq!(
                    source_config[expected_source_config_key],
                    expected_source_config_value
                );
            }
        });
    }

    #[test]
    fn lever_global_urls_detect_global_access_path_and_api() {
        tauri::async_runtime::block_on(async {
            let profile = builtin_profile("lever");
            let cases = [
                (
                    "https://lever-fixture.test/jobs",
                    r#"<a href="https://jobs.lever.co/acme/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">Senior Rust Engineer</a>"#,
                    "acme",
                ),
                (
                    "https://lever-fixture.test/api-link",
                    r#"<a href="https://api.lever.co/v0/postings/acme?mode=json">Lever postings API</a>"#,
                    "acme",
                ),
            ];

            for (input_url, html, expected_board_slug) in cases {
                let client = FixtureHttpClient::new([(input_url, html)]);

                let result = detect_with_source_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &[profile.clone()],
                )
                .await
                .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Detected);
                assert_eq!(result.profile_key.as_deref(), Some("lever"));
                assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
                let source_config = result.source_config.unwrap();
                assert_eq!(source_config["boardSlug"], expected_board_slug);
                assert_eq!(
                    access_path_inventory_fetch_url(&profile, "endpoint_inventory"),
                    "https://api.lever.co/v0/postings/{{sourceConfig:boardSlug}}?mode=json"
                );
            }
        });
    }

    #[test]
    fn lever_eu_urls_detect_eu_access_path_and_api() {
        tauri::async_runtime::block_on(async {
            let profile = builtin_profile("lever");
            let cases = [
                (
                    "https://lever-fixture.test/eu-jobs",
                    r#"<a href="https://jobs.eu.lever.co/acme-eu/9d39183d-5d2f-4c2d-aabb-1aa2bb3cc4dd">Senior Rust Engineer</a>"#,
                    "acme-eu",
                ),
                (
                    "https://lever-fixture.test/eu-api-link",
                    r#"<a href="https://api.eu.lever.co/v0/postings/acme-eu?mode=json">Lever EU postings API</a>"#,
                    "acme-eu",
                ),
            ];

            for (input_url, html, expected_board_slug) in cases {
                let client = FixtureHttpClient::new([(input_url, html)]);

                let result = detect_with_source_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &[profile.clone()],
                )
                .await
                .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Detected);
                assert_eq!(result.profile_key.as_deref(), Some("lever"));
                assert_eq!(result.path_key.as_deref(), Some("eu_endpoint_inventory"));
                let source_config = result.source_config.unwrap();
                assert_eq!(source_config["boardSlug"], expected_board_slug);
                assert_eq!(
                    access_path_inventory_fetch_url(&profile, "eu_endpoint_inventory"),
                    "https://api.eu.lever.co/v0/postings/{{sourceConfig:boardSlug}}?mode=json"
                );
            }
        });
    }

    #[test]
    fn ashby_identity_uses_board_slug_candidates_when_profile_captures_it() {
        tauri::async_runtime::block_on(async {
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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse(input_url).unwrap(),
                &[builtin_profile("ashby")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.key.as_deref(), Some("focused"));
            assert_eq!(result.name.as_deref(), Some("Focused"));
            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], "focused");
            assert!(source_config.get("startUrl").is_none());
            assert!(source_config.get("companyWebsite").is_none());
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
                builtin_profile("greenhouse"),
                builtin_profile("ashby"),
                builtin_profile("lever"),
            ];

            for input_url in [
                "https://example.com/greenhouse-mention",
                "https://example.com/ashby-mention",
                "https://example.com/lever-mention",
            ] {
                let result = detect_with_source_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &profiles,
                )
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
                builtin_profile("greenhouse"),
                builtin_profile("ashby"),
                builtin_profile("lever"),
            ];

            for input_url in ["https://openai.com/careers", "https://helsing.ai/careers"] {
                let result = detect_with_source_profiles(
                    &client,
                    &Url::parse(input_url).unwrap(),
                    &profiles,
                )
                .await
                .unwrap();

                assert_eq!(result.status, SourceDetectionStatus::Unsupported);
                assert!(result.matches.is_empty());
            }
        });
    }

    #[test]
    fn detects_personio_hosted_page_with_xml_feed_config() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://demo.jobs.personio.de/",
                    r#"
                    <html>
                      <head><title>Jobs bei Demo AG</title></head>
                      <body><h1>Jobs bei Demo AG</h1></body>
                    </html>
                    "#,
                ),
                (
                    "https://demo.jobs.personio.de/xml?language=en",
                    r#"<?xml version="1.0" encoding="UTF-8"?><workzag-jobs></workzag-jobs>"#,
                ),
            ]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://demo.jobs.personio.de/").unwrap(),
                &[builtin_profile("personio")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("personio"));
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
            assert_eq!(
                result.adapter_key.as_deref(),
                Some("declarative_endpoint_inventory")
            );
            assert!(result.evidence.join("\n").contains("workzag-jobs"));

            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], "demo");
            assert_eq!(source_config["personioHost"], "demo.jobs.personio.de");
            assert_eq!(source_config["language"], "en");
            assert_eq!(source_config["startUrl"], "https://demo.jobs.personio.de/");
        });
    }

    #[test]
    fn detects_personio_hosted_page_when_initial_fetch_fails_but_xml_feed_works() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://demo.jobs.personio.de/xml?language=en",
                r#"<?xml version="1.0" encoding="UTF-8"?><workzag-jobs></workzag-jobs>"#,
            )]);

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://demo.jobs.personio.de/").unwrap(),
                &[builtin_profile("personio")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("personio"));
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));

            let source_config = result.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], "demo");
            assert_eq!(source_config["personioHost"], "demo.jobs.personio.de");
            assert_eq!(source_config["language"], "en");

            let warnings = result.warnings.join("\n");
            assert!(warnings.contains("https://demo.jobs.personio.de/"));
            assert!(warnings.contains("not found"));
        });
    }

    #[test]
    fn detects_personio_linked_board_without_matching_generic_mentions() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://example.com/careers",
                    r#"<html><body><a href="https://demo.jobs.personio.com/">Open roles</a></body></html>"#,
                ),
                (
                    "https://example.com/personio-mention",
                    r#"<html><body><p>We use Personio in HR.</p></body></html>"#,
                ),
            ]);
            let profile = builtin_profile("personio");

            let detected = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/careers").unwrap(),
                &[profile.clone()],
            )
            .await
            .unwrap();

            assert_eq!(detected.status, SourceDetectionStatus::Detected);
            let source_config = detected.source_config.unwrap();
            assert_eq!(source_config["boardSlug"], "demo");
            assert_eq!(source_config["personioHost"], "demo.jobs.personio.com");

            let unsupported = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/personio-mention").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(unsupported.status, SourceDetectionStatus::Unsupported);
            assert!(unsupported.matches.is_empty());
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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://careers.example.com/search/").unwrap(),
                &[builtin_profile("successfactors")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("successfactors"));
            assert_eq!(result.path_key.as_deref(), Some("sitemap_inventory"));
            assert_eq!(
                result.adapter_key.as_deref(),
                Some("declarative_sitemap_inventory")
            );
            let evidence = result.evidence.join("\n");
            assert!(evidence.contains("HTML erfüllt Regex"));
            assert!(evidence.contains("SuccessFactors"));
            assert!(evidence.contains("https://careers.example.com/sitemap.xml"));
            assert!(evidence.contains("<urlset"));

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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://successfactors.example.com/jobs").unwrap(),
                &[builtin_profile("successfactors")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
            assert!(result.evidence.is_empty());
        });
    }

    #[test]
    fn detects_muz_with_source_profile_and_access_path_config() {
        tauri::async_runtime::block_on(async {
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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://jobs.commerzbank.com/index.php?ac=search_result").unwrap(),
                &[builtin_profile("muz_global_jobboard")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(result.profile_key.as_deref(), Some("muz_global_jobboard"));
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
            assert_eq!(
                result.adapter_key.as_deref(),
                Some("declarative_endpoint_inventory")
            );
            let evidence = result.evidence.join("\n");
            assert!(evidence.contains("HTML"));
            assert!(evidence.contains("Script"));
            assert!(evidence.contains("JSON-Pfad"));

            let source_config = result.source_config.unwrap();
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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://jobs.commerzbank.com/generic-careers").unwrap(),
                &[builtin_profile("muz_global_jobboard")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
            assert!(result.matches.is_empty());
            assert!(result.evidence.is_empty());
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

            let result = detect_with_source_profiles(
                &client,
                &Url::parse(
                    "https://www.ruv.de/karriere/jobsuche?reqPlace=&reqUmkreis=&jobSearchText=",
                )
                .unwrap(),
                &[builtin_profile("magnolia_esmp_job_search")],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(
                result.profile_key.as_deref(),
                Some("magnolia_esmp_job_search")
            );
            assert_eq!(result.path_key.as_deref(), Some("endpoint_inventory"));
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
                r#"<html><body>No known source profile</body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "example",
                "name": "Example",
                "kind": "recruiting_system",
                "detect": {
                    "phases": ["http"],
                    "required": [{ "htmlContains": "jobboard-widget" }]
                },
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory",
                    "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
                }]
            }));

            let result = detect_with_source_profiles(
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

    #[test]
    fn ambiguous_detection_reports_source_profile_and_path_terms() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><main id="shared-board-root"></main></body></html>"#,
            )]);
            let first = matching_profile("first_profile");
            let second = matching_profile("second_profile");

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[first, second],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Ambiguous);
            assert_eq!(result.matches.len(), 2);
            assert_eq!(result.matches[0].profile_key, "first_profile");
            assert_eq!(result.matches[0].path_key, "endpoint_inventory");
            let serialized = serde_json::to_string(&result).unwrap();
            assert!(serialized.contains("profileKey"));
            assert!(serialized.contains("pathKey"));
            assert!(!serialized.contains("systemProfile"));
        });
    }

    #[test]
    fn source_profiles_without_detection_blocks_are_not_global_detection_candidates() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([(
                "https://example.com/jobs",
                r#"<html><body><main>Any page</main></body></html>"#,
            )]);
            let profile = registry_profile(json!({
                "schemaVersion": 1,
                "key": "manual_only",
                "name": "Manual Only",
                "kind": "generic",
                "accessPaths": [{
                    "key": "endpoint_inventory",
                    "adapterKey": "declarative_endpoint_inventory"
                }]
            }));

            let result = detect_with_source_profiles(
                &client,
                &Url::parse("https://example.com/jobs").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Unsupported);
        });
    }

    fn any_of_profile() -> RegistrySourceProfile {
        registry_profile(json!({
            "schemaVersion": 1,
            "key": "example_board",
            "name": "Example Board",
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "example-board-root" }],
                "anyOf": [
                    [
                        {
                            "htmlRegex": "firstTenant\\s*=\\s*\"([^\"]+)\"",
                            "captureAs": "tenant"
                        },
                        { "htmlContains": "first-alt-ready" }
                    ],
                    [{
                        "htmlRegex": "secondTenant\\s*=\\s*\"([^\"]+)\"",
                        "captureAs": "tenant"
                    }]
                ]
            },
            "identity": {
                "keyCandidates": ["{{capture:tenant|technicalKey}}"],
                "nameCandidates": ["{{capture:tenant|titleCase}}"]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": {
                    "requiredCaptures": ["tenant"],
                    "sourceConfig": { "tenant": "{{capture:tenant}}" }
                }
            }]
        }))
    }

    fn matching_profile(key: &str) -> RegistrySourceProfile {
        registry_profile(json!({
            "schemaVersion": 1,
            "key": key,
            "name": title_from_key(key),
            "kind": "recruiting_system",
            "detect": {
                "phases": ["http"],
                "required": [{ "htmlContains": "shared-board-root" }]
            },
            "accessPaths": [{
                "key": "endpoint_inventory",
                "adapterKey": "declarative_endpoint_inventory",
                "availability": { "sourceConfig": { "startUrl": "{{inputUrl}}" } }
            }]
        }))
    }

    fn builtin_profile(key: &str) -> RegistrySourceProfile {
        match key {
            "ashby" => {
                registry_profile_from_str(include_str!("../../source-profiles/builtin/ashby.json"))
            }
            "greenhouse" => registry_profile_from_str(include_str!(
                "../../source-profiles/builtin/greenhouse.json"
            )),
            "lever" => {
                registry_profile_from_str(include_str!("../../source-profiles/builtin/lever.json"))
            }
            "magnolia_esmp_job_search" => registry_profile_from_str(include_str!(
                "../../source-profiles/builtin/magnolia_esmp_job_search.json"
            )),
            "muz_global_jobboard" => registry_profile_from_str(include_str!(
                "../../source-profiles/builtin/muz_global_jobboard.json"
            )),
            "personio" => registry_profile_from_str(include_str!(
                "../../source-profiles/builtin/personio.json"
            )),
            "successfactors" => registry_profile_from_str(include_str!(
                "../../source-profiles/builtin/successfactors.json"
            )),
            other => panic!("unknown built-in source profile fixture {other}"),
        }
    }

    fn access_path_inventory_fetch_url<'a>(
        profile: &'a RegistrySourceProfile,
        path_key: &str,
    ) -> &'a str {
        profile
            .document
            .access_paths
            .iter()
            .find(|access_path| access_path.key == path_key)
            .and_then(|access_path| access_path.inventory.as_ref())
            .and_then(|inventory| inventory.pointer("/fetch/url"))
            .and_then(Value::as_str)
            .unwrap()
    }

    fn registry_profile(value: Value) -> RegistrySourceProfile {
        let document: SourceProfileDocument = serde_json::from_value(value).unwrap();
        wrap_registry_profile(document)
    }

    fn registry_profile_from_str(contents: &str) -> RegistrySourceProfile {
        let document: SourceProfileDocument = serde_json::from_str(contents).unwrap();
        wrap_registry_profile(document)
    }

    fn wrap_registry_profile(document: SourceProfileDocument) -> RegistrySourceProfile {
        RegistrySourceProfile {
            origin: SourceRegistryDocumentOrigin::BuiltIn,
            path: format!("source-profiles/builtin/{}.json", document.key),
            document,
        }
    }

    fn title_from_key(key: &str) -> String {
        key.split('_')
            .map(|part| {
                let mut characters = part.chars();
                match characters.next() {
                    Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
