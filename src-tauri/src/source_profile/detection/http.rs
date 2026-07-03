use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use regex::Regex;

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::source_profile::documents::{DetectionEvidenceKind, DetectionHttpCheck};

use super::{
    detection_error, detection_warning, render_detection_template, template_diagnostic,
    SourceProposalEvidence,
};

pub type BoxedDetectionHttpFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DetectionHttpResponse, DetectionHttpError>> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionHttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionHttpError {
    pub message: String,
}

impl DetectionHttpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait DetectionHttpClient {
    fn get_text<'a>(&'a self, url: String, timeout_ms: u64) -> BoxedDetectionHttpFuture<'a>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopDetectionHttpClient;

impl DetectionHttpClient for NoopDetectionHttpClient {
    fn get_text<'a>(&'a self, url: String, _timeout_ms: u64) -> BoxedDetectionHttpFuture<'a> {
        Box::pin(async move {
            Err(DetectionHttpError::new(format!(
                "HTTP detection client is not configured for `{url}`"
            )))
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReqwestDetectionHttpClient {
    client: reqwest::Client,
}

impl ReqwestDetectionHttpClient {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .user_agent("JobRadarProfileDetection/0.1")
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

impl DetectionHttpClient for ReqwestDetectionHttpClient {
    fn get_text<'a>(&'a self, url: String, timeout_ms: u64) -> BoxedDetectionHttpFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .get(&url)
                .timeout(Duration::from_millis(timeout_ms.max(1)))
                .send()
                .await
                .map_err(|error| {
                    DetectionHttpError::new(format!("{url} could not be fetched: {error}"))
                })?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                DetectionHttpError::new(format!("{url} response body could not be read: {error}"))
            })?;
            Ok(DetectionHttpResponse { status, body })
        })
    }
}

pub(super) async fn evaluate_http_checks<C: DetectionHttpClient + Sync>(
    input_url: &str,
    checks: &[DetectionHttpCheck],
    captures: &mut BTreeMap<String, String>,
    evidence: &mut Vec<SourceProposalEvidence>,
    diagnostics: &mut Diagnostics,
    http_client: &C,
    base_path: &str,
) -> bool {
    for (index, check) in checks.iter().enumerate() {
        let check_path = format!("{base_path}/httpChecks/{index}");
        let rendered_url = match render_detection_template(&check.url, input_url, captures) {
            Ok(url) => url,
            Err(error) => {
                diagnostics.push(template_diagnostic(
                    error,
                    &format!("{check_path}/url"),
                    Some(&check.key),
                ));
                return false;
            }
        };
        let timeout_ms = check.timeout_ms.unwrap_or(10_000);
        let response = match http_client.get_text(rendered_url.clone(), timeout_ms).await {
            Ok(response) => response,
            Err(error) => {
                diagnostics.push(detection_error(
                    "http_check_failed",
                    format!(
                        "HTTP detection check `{}` could not fetch `{rendered_url}`: {}",
                        check.key, error.message
                    ),
                    format!("{check_path}/url"),
                    Some(&check.key),
                    serde_json::json!({
                        "checkKey": check.key,
                        "url": rendered_url,
                        "error": error.message,
                    }),
                ));
                return false;
            }
        };

        if let Some(expected_status) = check.expect_status {
            if response.status != expected_status {
                diagnostics.push(detection_warning(
                    "http_check_status_mismatch",
                    format!(
                        "HTTP detection check `{}` returned status {}, expected {}",
                        check.key, response.status, expected_status
                    ),
                    format!("{check_path}/expectStatus"),
                    Some(&check.key),
                    serde_json::json!({
                        "checkKey": check.key,
                        "expectedStatus": expected_status,
                        "actualStatus": response.status,
                    }),
                ));
                return false;
            }
        }

        if let Some(needle) = &check.contains {
            if !response.body.contains(needle) {
                diagnostics.push(detection_warning(
                    "http_check_contains_mismatch",
                    format!(
                        "HTTP detection check `{}` response did not contain the required text",
                        check.key
                    ),
                    format!("{check_path}/contains"),
                    Some(&check.key),
                    serde_json::json!({ "checkKey": check.key }),
                ));
                return false;
            }
        }

        if let Some(pattern) = &check.regex {
            let regex = match Regex::new(pattern) {
                Ok(regex) => regex,
                Err(error) => {
                    diagnostics.push(detection_error(
                        "invalid_http_check_regex",
                        format!(
                            "HTTP detection check `{}` has an invalid regex: {error}",
                            check.key
                        ),
                        format!("{check_path}/regex"),
                        Some(&check.key),
                        serde_json::json!({ "checkKey": check.key }),
                    ));
                    return false;
                }
            };
            let Some(matches) = regex.captures(&response.body) else {
                diagnostics.push(detection_warning(
                    "http_check_regex_mismatch",
                    format!(
                        "HTTP detection check `{}` response did not match the required regex",
                        check.key
                    ),
                    format!("{check_path}/regex"),
                    Some(&check.key),
                    serde_json::json!({ "checkKey": check.key }),
                ));
                return false;
            };
            for name in regex.capture_names().flatten() {
                if let Some(value) = matches.name(name).map(|capture| capture.as_str()) {
                    if !value.trim().is_empty() {
                        captures.insert(name.to_string(), value.to_string());
                    }
                }
            }
        }

        evidence.push(SourceProposalEvidence {
            kind: DetectionEvidenceKind::Http,
            message: check
                .evidence
                .clone()
                .unwrap_or_else(|| format!("HTTP detection check `{}` passed", check.key)),
            path: Some(check_path),
            probe_key: Some(check.key.clone()),
        });
    }

    true
}
