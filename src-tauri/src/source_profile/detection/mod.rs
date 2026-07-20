use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::profile_dsl::diagnostics::{DiagnosticSeverity, Diagnostics};
use crate::profile_dsl::documents::{DetectionDocument, DetectionEvidenceKind, SupportLevel};
use crate::profile_dsl::runtime::ProfileBrowserClient;
use crate::profile_dsl::source_config::{compile_contract, SchemaLocation};
use crate::source_profile::documents::SourceProfileDocument;

mod browser;
mod diagnostics;
mod http;
mod proposal;
mod templates;

use browser::{browser_probe_unavailable_diagnostics, evaluate_browser_probes};
use diagnostics::{
    detection_error, detection_warning, failed_result, failed_result_with_unsupported,
};
use http::evaluate_http_checks;
#[allow(unused_imports)]
pub use http::{
    BoxedDetectionHttpFuture, DetectionHttpClient, DetectionHttpError, DetectionHttpResponse,
    NoopDetectionHttpClient, ReqwestDetectionHttpClient,
};
use proposal::{
    build_source_config, build_source_proposal, compiler_definition_diagnostic,
    detection_document_evidence, recommended_access_path, validate_detection_source_config_values,
    ValidationCompleteness,
};
use templates::{
    render_detection_template, render_detection_template_with_source_config, template_diagnostic,
};

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

    let definition_diagnostics = validate_detection_source_config_contracts(profiles);
    if !definition_diagnostics.is_empty() {
        return failed_result(definition_diagnostics);
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

fn validate_detection_source_config_contracts(profiles: &[SourceProfileDocument]) -> Diagnostics {
    let mut diagnostics = Vec::new();
    for (profile_index, profile) in profiles.iter().enumerate() {
        let profile_path = format!("/profiles/{profile_index}/sourceConfigSchema");
        if profile.access_paths.is_empty() {
            if let Err(violations) = compile_contract(&[SchemaLocation {
                schema: profile.source_config_schema.as_ref(),
                path: &profile_path,
                title_allowed: true,
            }]) {
                for violation in violations {
                    let diagnostic = compiler_definition_diagnostic(violation);
                    if !diagnostics.contains(&diagnostic) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
            continue;
        }
        for (path_index, access_path) in profile.access_paths.iter().enumerate() {
            let access_path_path =
                format!("/profiles/{profile_index}/accessPaths/{path_index}/sourceConfigSchema");
            if let Err(violations) = compile_contract(&[
                SchemaLocation {
                    schema: profile.source_config_schema.as_ref(),
                    path: &profile_path,
                    title_allowed: true,
                },
                SchemaLocation {
                    schema: access_path.source_config_schema.as_ref(),
                    path: &access_path_path,
                    title_allowed: true,
                },
            ]) {
                for violation in violations {
                    let diagnostic = compiler_definition_diagnostic(violation);
                    if !diagnostics.contains(&diagnostic) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
    }
    diagnostics
}

async fn evaluate_profile<C: DetectionHttpClient + Sync>(
    input_url: &str,
    profile_index: usize,
    profile: &SourceProfileDocument,
    http_client: &C,
    browser_client: Option<&(dyn ProfileBrowserClient + Sync)>,
) -> Candidate {
    let Some(detection) = &profile.detection else {
        return Candidate {
            proposal: None,
            unsupported: None,
            failed: false,
            diagnostics: Vec::new(),
        };
    };

    let base_path = format!("/profiles/{profile_index}/detect");
    let mut diagnostics = Vec::new();
    let Some((mut captures, mut evidence)) =
        match_input_url_patterns(input_url, detection, &base_path, &mut diagnostics)
    else {
        let failed = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
        return Candidate {
            proposal: None,
            unsupported: None,
            failed,
            diagnostics,
        };
    };

    if !evaluate_http_checks(
        input_url,
        detection.http_checks.as_deref().unwrap_or_default(),
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

    if let Some(browser_probes) = detection.browser_probes.as_deref() {
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
            let recommended_path = recommended_access_path(profile, detection);
            let source_config_for_probes = match build_source_config(
                input_url,
                profile,
                recommended_path,
                detection,
                &captures,
            ) {
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
            if let Some(access_path) = recommended_path {
                if let Err(value_diagnostics) = validate_detection_source_config_values(
                    &source_config_for_probes,
                    profile,
                    access_path,
                    &base_path,
                    ValidationCompleteness::Incremental,
                ) {
                    diagnostics.extend(value_diagnostics);
                    return Candidate {
                        proposal: None,
                        unsupported: None,
                        failed: true,
                        diagnostics,
                    };
                }
            }
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

    match build_source_proposal(
        input_url, profile, detection, captures, evidence, &base_path,
    ) {
        Ok(proposal) => Candidate {
            proposal: Some(proposal),
            unsupported: None,
            failed: false,
            diagnostics,
        },
        Err(proposal_diagnostics) => {
            diagnostics.extend(proposal_diagnostics);
            Candidate {
                proposal: None,
                unsupported: None,
                failed: true,
                diagnostics,
            }
        }
    }
}

fn match_input_url_patterns(
    input_url: &str,
    detection: &DetectionDocument,
    base_path: &str,
    diagnostics: &mut Diagnostics,
) -> Option<(BTreeMap<String, String>, Vec<SourceProposalEvidence>)> {
    let patterns = detection.input_url_patterns.as_deref().unwrap_or_default();
    if patterns.is_empty() {
        return Some((BTreeMap::new(), detection_document_evidence(detection)));
    }

    for (index, pattern) in patterns.iter().enumerate() {
        let regex = match Regex::new(&pattern.pattern) {
            Ok(regex) => regex,
            Err(error) => {
                diagnostics.push(detection_error(
                    "invalid_input_url_pattern_regex",
                    format!("Profile Detection input URL pattern is an invalid regex: {error}"),
                    format!("{base_path}/inputUrlPatterns/{index}/pattern"),
                    None,
                    serde_json::json!({ "pattern": pattern.pattern }),
                ));
                return None;
            }
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

        let mut evidence = detection_document_evidence(detection);
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
