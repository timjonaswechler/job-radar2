use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::source_model::{list_system_profiles, SourceStatus, SystemProfile};

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
    let required = profile
        .definition
        .pointer("/detect/required")
        .and_then(Value::as_array)
        .ok_or_else(|| "definition.detect.required must be an array".to_string())?;

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

    let company_name = derive_company_name(input_url);
    let source_config = build_source_config(&profile.definition, input_url, &captures)?;
    Ok(Some(SourceDetectionMatch {
        adapter_key: profile.adapter_key.clone(),
        system_profile_id: profile.id,
        system_profile_key: profile.key.clone(),
        system_profile_name: profile.name.clone(),
        key: format!("{}_careers", to_technical_key(&company_name)),
        name: format!("{company_name} Karriere"),
        source_config,
        evidence,
    }))
}

async fn evaluate_check<C: DetectionHttpClient + Sync>(
    client: &C,
    input_url: &Url,
    html: &str,
    check: &Value,
    captures: &mut HashMap<String, String>,
) -> Result<Option<String>, String> {
    if let Some(needle) = check.get("htmlContains").and_then(Value::as_str) {
        return Ok(contains_text(html, needle).then(|| format!("HTML enthält `{needle}`")));
    }

    if let Some(pattern) = check.get("htmlRegex").and_then(Value::as_str) {
        return evaluate_regex("HTML", html, pattern, check.get("captureAs"), captures);
    }

    if let Some(fetch_text) = check.get("fetchText").and_then(Value::as_object) {
        let url = required_string(fetch_text, "url")?;
        let fetched_url = input_url
            .join(url)
            .map_err(|error| format!("fetchText.url is invalid: {error}"))?;
        let text = client.get_text(fetched_url.clone()).await.ok();
        let Some(text) = text else {
            return Ok(None);
        };
        if let Some(needle) = fetch_text.get("contains").and_then(Value::as_str) {
            return Ok(contains_text(&text, needle)
                .then(|| format!("{} enthält `{needle}`", fetched_url.as_str())));
        }
        if let Some(pattern) = fetch_text.get("regex").and_then(Value::as_str) {
            return evaluate_regex(
                fetched_url.as_str(),
                &text,
                pattern,
                fetch_text.get("captureAs"),
                captures,
            );
        }
        return Ok(Some(format!(
            "{} wurde erfolgreich geladen",
            fetched_url.as_str()
        )));
    }

    if let Some(fetch_json) = check.get("fetchJson").and_then(Value::as_object) {
        let url = required_string(fetch_json, "url")?;
        let fetched_url = input_url
            .join(url)
            .map_err(|error| format!("fetchJson.url is invalid: {error}"))?;
        let text = client.get_text(fetched_url.clone()).await.ok();
        let Some(text) = text else {
            return Ok(None);
        };
        let json: Value = match serde_json::from_str(&text) {
            Ok(json) => json,
            Err(_) => return Ok(None),
        };
        if let Some(path) = fetch_json.get("pathExists").and_then(Value::as_str) {
            return Ok(json_path_exists(&json, path)
                .then(|| format!("{} enthält JSON-Pfad `{path}`", fetched_url.as_str())));
        }
        return Ok(Some(format!(
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
) -> Result<Option<String>, String> {
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

    for script_url in script_urls {
        let Ok(script_text) = client.get_text(script_url.clone()).await else {
            continue;
        };
        if let Some(needle) = fetch_script.get("contains").and_then(Value::as_str) {
            if contains_text(&script_text, needle) {
                return Ok(Some(format!(
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
                return Ok(Some(evidence));
            }
        }
        if fetch_script.get("contains").is_none() && fetch_script.get("regex").is_none() {
            return Ok(Some(format!(
                "Script {} wurde erfolgreich geladen",
                script_url.as_str()
            )));
        }
    }

    Ok(None)
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

fn build_source_config(
    definition: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, String> {
    let template = definition.get("sourceConfig").unwrap_or(&Value::Null);
    if let Some(object) = template.as_object() {
        let mut rendered = Map::new();
        for (key, value) in object {
            rendered.insert(
                key.clone(),
                render_template_value(value, input_url, captures)?,
            );
        }
        return Ok(Value::Object(rendered));
    }

    Ok(json_default_source_config(input_url))
}

fn render_template_value(
    value: &Value,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<Value, String> {
    match value {
        Value::String(template) => Ok(Value::String(render_template(
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
            .collect::<Result<Map<_, _>, String>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

fn render_template(
    template: &str,
    input_url: &Url,
    captures: &HashMap<String, String>,
) -> Result<String, String> {
    let mut rendered = template
        .replace("{{inputUrl}}", input_url.as_str())
        .replace("{{origin}}", &origin(input_url));

    let capture_regex = Regex::new(r"\{\{capture:([a-zA-Z0-9_]+)\}\}").unwrap();
    let mut missing_capture = None;
    rendered = capture_regex
        .replace_all(&rendered, |captures_match: &regex::Captures<'_>| {
            let capture_key = &captures_match[1];
            match captures.get(capture_key) {
                Some(value) => value.clone(),
                None => {
                    missing_capture = Some(capture_key.to_string());
                    String::new()
                }
            }
        })
        .to_string();

    if let Some(capture_key) = missing_capture {
        return Err(format!(
            "sourceConfig references missing capture `{capture_key}`"
        ));
    }

    Ok(rendered)
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
        .find(|label| {
            !matches!(
                *label,
                "www" | "jobs" | "job" | "careers" | "career" | "join" | "boards" | "job-boards"
            )
        })
        .unwrap_or("neue_quelle");
    title_case(label)
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
    fn detects_muz_global_jobboard_only_when_required_evidence_matches() {
        tauri::async_runtime::block_on(async {
            let client = FixtureHttpClient::new([
                (
                    "https://jobs.commerzbank.com/index.php?ac=search_result",
                    r#"<div class="jobboard-widget" data-widget="jobboardDatatable"></div>"#,
                ),
                (
                    "https://jobs.commerzbank.com/script/gjb_scripts.js",
                    r#"var gjbAddress = "https://api-jobs.commerzbank.com/";"#,
                ),
                (
                    "https://jobs.commerzbank.com/assets/js/jobboard.config.json",
                    r#"{"configWidgetContainer":{"search":{}}}"#,
                ),
            ]);
            let profile = SystemProfile {
                id: 42,
                key: "muz_global_jobboard".to_string(),
                name: "Milch & Zucker Global Jobboard".to_string(),
                description: None,
                adapter_key: "declarative_http_jobboard".to_string(),
                definition_schema_version: 1,
                definition: json!({
                    "detect": { "required": [
                        { "htmlContains": "jobboard-widget" },
                        { "fetchText": {
                            "url": "/script/gjb_scripts.js",
                            "regex": "gjbAddress\\s*=\\s*\"([^\"]+)\"",
                            "captureAs": "apiBaseUrl"
                        }},
                        { "fetchJson": {
                            "url": "/assets/js/jobboard.config.json",
                            "pathExists": "$.configWidgetContainer.search"
                        }}
                    ]},
                    "sourceConfig": {
                        "startUrl": "{{inputUrl}}",
                        "apiBaseUrl": "{{capture:apiBaseUrl}}",
                        "configUrl": "{{origin}}/assets/js/jobboard.config.json"
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
                &Url::parse("https://jobs.commerzbank.com/index.php?ac=search_result").unwrap(),
                &[profile],
            )
            .await
            .unwrap();

            assert_eq!(result.status, SourceDetectionStatus::Detected);
            assert_eq!(
                result.system_profile_key.as_deref(),
                Some("muz_global_jobboard")
            );
            assert_eq!(
                result.source_config.unwrap()["apiBaseUrl"],
                "https://api-jobs.commerzbank.com/"
            );
            assert_eq!(result.evidence.len(), 3);
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
}
