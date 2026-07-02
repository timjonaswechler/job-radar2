use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::declarative::template::{render_template, TemplateContext, TemplateError};
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::SupportLevel;
use crate::profile_dsl::execution_plan::capabilities::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
};
use crate::profile_dsl::runtime::{
    ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
};
use crate::source_profile::documents::{
    DetectionBrowserInteraction, DetectionBrowserProbe, DetectionEvidenceKind, DetectionHttpCheck,
    ProfileDetectionDocument, SourceProfileDocument,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProposalDetectionStatus {
    Matched,
    Ambiguous,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProposalDetectionResult {
    pub status: SourceProposalDetectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<SourceProposal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposals: Vec<SourceProposal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported_profiles: Vec<UnsupportedSourceProfile>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProposal {
    pub profile_key: String,
    pub profile_name: String,
    pub recommended_access_path_key: String,
    pub recommended_access_path_name: String,
    pub source_config: Value,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub captures: BTreeMap<String, String>,
    pub evidence: Vec<SourceProposalEvidence>,
    pub support_level: SupportLevel,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProposalEvidence {
    pub kind: DetectionEvidenceKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedSourceProfile {
    pub profile_key: String,
    pub profile_name: String,
    pub support_level: SupportLevel,
    pub captures: BTreeMap<String, String>,
    pub evidence: Vec<SourceProposalEvidence>,
}

#[derive(Clone, Debug)]
struct Candidate {
    proposal: Option<SourceProposal>,
    unsupported: Option<UnsupportedSourceProfile>,
    failed: bool,
    diagnostics: Diagnostics,
}

pub async fn detect_source_proposal_with_http_client<C: DetectionHttpClient + Sync>(
    input_url: &str,
    profiles: &[SourceProfileDocument],
    http_client: &C,
) -> SourceProposalDetectionResult {
    detect_source_proposal_internal(input_url, profiles, http_client, None).await
}

pub async fn detect_source_proposal_with_clients<C, B>(
    input_url: &str,
    profiles: &[SourceProfileDocument],
    http_client: &C,
    browser_client: &B,
) -> SourceProposalDetectionResult
where
    C: DetectionHttpClient + Sync,
    B: ProfileBrowserClient + Sync,
{
    detect_source_proposal_internal(input_url, profiles, http_client, Some(browser_client)).await
}

async fn detect_source_proposal_internal<C: DetectionHttpClient + Sync>(
    input_url: &str,
    profiles: &[SourceProfileDocument],
    http_client: &C,
    browser_client: Option<&(dyn ProfileBrowserClient + Sync)>,
) -> SourceProposalDetectionResult {
    let mut diagnostics = Vec::new();
    let input_url = input_url.trim();
    if input_url.is_empty() {
        diagnostics.push(detection_error(
            "invalid_input_url",
            "Profile Detection requires a non-empty input URL",
            "",
            None,
            serde_json::json!({ "inputUrl": input_url }),
        ));
        return failed_result(diagnostics);
    }

    let mut proposals = Vec::new();
    let mut unsupported_profiles = Vec::new();
    let mut failed = false;

    for (profile_index, profile) in profiles.iter().enumerate() {
        let candidate = evaluate_profile(
            input_url,
            profile_index,
            profile,
            http_client,
            browser_client,
        )
        .await;
        diagnostics.extend(candidate.diagnostics);
        if let Some(proposal) = candidate.proposal {
            proposals.push(proposal);
        }
        if let Some(unsupported) = candidate.unsupported {
            unsupported_profiles.push(unsupported);
        }
        failed |= candidate.failed;
    }

    if !proposals.is_empty() {
        if proposals.len() == 1 {
            return SourceProposalDetectionResult {
                status: SourceProposalDetectionStatus::Matched,
                proposal: proposals.first().cloned(),
                proposals,
                unsupported_profiles,
                diagnostics,
            };
        }
        return SourceProposalDetectionResult {
            status: SourceProposalDetectionStatus::Ambiguous,
            proposal: None,
            proposals,
            unsupported_profiles,
            diagnostics,
        };
    }

    if failed {
        return failed_result_with_unsupported(diagnostics, unsupported_profiles);
    }

    SourceProposalDetectionResult {
        status: SourceProposalDetectionStatus::Unsupported,
        proposal: None,
        proposals,
        unsupported_profiles,
        diagnostics,
    }
}

pub async fn detect_source_proposal(
    input_url: &str,
    profiles: &[SourceProfileDocument],
) -> SourceProposalDetectionResult {
    detect_source_proposal_with_http_client(input_url, profiles, &NoopDetectionHttpClient).await
}

async fn evaluate_profile<C: DetectionHttpClient + Sync>(
    input_url: &str,
    profile_index: usize,
    profile: &SourceProfileDocument,
    http_client: &C,
    browser_client: Option<&(dyn ProfileBrowserClient + Sync)>,
) -> Candidate {
    let Some(detect) = &profile.detect else {
        return Candidate {
            proposal: None,
            unsupported: None,
            failed: false,
            diagnostics: Vec::new(),
        };
    };

    let base_path = format!("/profiles/{profile_index}/detect");
    let Some((mut captures, mut evidence)) =
        match_input_url_patterns(input_url, detect, &base_path)
    else {
        return Candidate {
            proposal: None,
            unsupported: None,
            failed: false,
            diagnostics: Vec::new(),
        };
    };

    let mut diagnostics = Vec::new();
    if !evaluate_http_checks(
        input_url,
        detect.http_checks.as_deref().unwrap_or_default(),
        &mut captures,
        &mut evidence,
        &mut diagnostics,
        http_client,
        &base_path,
    )
    .await
    {
        let failed = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
        return Candidate {
            proposal: None,
            unsupported: None,
            failed,
            diagnostics,
        };
    }

    if let Some(browser_probes) = detect.browser_probes.as_deref() {
        if !browser_probes.is_empty() {
            let Some(browser_client) = browser_client else {
                diagnostics.extend(browser_probe_unavailable_diagnostics(
                    browser_probes,
                    &base_path,
                    &profile.key,
                ));
                return Candidate {
                    proposal: None,
                    unsupported: None,
                    failed: true,
                    diagnostics,
                };
            };
            let source_config_for_probes =
                match build_source_config(input_url, profile, detect, &captures) {
                    Ok(source_config) => source_config,
                    Err(error) => {
                        return Candidate {
                            proposal: None,
                            unsupported: None,
                            failed: true,
                            diagnostics: vec![template_diagnostic(
                                error,
                                &format!("{base_path}/sourceConfig"),
                                None,
                            )],
                        };
                    }
                };
            if !evaluate_browser_probes(
                input_url,
                browser_probes,
                &mut captures,
                source_config_for_probes.as_object(),
                &mut evidence,
                &mut diagnostics,
                browser_client,
                &base_path,
            )
            .await
            {
                let failed = diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
                return Candidate {
                    proposal: None,
                    unsupported: None,
                    failed,
                    diagnostics,
                };
            }
        }
    }

    if profile.support.level == SupportLevel::Unsupported {
        return Candidate {
            proposal: None,
            unsupported: Some(UnsupportedSourceProfile {
                profile_key: profile.key.clone(),
                profile_name: profile.name.clone(),
                support_level: profile.support.level,
                captures,
                evidence,
            }),
            failed: false,
            diagnostics,
        };
    }

    match build_source_proposal(input_url, profile, detect, captures, evidence, &base_path) {
        Ok(proposal) => Candidate {
            proposal: Some(proposal),
            unsupported: None,
            failed: false,
            diagnostics,
        },
        Err(diagnostic) => Candidate {
            proposal: None,
            unsupported: None,
            failed: true,
            diagnostics: vec![diagnostic],
        },
    }
}

fn match_input_url_patterns(
    input_url: &str,
    detect: &ProfileDetectionDocument,
    base_path: &str,
) -> Option<(BTreeMap<String, String>, Vec<SourceProposalEvidence>)> {
    let patterns = detect.input_url_patterns.as_deref().unwrap_or_default();
    if patterns.is_empty() {
        return Some((BTreeMap::new(), detection_document_evidence(detect)));
    }

    for (index, pattern) in patterns.iter().enumerate() {
        let regex = match Regex::new(&pattern.pattern) {
            Ok(regex) => regex,
            Err(_) => continue,
        };
        let Some(matches) = regex.captures(input_url) else {
            continue;
        };

        let mut captures = BTreeMap::new();
        for name in regex.capture_names().flatten() {
            if let Some(value) = matches.name(name).map(|capture| capture.as_str()) {
                if !value.trim().is_empty() {
                    captures.insert(name.to_string(), value.to_string());
                }
            }
        }
        if let Some(capture_names) = &pattern.captures {
            for name in capture_names {
                if let Some(value) = matches.name(name).map(|capture| capture.as_str()) {
                    if !value.trim().is_empty() {
                        captures.insert(name.clone(), value.to_string());
                    }
                }
            }
        }

        let mut evidence = detection_document_evidence(detect);
        evidence.push(SourceProposalEvidence {
            kind: DetectionEvidenceKind::Url,
            message: format!("Input URL matched detection pattern `{}`", pattern.pattern),
            path: Some(format!("{base_path}/inputUrlPatterns/{index}/pattern")),
            probe_key: None,
        });
        return Some((captures, evidence));
    }

    None
}

async fn evaluate_http_checks<C: DetectionHttpClient + Sync>(
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

async fn evaluate_browser_probes(
    input_url: &str,
    probes: &[DetectionBrowserProbe],
    captures: &mut BTreeMap<String, String>,
    source_config: Option<&Map<String, Value>>,
    evidence: &mut Vec<SourceProposalEvidence>,
    diagnostics: &mut Diagnostics,
    browser_client: &(dyn ProfileBrowserClient + Sync),
    base_path: &str,
) -> bool {
    for (index, probe) in probes.iter().enumerate() {
        let probe_path = format!("{base_path}/browserProbes/{index}");
        let rendered_url = match render_detection_template_with_source_config(
            &probe.url,
            input_url,
            captures,
            source_config,
        ) {
            Ok(url) => url,
            Err(error) => {
                diagnostics.push(template_diagnostic(
                    error,
                    &format!("{probe_path}/url"),
                    Some(&probe.key),
                ));
                return false;
            }
        };

        let request = match browser_probe_request(probe, rendered_url.clone(), &probe_path) {
            Ok(request) => request,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return false;
            }
        };

        let response = match browser_client.render(request).await {
            Ok(response) => response,
            Err(error) => {
                diagnostics.push(browser_probe_error_diagnostic(
                    error,
                    &rendered_url,
                    &probe_path,
                    &probe.key,
                ));
                return false;
            }
        };

        if !evaluate_rendered_html_checks(
            probe,
            &response,
            captures,
            evidence,
            diagnostics,
            &probe_path,
        ) {
            return false;
        }
    }

    true
}

fn browser_probe_request(
    probe: &DetectionBrowserProbe,
    rendered_url: String,
    probe_path: &str,
) -> Result<ProfileBrowserFetchRequest, Diagnostic> {
    let timeout_ms = probe.timeout_ms.unwrap_or(10_000);
    if timeout_ms == 0 {
        return Err(detection_error(
            "browser_probe_timeout_required",
            format!(
                "Browser probe `{}` must declare a positive timeoutMs or use the bounded default",
                probe.key
            ),
            format!("{probe_path}/timeoutMs"),
            Some(&probe.key),
            serde_json::json!({ "probeKey": probe.key }),
        ));
    }

    let mut waits = Vec::new();
    for (index, wait) in probe
        .waits
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let path = format!("{probe_path}/waits/{index}");
        let timeout_ms = match wait {
            crate::profile_dsl::documents::BrowserWait::Selector { timeout_ms, .. }
            | crate::profile_dsl::documents::BrowserWait::NetworkIdle { timeout_ms, .. } => {
                timeout_ms.filter(|value| *value > 0).ok_or_else(|| {
                    detection_error(
                        "browser_wait_timeout_required",
                        format!(
                            "Browser probe `{}` wait must declare a positive timeoutMs",
                            probe.key
                        ),
                        format!("{path}/timeoutMs"),
                        Some(&probe.key),
                        serde_json::json!({ "probeKey": probe.key }),
                    )
                })?
            }
        };
        waits.push(match wait {
            crate::profile_dsl::documents::BrowserWait::Selector { selector, .. } => {
                ExecutionPlanBrowserWait::Selector {
                    selector: selector.clone(),
                    timeout_ms,
                }
            }
            crate::profile_dsl::documents::BrowserWait::NetworkIdle { selector, .. } => {
                ExecutionPlanBrowserWait::NetworkIdle {
                    selector: selector.clone(),
                    timeout_ms,
                }
            }
        });
    }

    let mut interactions = Vec::new();
    for (index, interaction) in probe
        .interactions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let path = format!("{probe_path}/interactions/{index}");
        interactions.push(match interaction {
            DetectionBrowserInteraction::ClickIfVisible {
                selector,
                max_count,
                wait_after_ms,
            } => ExecutionPlanBrowserInteraction::ClickIfVisible {
                selector: selector.clone(),
                max_count: max_count.filter(|value| *value > 0).ok_or_else(|| {
                    detection_error(
                        "browser_interaction_max_count_required",
                        format!(
                            "Browser probe `{}` interaction must declare a positive maxCount",
                            probe.key
                        ),
                        format!("{path}/maxCount"),
                        Some(&probe.key),
                        serde_json::json!({ "probeKey": probe.key }),
                    )
                })?,
                wait_after_ms: *wait_after_ms,
            },
            DetectionBrowserInteraction::ClickUntilGone {
                selector,
                max_count,
                wait_after_ms,
            } => ExecutionPlanBrowserInteraction::ClickUntilGone {
                selector: selector.clone(),
                max_count: max_count.filter(|value| *value > 0).ok_or_else(|| {
                    detection_error(
                        "browser_interaction_max_count_required",
                        format!(
                            "Browser probe `{}` interaction must declare a positive maxCount",
                            probe.key
                        ),
                        format!("{path}/maxCount"),
                        Some(&probe.key),
                        serde_json::json!({ "probeKey": probe.key }),
                    )
                })?,
                wait_after_ms: *wait_after_ms,
            },
        });
    }

    Ok(ProfileBrowserFetchRequest {
        url: rendered_url,
        timeout_ms,
        waits,
        interactions,
    })
}

fn evaluate_rendered_html_checks(
    probe: &DetectionBrowserProbe,
    response: &ProfileBrowserFetchResponse,
    captures: &mut BTreeMap<String, String>,
    evidence: &mut Vec<SourceProposalEvidence>,
    diagnostics: &mut Diagnostics,
    probe_path: &str,
) -> bool {
    if let Some(needle) = &probe.html_contains {
        if !response.body.contains(needle) {
            diagnostics.push(detection_warning(
                "browser_probe_html_contains_mismatch",
                format!(
                    "Browser probe `{}` rendered HTML did not contain the required text",
                    probe.key
                ),
                format!("{probe_path}/htmlContains"),
                Some(&probe.key),
                serde_json::json!({ "probeKey": probe.key }),
            ));
            return false;
        }
    }

    if let Some(pattern) = &probe.html_regex {
        let regex = match Regex::new(pattern) {
            Ok(regex) => regex,
            Err(error) => {
                diagnostics.push(detection_error(
                    "invalid_browser_probe_html_regex",
                    format!(
                        "Browser probe `{}` has an invalid regex: {error}",
                        probe.key
                    ),
                    format!("{probe_path}/htmlRegex"),
                    Some(&probe.key),
                    serde_json::json!({ "probeKey": probe.key }),
                ));
                return false;
            }
        };
        let Some(matches) = regex.captures(&response.body) else {
            diagnostics.push(detection_warning(
                "browser_probe_html_regex_mismatch",
                format!(
                    "Browser probe `{}` rendered HTML did not match the required regex",
                    probe.key
                ),
                format!("{probe_path}/htmlRegex"),
                Some(&probe.key),
                serde_json::json!({ "probeKey": probe.key }),
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
        kind: DetectionEvidenceKind::Browser,
        message: probe
            .evidence
            .clone()
            .unwrap_or_else(|| format!("Browser probe `{}` matched rendered HTML", probe.key)),
        path: Some(probe_path.to_string()),
        probe_key: Some(probe.key.clone()),
    });

    true
}

fn browser_probe_error_diagnostic(
    error: ProfileBrowserFetchError,
    rendered_url: &str,
    probe_path: &str,
    probe_key: &str,
) -> Diagnostic {
    let (code, path) = match error.kind {
        ProfileBrowserFetchErrorKind::RuntimeUnavailable => {
            ("browser_runtime_unavailable", probe_path.to_string())
        }
        ProfileBrowserFetchErrorKind::NavigationFailed => {
            ("browser_navigation_failed", format!("{probe_path}/url"))
        }
        ProfileBrowserFetchErrorKind::WaitTimeout { wait_index } => (
            "browser_wait_timeout",
            wait_index
                .map(|index| format!("{probe_path}/waits/{index}"))
                .unwrap_or_else(|| format!("{probe_path}/waits")),
        ),
        ProfileBrowserFetchErrorKind::InteractionFailed { interaction_index } => (
            "browser_interaction_failed",
            interaction_index
                .map(|index| format!("{probe_path}/interactions/{index}"))
                .unwrap_or_else(|| format!("{probe_path}/interactions")),
        ),
        ProfileBrowserFetchErrorKind::RenderTimeout => {
            ("browser_render_timeout", format!("{probe_path}/timeoutMs"))
        }
        ProfileBrowserFetchErrorKind::ContentReadFailed => {
            ("browser_content_read_failed", probe_path.to_string())
        }
    };

    detection_error(
        code,
        format!(
            "Browser probe `{probe_key}` failed for {rendered_url}: {}",
            error.message
        ),
        path,
        Some(probe_key),
        serde_json::json!({
            "probeKey": probe_key,
            "url": rendered_url,
            "error": error.message,
        }),
    )
}

fn browser_probe_unavailable_diagnostics(
    browser_probes: &[DetectionBrowserProbe],
    base_path: &str,
    profile_key: &str,
) -> Diagnostics {
    browser_probes
        .iter()
        .enumerate()
        .map(|(index, probe)| {
            detection_error(
                "browser_probe_executor_unavailable",
                format!(
                    "Source Profile `{profile_key}` requires browser probe `{}` but no browser-probe executor is available",
                    probe.key
                ),
                format!("{base_path}/browserProbes/{index}"),
                Some(&probe.key),
                serde_json::json!({
                    "sourceProfileKey": profile_key,
                    "probeKey": probe.key,
                }),
            )
        })
        .collect()
}

fn build_source_proposal(
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
) -> Option<&'a crate::profile_dsl::documents::ReusableAccessPathDocument> {
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
    access_path: &crate::profile_dsl::documents::ReusableAccessPathDocument,
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

fn build_source_config(
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
        .map(|value| vec![crate::declarative::template::to_technical_key(value)])
        .unwrap_or_else(|| vec![profile_key.to_string()])
}

fn default_name_candidates(captures: &BTreeMap<String, String>, profile_name: &str) -> Vec<String> {
    captures
        .values()
        .next()
        .map(|value| vec![crate::declarative::template::title_case(value)])
        .unwrap_or_else(|| vec![profile_name.to_string()])
}

fn detection_document_evidence(detect: &ProfileDetectionDocument) -> Vec<SourceProposalEvidence> {
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

struct DetectionTemplateContext<'a> {
    input_url: &'a str,
    captures: &'a BTreeMap<String, String>,
    source_config: Option<&'a Map<String, Value>>,
}

impl TemplateContext for DetectionTemplateContext<'_> {
    fn resolve_variable(&self, variable: &str) -> Result<Option<String>, TemplateError> {
        if variable == "inputUrl" {
            return Ok(Some(self.input_url.to_string()));
        }
        if let Some(capture_key) = variable.strip_prefix("capture:") {
            return Ok(self.captures.get(capture_key).cloned());
        }
        if let Some(source_config_key) = variable
            .strip_prefix("sourceConfig:")
            .or_else(|| variable.strip_prefix("sourceConfig."))
        {
            return Ok(self
                .source_config
                .and_then(|source_config| source_config.get(source_config_key))
                .and_then(json_scalar_as_string));
        }
        Ok(None)
    }
}

fn render_detection_template(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    render_detection_template_with_source_config(template, input_url, captures, None)
}

fn render_detection_template_with_source_config(
    template: &str,
    input_url: &str,
    captures: &BTreeMap<String, String>,
    source_config: Option<&Map<String, Value>>,
) -> Result<String, TemplateError> {
    render_template(
        template,
        &DetectionTemplateContext {
            input_url,
            captures,
            source_config,
        },
    )
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn template_diagnostic(error: TemplateError, path: &str, probe_key: Option<&str>) -> Diagnostic {
    let code = match error {
        TemplateError::MissingVariable(_) => "missing_detection_template_variable",
        TemplateError::Invalid(_) => "invalid_detection_template",
    };
    detection_error(
        code,
        format!("Profile Detection template could not be rendered: {error}"),
        path,
        probe_key,
        serde_json::json!({ "error": error.to_string() }),
    )
}

fn detection_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    probe_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: probe_key.map(ToString::to_string),
        details: Some(details),
    }
}

fn detection_warning(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    probe_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Warning,
        path: path.into(),
        strategy_key: probe_key.map(ToString::to_string),
        details: Some(details),
    }
}

fn failed_result(diagnostics: Diagnostics) -> SourceProposalDetectionResult {
    failed_result_with_unsupported(diagnostics, Vec::new())
}

fn failed_result_with_unsupported(
    diagnostics: Diagnostics,
    unsupported_profiles: Vec<UnsupportedSourceProfile>,
) -> SourceProposalDetectionResult {
    SourceProposalDetectionResult {
        status: SourceProposalDetectionStatus::Failed,
        proposal: None,
        proposals: Vec::new(),
        unsupported_profiles,
        diagnostics,
    }
}
