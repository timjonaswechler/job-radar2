use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::{DetectionEvidenceKind, JsonObject, SupportLevel};
use crate::profile_dsl::source_config::{
    compile_contract, escape_pointer_segment, ContractViolation, EffectiveSourceConfigContract,
    SchemaLocation,
};
use crate::source_profile::documents::SourceProfileDocument;

const MAX_DIAGNOSTIC_PATH_CHARS: usize = 256;
const MAX_DIAGNOSTIC_ORIGINS: usize = 16;
const PREPARATION_STRATEGY: &str = "proposal_preparation";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionOrigin {
    strategy_key: String,
    schema_path: String,
}

impl DetectionOrigin {
    pub fn new(
        strategy_key: impl Into<String>,
        schema_path: impl Into<String>,
    ) -> Result<Self, DetectionDefinitionError> {
        let strategy_key = strategy_key.into();
        let schema_path = schema_path.into();
        if strategy_key.trim().is_empty() {
            return Err(DetectionDefinitionError::new(
                "invalid_detection_origin",
                "Detection origin requires a non-empty Strategy key",
                &schema_path,
            ));
        }
        if !is_authored_descriptor_path(&schema_path) {
            return Err(DetectionDefinitionError::new(
                "invalid_detection_origin",
                "Detection origin requires an absolute authored descriptor path",
                &schema_path,
            ));
        }
        Ok(Self {
            strategy_key,
            schema_path,
        })
    }

    pub fn strategy_key(&self) -> &str {
        &self.strategy_key
    }

    pub fn schema_path(&self) -> &str {
        &self.schema_path
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetectionDefinitionError {
    diagnostic: Diagnostic,
}

impl DetectionDefinitionError {
    fn new(code: &str, message: &str, path: &str) -> Self {
        Self {
            diagnostic: Diagnostic {
                category: DiagnosticCategory::Detection,
                code: code.to_string(),
                message: message.to_string(),
                severity: DiagnosticSeverity::Error,
                path: bounded(path),
                strategy_key: None,
                details: None,
            },
        }
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalConfigPointer {
    encoded: String,
    segments: Vec<String>,
}

impl CanonicalConfigPointer {
    fn parse(pointer: &str) -> Result<Self, DetectionDefinitionError> {
        if pointer.is_empty() || !pointer.starts_with('/') {
            return Err(DetectionDefinitionError::new(
                "invalid_detection_source_config_pointer",
                "Detection Source Config responsibility requires a non-root RFC-6901 JSON Pointer",
                pointer,
            ));
        }
        let mut segments = Vec::new();
        for encoded in pointer[1..].split('/') {
            let decoded = decode_pointer_segment(encoded).ok_or_else(|| {
                DetectionDefinitionError::new(
                    "invalid_detection_source_config_pointer",
                    "Detection Source Config responsibility contains an invalid RFC-6901 escape",
                    pointer,
                )
            })?;
            if escape_pointer_segment(&decoded) != encoded {
                return Err(DetectionDefinitionError::new(
                    "noncanonical_detection_source_config_pointer",
                    "Detection Source Config responsibility must use canonical RFC-6901 escaping",
                    pointer,
                ));
            }
            segments.push(decoded);
        }
        Ok(Self {
            encoded: pointer.to_string(),
            segments,
        })
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.segments.len() != other.segments.len()
            && (self.segments.starts_with(&other.segments)
                || other.segments.starts_with(&self.segments))
    }
}

fn decode_pointer_segment(segment: &str) -> Option<String> {
    let mut decoded = String::new();
    let mut characters = segment.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match characters.next()? {
            '0' => decoded.push('~'),
            '1' => decoded.push('/'),
            _ => return None,
        }
    }
    Some(decoded)
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetectionConfigContribution {
    pointer: CanonicalConfigPointer,
    value: Value,
}

impl DetectionConfigContribution {
    pub fn new(pointer: impl AsRef<str>, value: Value) -> Result<Self, DetectionDefinitionError> {
        Ok(Self {
            pointer: CanonicalConfigPointer::parse(pointer.as_ref())?,
            value,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionEvidenceContribution {
    kind: DetectionEvidenceKind,
    descriptor_path: String,
    message: String,
}

impl DetectionEvidenceContribution {
    pub fn new(
        kind: DetectionEvidenceKind,
        descriptor_path: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, DetectionDefinitionError> {
        let descriptor_path = descriptor_path.into();
        if !is_authored_descriptor_path(&descriptor_path) {
            return Err(DetectionDefinitionError::new(
                "invalid_detection_evidence_descriptor_path",
                "Detection evidence requires an absolute authored descriptor path",
                &descriptor_path,
            ));
        }
        Ok(Self {
            kind,
            descriptor_path,
            message: message.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetectionContribution {
    origins: Vec<DetectionOrigin>,
    captures: Vec<(String, String)>,
    source_config: Vec<DetectionConfigContribution>,
    recommendation: Option<String>,
    evidence: Vec<DetectionEvidenceContribution>,
}

impl DetectionContribution {
    pub fn new(origin: DetectionOrigin) -> Self {
        Self {
            origins: vec![origin],
            captures: Vec::new(),
            source_config: Vec::new(),
            recommendation: None,
            evidence: Vec::new(),
        }
    }

    pub fn with_origins(origins: Vec<DetectionOrigin>) -> Result<Self, DetectionDefinitionError> {
        if origins.is_empty() {
            return Err(DetectionDefinitionError::new(
                "missing_detection_contribution_origin",
                "Detection contribution requires at least one complete origin",
                "/detection",
            ));
        }
        let mut deduplicated = Vec::new();
        for origin in origins {
            union_origin(&mut deduplicated, &origin);
        }
        Ok(Self {
            origins: deduplicated,
            captures: Vec::new(),
            source_config: Vec::new(),
            recommendation: None,
            evidence: Vec::new(),
        })
    }

    pub fn with_capture(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.captures.push((key.into(), value.into()));
        self
    }

    pub fn with_config(mut self, contribution: DetectionConfigContribution) -> Self {
        self.source_config.push(contribution);
        self
    }

    pub fn with_recommendation(mut self, access_path_key: impl Into<String>) -> Self {
        self.recommendation = Some(access_path_key.into());
        self
    }

    pub fn with_evidence(mut self, evidence: DetectionEvidenceContribution) -> Self {
        self.evidence.push(evidence);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledCapture {
    key: String,
    value: String,
    origins: Vec<DetectionOrigin>,
}

impl ReconciledCapture {
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn origins(&self) -> &[DetectionOrigin] {
        &self.origins
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledSourceConfigValue {
    pointer: String,
    value: Value,
    origins: Vec<DetectionOrigin>,
    #[serde(skip)]
    segments: Vec<String>,
}

impl ReconciledSourceConfigValue {
    pub fn pointer(&self) -> &str {
        &self.pointer
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn origins(&self) -> &[DetectionOrigin] {
        &self.origins
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledRecommendation {
    access_path_key: String,
    origins: Vec<DetectionOrigin>,
}

impl ReconciledRecommendation {
    pub fn access_path_key(&self) -> &str {
        &self.access_path_key
    }
    pub fn origins(&self) -> &[DetectionOrigin] {
        &self.origins
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledEvidence {
    kind: DetectionEvidenceKind,
    descriptor_path: String,
    message: String,
    origins: Vec<DetectionOrigin>,
}

impl ReconciledEvidence {
    pub fn kind(&self) -> DetectionEvidenceKind {
        self.kind
    }
    pub fn descriptor_path(&self) -> &str {
        &self.descriptor_path
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn origins(&self) -> &[DetectionOrigin] {
        &self.origins
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledDetectionState {
    captures: Vec<ReconciledCapture>,
    source_config: Vec<ReconciledSourceConfigValue>,
    recommendation: Option<ReconciledRecommendation>,
    evidence: Vec<ReconciledEvidence>,
}

impl ReconciledDetectionState {
    pub fn captures(&self) -> &[ReconciledCapture] {
        &self.captures
    }
    pub fn source_config(&self) -> &[ReconciledSourceConfigValue] {
        &self.source_config
    }
    pub fn recommendation(&self) -> Option<&ReconciledRecommendation> {
        self.recommendation.as_ref()
    }
    pub fn evidence(&self) -> &[ReconciledEvidence] {
        &self.evidence
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionStateConflictKind {
    Capture,
    SourceConfigValue,
    SourceConfigOverlap,
    Recommendation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionStateConflict {
    kind: DetectionStateConflictKind,
    responsibility_path: String,
    existing_origins: Vec<DetectionOrigin>,
    incoming_origins: Vec<DetectionOrigin>,
}

impl DetectionStateConflict {
    pub fn kind(&self) -> DetectionStateConflictKind {
        self.kind
    }
    pub fn responsibility_path(&self) -> &str {
        &self.responsibility_path
    }

    fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            category: DiagnosticCategory::Detection,
            code: "detection_contribution_conflict".to_string(),
            message: "Detection contributions conflict for one retained responsibility".to_string(),
            severity: DiagnosticSeverity::Error,
            path: bounded(&self.responsibility_path),
            strategy_key: None,
            details: Some(serde_json::json!({
                "kind": self.kind,
                "responsibilityPath": bounded(&self.responsibility_path),
                "existingOrigins": self.existing_origins.iter().take(MAX_DIAGNOSTIC_ORIGINS).map(diagnostic_origin).collect::<Vec<_>>(),
                "incomingOrigins": self.incoming_origins.iter().take(MAX_DIAGNOSTIC_ORIGINS).map(diagnostic_origin).collect::<Vec<_>>()
            })),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DetectionReconciliationError {
    Conflict(DetectionStateConflict),
    InvalidState(Diagnostics),
}

impl DetectionReconciliationError {
    pub fn diagnostics(&self) -> Diagnostics {
        match self {
            Self::Conflict(conflict) => vec![conflict.diagnostic()],
            Self::InvalidState(diagnostics) => diagnostics.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct DetectionAccessPathContext {
    key: String,
    name: String,
    contract: EffectiveSourceConfigContract,
}

#[derive(Clone, Debug)]
pub struct DetectionProfileContext {
    profile_key: String,
    profile_name: String,
    support_level: SupportLevel,
    explicit_recommendation: Option<String>,
    profile_contract: EffectiveSourceConfigContract,
    access_paths: Vec<DetectionAccessPathContext>,
    profile_evidence: Vec<DetectionEvidenceContribution>,
}

impl DetectionProfileContext {
    pub fn compile(profile: &SourceProfileDocument) -> Result<Self, Diagnostics> {
        let mut diagnostics = Vec::new();
        let detection = match profile.detection.as_ref() {
            Some(detection) => detection,
            None => {
                return Err(vec![definition_diagnostic(
                    "missing_detection_plan",
                    "Source Profile does not define Detection",
                    "/detection",
                )]);
            }
        };

        // Definition validation is deliberately exhaustive across the profile and
        // every Access Path before any state can be created or acquisition can begin.
        let profile_contract = compile_contract(&[SchemaLocation {
            schema: profile.source_config_schema.as_ref(),
            path: "/sourceConfigSchema",
            title_allowed: true,
        }])
        .map_err(|violations| {
            violations
                .into_iter()
                .map(contract_definition_diagnostic)
                .collect::<Vec<_>>()
        })?;
        let mut access_paths = Vec::new();
        for (index, access_path) in profile.access_paths.iter().enumerate() {
            let access_path_path = format!("/accessPaths/{index}/sourceConfigSchema");
            match compile_contract(&[
                SchemaLocation {
                    schema: profile.source_config_schema.as_ref(),
                    path: "/sourceConfigSchema",
                    title_allowed: true,
                },
                SchemaLocation {
                    schema: access_path.source_config_schema.as_ref(),
                    path: &access_path_path,
                    title_allowed: true,
                },
            ]) {
                Ok(contract) => access_paths.push(DetectionAccessPathContext {
                    key: access_path.key.clone(),
                    name: access_path.name.clone(),
                    contract,
                }),
                Err(violations) => {
                    diagnostics.extend(violations.into_iter().map(contract_definition_diagnostic))
                }
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        if let Some(key) = detection.recommended_access_path_key.as_deref() {
            if !access_paths.iter().any(|path| path.key == key) {
                return Err(vec![definition_diagnostic(
                    "recommended_access_path_not_found",
                    "Detection recommends an Access Path that the Source Profile does not define",
                    "/detection/recommendedAccessPathKey",
                )]);
            }
        }
        let mut profile_evidence = Vec::new();
        for (index, evidence) in detection
            .evidence
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let descriptor_path = format!("/detection/evidence/{index}");
            profile_evidence.push(
                DetectionEvidenceContribution::new(
                    evidence.kind,
                    descriptor_path,
                    evidence.message.clone(),
                )
                .expect("compiled profile evidence has a canonical descriptor path"),
            );
        }

        Ok(Self {
            profile_key: profile.key.clone(),
            profile_name: profile.name.clone(),
            support_level: profile.support.level,
            explicit_recommendation: detection.recommended_access_path_key.clone(),
            profile_contract,
            access_paths,
            profile_evidence,
        })
    }

    pub fn initial_state(&self) -> ReconciledDetectionState {
        let mut state = ReconciledDetectionState::default();
        for evidence in &self.profile_evidence {
            let origin = DetectionOrigin::new("profile", evidence.descriptor_path.clone())
                .expect("compiled profile evidence descriptor is a valid origin");
            state = reduce(
                &state,
                DetectionContribution::new(origin).with_evidence(evidence.clone()),
            )
            .expect("profile evidence cannot conflict");
        }
        state
    }

    pub fn apply(
        &self,
        state: &ReconciledDetectionState,
        contribution: DetectionContribution,
    ) -> Result<ReconciledDetectionState, DetectionReconciliationError> {
        let next = reduce(state, contribution).map_err(DetectionReconciliationError::Conflict)?;
        let values = materialize_source_config(&next.source_config);
        let contract = self.contract_for_state(&next)?;
        let violations = contract.validate_incremental(&values);
        if violations.is_empty() {
            Ok(next)
        } else {
            Err(DetectionReconciliationError::InvalidState(
                violations
                    .into_iter()
                    .map(contract_value_diagnostic)
                    .collect(),
            ))
        }
    }

    fn contract_for_state(
        &self,
        state: &ReconciledDetectionState,
    ) -> Result<&EffectiveSourceConfigContract, DetectionReconciliationError> {
        if state.recommendation.as_ref().is_some_and(|recommendation| {
            !self
                .access_paths
                .iter()
                .any(|path| path.key == recommendation.access_path_key)
        }) {
            return Err(DetectionReconciliationError::InvalidState(vec![
                definition_diagnostic(
                    "recommended_access_path_not_found",
                    "Detection produced an unknown Access Path recommendation",
                    "/recommendedAccessPathKey",
                ),
            ]));
        }
        let selected_key = self
            .explicit_recommendation
            .as_deref()
            .or_else(|| {
                state
                    .recommendation
                    .as_ref()
                    .map(|recommendation| recommendation.access_path_key.as_str())
            })
            .or_else(|| (self.access_paths.len() == 1).then(|| self.access_paths[0].key.as_str()));
        let Some(selected_key) = selected_key else {
            return Ok(&self.profile_contract);
        };
        self.access_paths
            .iter()
            .find(|path| path.key == selected_key)
            .map(|path| &path.contract)
            .ok_or_else(|| {
                DetectionReconciliationError::InvalidState(vec![definition_diagnostic(
                    "recommended_access_path_not_found",
                    "Detection produced an unknown Access Path recommendation",
                    "/recommendedAccessPathKey",
                )])
            })
    }

    pub fn complete(
        &self,
        state: &ReconciledDetectionState,
    ) -> Result<(), DetectionReconciliationError> {
        let contract = self.contract_for_state(state)?;
        let violations =
            contract.validate_complete(&materialize_source_config(&state.source_config));
        if violations.is_empty() {
            Ok(())
        } else {
            Err(DetectionReconciliationError::InvalidState(
                violations
                    .into_iter()
                    .map(contract_value_diagnostic)
                    .collect(),
            ))
        }
    }

    pub fn prepare_proposal(
        &self,
        state: &ReconciledDetectionState,
        input_url: &str,
        rendered_source_config_template: Option<JsonObject>,
        key_candidates: Vec<String>,
        name_candidates: Vec<String>,
    ) -> Result<PreparedDetectionOutput, DetectionReconciliationError> {
        let canonical_input_url = canonical_http_url(input_url)
            .map_err(|diagnostic| DetectionReconciliationError::InvalidState(vec![diagnostic]))?;
        self.prepare_proposal_with_canonical_url(
            state,
            &canonical_input_url,
            rendered_source_config_template,
            key_candidates,
            name_candidates,
        )
    }

    pub(crate) fn prepare_proposal_with_canonical_url(
        &self,
        state: &ReconciledDetectionState,
        canonical_input_url: &str,
        rendered_source_config_template: Option<JsonObject>,
        key_candidates: Vec<String>,
        name_candidates: Vec<String>,
    ) -> Result<PreparedDetectionOutput, DetectionReconciliationError> {
        let selected_key = self
            .explicit_recommendation
            .clone()
            .or_else(|| {
                state
                    .recommendation
                    .as_ref()
                    .map(|recommendation| recommendation.access_path_key.clone())
            })
            .or_else(|| (self.access_paths.len() == 1).then(|| self.access_paths[0].key.clone()))
            .ok_or_else(|| {
                DetectionReconciliationError::InvalidState(vec![definition_diagnostic(
                    "recommended_access_path_not_found",
                    "Detection did not produce an Access Path recommendation",
                    "/recommendedAccessPathKey",
                )])
            })?;
        let selected = self
            .access_paths
            .iter()
            .find(|path| path.key == selected_key)
            .ok_or_else(|| {
                DetectionReconciliationError::InvalidState(vec![definition_diagnostic(
                    "recommended_access_path_not_found",
                    "Detection produced an unknown Access Path recommendation",
                    "/recommendedAccessPathKey",
                )])
            })?;
        let property_keys = selected.contract.property_keys();
        let mut next = state.clone();

        if let Some(template) = rendered_source_config_template {
            for (key, value) in template {
                let pointer = format!("/{}", escape_pointer_segment(&key));
                let config = DetectionConfigContribution::new(&pointer, value)
                    .expect("escaped object key is a canonical pointer");
                let origin = DetectionOrigin::new(
                    PREPARATION_STRATEGY,
                    format!("/detection/sourceConfig/{}", escape_pointer_segment(&key)),
                )
                .expect("escaped object key is a canonical descriptor path");
                next = self.apply(
                    &next,
                    DetectionContribution::new(origin).with_config(config),
                )?;
            }
        }

        for property in &property_keys {
            let Some(capture) = next
                .captures
                .iter()
                .find(|capture| capture.key == *property)
            else {
                continue;
            };
            let pointer = format!("/{}", escape_pointer_segment(property));
            let config =
                DetectionConfigContribution::new(pointer, Value::String(capture.value.clone()))
                    .expect("escaped property is a canonical pointer");
            let derivation_origin = DetectionOrigin::new(
                PREPARATION_STRATEGY,
                format!(
                    "/sourceConfigSchema/properties/{}",
                    escape_pointer_segment(property)
                ),
            )
            .expect("escaped property is a canonical descriptor path");
            let mut origins = capture.origins.clone();
            origins.push(derivation_origin);
            next = self.apply(
                &next,
                DetectionContribution::with_origins(origins)
                    .expect("capture-copy contribution retains at least one origin")
                    .with_config(config),
            )?;
        }

        if selected.contract.requires_property("startUrl") {
            let config = DetectionConfigContribution::new(
                "/startUrl",
                Value::String(canonical_input_url.to_string()),
            )
            .expect("static startUrl pointer is valid");
            let origin = DetectionOrigin::new(PREPARATION_STRATEGY, "/inputUrl")
                .expect("static input origin is valid");
            next = self.apply(
                &next,
                DetectionContribution::new(origin).with_config(config),
            )?;
        }

        if let Some(explicit) = self.explicit_recommendation.as_deref() {
            let origin =
                DetectionOrigin::new(PREPARATION_STRATEGY, "/detection/recommendedAccessPathKey")
                    .expect("static recommendation origin is valid");
            next = self.apply(
                &next,
                DetectionContribution::new(origin).with_recommendation(explicit),
            )?;
        } else if self.access_paths.len() == 1 {
            let origin = DetectionOrigin::new(PREPARATION_STRATEGY, "/accessPaths")
                .expect("static sole-path origin is valid");
            next = self.apply(
                &next,
                DetectionContribution::new(origin).with_recommendation(&selected.key),
            )?;
        }
        self.complete(&next)?;

        let proposal = construct_proposal(self, selected, &next, key_candidates, name_candidates);
        if self.support_level == SupportLevel::Unsupported {
            Ok(PreparedDetectionOutput::Unsupported(
                UnsupportedReconciledDetection {
                    profile_key: proposal.profile_key,
                    profile_name: proposal.profile_name,
                    captures: proposal.captures,
                    evidence: proposal.evidence,
                    provenance: proposal.provenance,
                    support_level: proposal.support_level,
                },
            ))
        } else {
            Ok(PreparedDetectionOutput::Proposal(proposal))
        }
    }
}

fn reduce(
    state: &ReconciledDetectionState,
    contribution: DetectionContribution,
) -> Result<ReconciledDetectionState, DetectionStateConflict> {
    let mut next = state.clone();
    let incoming_origins = contribution.origins;

    for (key, value) in contribution.captures {
        if let Some(existing) = next.captures.iter_mut().find(|item| item.key == key) {
            if existing.value != value {
                return Err(conflict(
                    DetectionStateConflictKind::Capture,
                    format!("/captures/{}", escape_pointer_segment(&key)),
                    &existing.origins,
                    &incoming_origins,
                ));
            }
            union_origins(&mut existing.origins, &incoming_origins);
        } else {
            next.captures.push(ReconciledCapture {
                key,
                value,
                origins: incoming_origins.clone(),
            });
        }
    }

    for item in contribution.source_config {
        for existing in &next.source_config {
            let existing_pointer = CanonicalConfigPointer {
                encoded: existing.pointer.clone(),
                segments: existing.segments.clone(),
            };
            if existing_pointer.overlaps(&item.pointer) {
                return Err(conflict(
                    DetectionStateConflictKind::SourceConfigOverlap,
                    format!("/sourceConfig{}", item.pointer.encoded),
                    &existing.origins,
                    &incoming_origins,
                ));
            }
        }
        if let Some(existing) = next
            .source_config
            .iter_mut()
            .find(|value| value.pointer == item.pointer.encoded)
        {
            if existing.value != item.value {
                return Err(conflict(
                    DetectionStateConflictKind::SourceConfigValue,
                    format!("/sourceConfig{}", item.pointer.encoded),
                    &existing.origins,
                    &incoming_origins,
                ));
            }
            union_origins(&mut existing.origins, &incoming_origins);
        } else {
            next.source_config.push(ReconciledSourceConfigValue {
                pointer: item.pointer.encoded,
                value: item.value,
                origins: incoming_origins.clone(),
                segments: item.pointer.segments,
            });
        }
    }

    if let Some(access_path_key) = contribution.recommendation {
        if let Some(existing) = next.recommendation.as_mut() {
            if existing.access_path_key != access_path_key {
                return Err(conflict(
                    DetectionStateConflictKind::Recommendation,
                    "/recommendedAccessPathKey".to_string(),
                    &existing.origins,
                    &incoming_origins,
                ));
            }
            union_origins(&mut existing.origins, &incoming_origins);
        } else {
            next.recommendation = Some(ReconciledRecommendation {
                access_path_key,
                origins: incoming_origins.clone(),
            });
        }
    }

    for item in contribution.evidence {
        if let Some(existing) = next.evidence.iter_mut().find(|evidence| {
            evidence.kind == item.kind && evidence.descriptor_path == item.descriptor_path
        }) {
            union_origins(&mut existing.origins, &incoming_origins);
        } else {
            next.evidence.push(ReconciledEvidence {
                kind: item.kind,
                descriptor_path: item.descriptor_path,
                message: item.message,
                origins: incoming_origins.clone(),
            });
        }
    }

    Ok(next)
}

fn union_origins(origins: &mut Vec<DetectionOrigin>, incoming: &[DetectionOrigin]) {
    for origin in incoming {
        union_origin(origins, origin);
    }
}

fn union_origin(origins: &mut Vec<DetectionOrigin>, incoming: &DetectionOrigin) {
    if !origins.iter().any(|origin| origin == incoming) {
        origins.push(incoming.clone());
    }
}

fn conflict(
    kind: DetectionStateConflictKind,
    responsibility_path: String,
    existing_origins: &[DetectionOrigin],
    incoming_origins: &[DetectionOrigin],
) -> DetectionStateConflict {
    DetectionStateConflict {
        kind,
        responsibility_path,
        existing_origins: existing_origins.to_vec(),
        incoming_origins: incoming_origins.to_vec(),
    }
}

fn materialize_source_config(values: &[ReconciledSourceConfigValue]) -> JsonObject {
    let mut root = Map::new();
    for item in values {
        insert_atomic_value(&mut root, &item.segments, item.value.clone());
    }
    root
}

fn insert_atomic_value(root: &mut JsonObject, segments: &[String], value: Value) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    if rest.is_empty() {
        root.insert(first.clone(), value);
        return;
    }
    let child = root
        .entry(first.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    let object = child
        .as_object_mut()
        .expect("ancestor overlap prevention guarantees an object container");
    insert_atomic_value(object, rest, value);
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposalEvidence {
    pub kind: DetectionEvidenceKind,
    pub descriptor_path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionProposalProvenance {
    pub captures: BTreeMap<String, Vec<DetectionOrigin>>,
    pub source_config: BTreeMap<String, Vec<DetectionOrigin>>,
    pub recommendation: Vec<DetectionOrigin>,
    pub evidence: Vec<Vec<DetectionOrigin>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconciledSourceProposal {
    pub profile_key: String,
    pub profile_name: String,
    pub recommended_access_path_key: String,
    pub recommended_access_path_name: String,
    pub source_config: Value,
    pub key_candidates: Vec<String>,
    pub name_candidates: Vec<String>,
    pub captures: BTreeMap<String, String>,
    pub evidence: Vec<ProposalEvidence>,
    pub support_level: SupportLevel,
    pub provenance: DetectionProposalProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnsupportedReconciledDetection {
    pub profile_key: String,
    pub profile_name: String,
    pub captures: BTreeMap<String, String>,
    pub evidence: Vec<ProposalEvidence>,
    pub support_level: SupportLevel,
    pub provenance: DetectionProposalProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreparedDetectionOutput {
    Proposal(ReconciledSourceProposal),
    Unsupported(UnsupportedReconciledDetection),
}

impl PreparedDetectionOutput {
    pub fn proposal(&self) -> Option<&ReconciledSourceProposal> {
        match self {
            Self::Proposal(proposal) => Some(proposal),
            Self::Unsupported(_) => None,
        }
    }
}

fn construct_proposal(
    context: &DetectionProfileContext,
    access_path: &DetectionAccessPathContext,
    state: &ReconciledDetectionState,
    key_candidates: Vec<String>,
    name_candidates: Vec<String>,
) -> ReconciledSourceProposal {
    let captures = state
        .captures
        .iter()
        .map(|capture| (capture.key.clone(), capture.value.clone()))
        .collect();
    let evidence = state
        .evidence
        .iter()
        .map(|evidence| ProposalEvidence {
            kind: evidence.kind,
            descriptor_path: evidence.descriptor_path.clone(),
            message: evidence.message.clone(),
        })
        .collect();
    let provenance = DetectionProposalProvenance {
        captures: state
            .captures
            .iter()
            .map(|capture| (capture.key.clone(), capture.origins.clone()))
            .collect(),
        source_config: state
            .source_config
            .iter()
            .map(|value| (value.pointer.clone(), value.origins.clone()))
            .collect(),
        recommendation: state
            .recommendation
            .as_ref()
            .map(|recommendation| recommendation.origins.clone())
            .unwrap_or_default(),
        evidence: state
            .evidence
            .iter()
            .map(|evidence| evidence.origins.clone())
            .collect(),
    };
    ReconciledSourceProposal {
        profile_key: context.profile_key.clone(),
        profile_name: context.profile_name.clone(),
        recommended_access_path_key: access_path.key.clone(),
        recommended_access_path_name: access_path.name.clone(),
        source_config: Value::Object(materialize_source_config(&state.source_config)),
        key_candidates,
        name_candidates,
        captures,
        evidence,
        support_level: context.support_level,
        provenance,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DetectionAttempt {
    Matched(ReconciledSourceProposal),
    Unsupported(UnsupportedReconciledDetection),
    Failed(Diagnostics),
    Conflict(Diagnostics),
    BudgetExhausted(Diagnostics),
    Cancelled(Diagnostics),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRunStatus {
    Matched,
    Ambiguous,
    Unsupported,
    Failed,
    BudgetExhausted,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconciledDetectionRunResult {
    pub status: DetectionRunStatus,
    pub proposals: Vec<ReconciledSourceProposal>,
    pub unsupported_profiles: Vec<UnsupportedReconciledDetection>,
    pub diagnostics: Diagnostics,
}

pub fn aggregate_detection_attempts(
    attempts: Vec<DetectionAttempt>,
) -> ReconciledDetectionRunResult {
    let mut proposals = Vec::new();
    let mut unsupported_profiles = Vec::new();
    let mut diagnostics = Vec::new();
    let mut failed = false;
    let mut budget_exhausted = false;
    let mut cancelled = false;
    for attempt in attempts {
        match attempt {
            DetectionAttempt::Matched(proposal) => proposals.push(proposal),
            DetectionAttempt::Unsupported(profile) => unsupported_profiles.push(profile),
            DetectionAttempt::Failed(mut attempt_diagnostics)
            | DetectionAttempt::Conflict(mut attempt_diagnostics) => {
                failed = true;
                diagnostics.append(&mut attempt_diagnostics);
            }
            DetectionAttempt::BudgetExhausted(mut attempt_diagnostics) => {
                budget_exhausted = true;
                diagnostics.append(&mut attempt_diagnostics);
            }
            DetectionAttempt::Cancelled(mut attempt_diagnostics) => {
                cancelled = true;
                diagnostics.append(&mut attempt_diagnostics);
            }
        }
    }
    let status = if cancelled {
        proposals.clear();
        unsupported_profiles.clear();
        DetectionRunStatus::Cancelled
    } else if budget_exhausted {
        proposals.clear();
        unsupported_profiles.clear();
        DetectionRunStatus::BudgetExhausted
    } else if proposals.len() > 1 {
        DetectionRunStatus::Ambiguous
    } else if proposals.len() == 1 {
        DetectionRunStatus::Matched
    } else if failed {
        DetectionRunStatus::Failed
    } else {
        DetectionRunStatus::Unsupported
    };
    ReconciledDetectionRunResult {
        status,
        proposals,
        unsupported_profiles,
        diagnostics,
    }
}

fn canonical_http_url(input: &str) -> Result<String, Diagnostic> {
    let url = Url::parse(input.trim()).map_err(|_| {
        definition_diagnostic(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL",
            "/inputUrl",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(definition_diagnostic(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL without userinfo",
            "/inputUrl",
        ));
    }
    Ok(url.to_string())
}

fn contract_definition_diagnostic(violation: ContractViolation) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Compiler,
        code: violation.code.to_string(),
        message: violation.message,
        severity: DiagnosticSeverity::Error,
        path: violation.path,
        strategy_key: None,
        details: Some(violation.details),
    }
}

fn contract_value_diagnostic(violation: ContractViolation) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: violation.code.to_string(),
        message: violation.message,
        severity: DiagnosticSeverity::Error,
        path: format!("/sourceConfig{}", violation.path),
        strategy_key: None,
        details: Some(violation.details),
    }
}

fn definition_diagnostic(code: &str, message: &str, path: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.to_string(),
        strategy_key: None,
        details: None,
    }
}

fn is_authored_descriptor_path(path: &str) -> bool {
    !path.is_empty() && path.starts_with('/') && decode_descriptor_path(path)
}

fn decode_descriptor_path(path: &str) -> bool {
    path[1..]
        .split('/')
        .all(|segment| decode_pointer_segment(segment).is_some())
}

fn diagnostic_origin(origin: &DetectionOrigin) -> Value {
    serde_json::json!({
        "strategyKey": bounded(origin.strategy_key()),
        "schemaPath": bounded(origin.schema_path()),
    })
}

fn bounded(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_DIAGNOSTIC_PATH_CHARS)
        .collect()
}
