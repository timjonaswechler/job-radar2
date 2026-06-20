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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DetectionCheckStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DetectionCheckOutcome {
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

pub(super) async fn evaluate_check<C: DetectionHttpClient + Sync>(
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

pub(super) async fn evaluate_check_outcome<C: DetectionHttpClient + Sync>(
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

pub(super) async fn evaluate_fetch_script<C: DetectionHttpClient + Sync>(
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

pub(super) fn extract_script_srcs(html: &str) -> Vec<String> {
    let script_src_regex = Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#).unwrap();
    script_src_regex
        .captures_iter(html)
        .filter_map(|captures| captures.get(1).map(|src| src.as_str().to_string()))
        .collect()
}

pub(super) fn contains_text(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(&needle.to_lowercase())
}

pub(super) fn evaluate_regex(
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

pub(super) fn evaluate_regex_outcome(
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

pub(super) fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} is required"))
}
