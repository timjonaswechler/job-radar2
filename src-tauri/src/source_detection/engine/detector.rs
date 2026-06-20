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

pub(super) fn source_profile_registry_warnings(
    diagnostics: &[SourceRegistryDiagnostic],
) -> Vec<String> {
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
