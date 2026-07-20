use std::collections::BTreeMap;

use regex::Regex;
use serde_json::{Map, Value};

use super::{
    detection_error, detection_warning, render_detection_template_with_source_config,
    template_diagnostic, SourceProposalEvidence,
};
use crate::profile_dsl::diagnostics::{Diagnostic, Diagnostics};
use crate::profile_dsl::documents::{
    DetectionBrowserInteraction, DetectionBrowserProbe, DetectionEvidenceKind,
};
use crate::profile_dsl::execution_plan::capabilities::{
    ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait,
};
use crate::profile_dsl::runtime::{
    ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
    ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
};

pub(super) async fn evaluate_browser_probes(
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
    let Some(timeout_ms) = probe.timeout_ms.filter(|timeout| *timeout > 0) else {
        return Err(detection_error(
            "browser_probe_timeout_required",
            format!(
                "Browser probe `{}` must declare a positive timeoutMs",
                probe.key
            ),
            format!("{probe_path}/timeoutMs"),
            Some(&probe.key),
            serde_json::json!({ "probeKey": probe.key }),
        ));
    };

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
        ProfileBrowserFetchErrorKind::Cancelled => {
            ("browser_probe_cancelled", probe_path.to_string())
        }
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

pub(super) fn browser_probe_unavailable_diagnostics(
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
