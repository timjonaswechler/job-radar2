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

#[derive(Default)]
pub(super) struct ProfileEvaluation {
    matches: Vec<SourceDetectionMatch>,
    warnings: Vec<String>,
}

pub(in crate::source_detection) async fn detect_with_source_profiles<
    C: DetectionHttpClient + Sync,
>(
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

pub(super) async fn evaluate_source_profile<C: DetectionHttpClient + Sync>(
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

pub(super) fn detect_supports_http(detect: &DetectionBlock) -> bool {
    detect.phases.is_empty() || detect.phases.contains(&DetectionPhase::Http)
}

pub(super) async fn evaluate_detection_any_of<C: DetectionHttpClient + Sync>(
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

pub(super) async fn evaluate_access_path_availability<C: DetectionHttpClient + Sync>(
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

pub(super) async fn evaluate_availability_checks<C: DetectionHttpClient + Sync>(
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

pub(super) fn required_captures_available(
    availability: &AvailabilityBlock,
    captures: &HashMap<String, String>,
) -> bool {
    availability.required_captures.iter().all(|capture_key| {
        captures
            .get(capture_key)
            .is_some_and(|value| !value.trim().is_empty())
    })
}
