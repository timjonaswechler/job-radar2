//! Dormant canonical Source behavior fingerprint preparation.
//!
//! This module deliberately has no productive Source Live Check caller. A01
//! will activate the single preparation boundary after schema-v3 activation.
//! Each closed component is serialized and hashed independently; projection
//! material and version tokens never enter a Check Report.

use std::collections::HashSet;
use std::fmt;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::profile_dsl::compiler::{
    forbidden_request_key_behavior, CompileSourceOutcome, CompiledSource, CompiledSourceAccess,
    CompiledSourceProvenance, ProvenanceEntry, ProvenancePathSegment, SourceOwnedAccessPath,
    SourceRuntimeBinding, MAX_FALLBACK_STRATEGIES,
};
use crate::profile_dsl::documents::{
    AccessPathFragment, DetailStep, DiscoveryStep, JsonSchemaObject,
};
use crate::profile_dsl::execution_plan::ExecutionPlanAccessPath;
use crate::source::documents::{SelectedAccessPath, SourceDocument};
use crate::source_profile::documents::SourceProfileDocument;

use super::source_live::SOURCE_LIVE_CHECK_MAX_DISCOVERY_REQUESTS;
use super::CheckFingerprint;

const SOURCE_BEHAVIOR: &str = "source_behavior";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceBehaviorFingerprintPreparationErrorKind {
    Serialization,
    DuplicateIdentity,
    InconsistentCompilerOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBehaviorFingerprintPreparationError {
    pub kind: SourceBehaviorFingerprintPreparationErrorKind,
    pub component_kind: &'static str,
    pub component_reference: &'static str,
}

impl fmt::Display for SourceBehaviorFingerprintPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fingerprint preparation failed for {}/{}",
            self.component_kind, self.component_reference
        )
    }
}

impl std::error::Error for SourceBehaviorFingerprintPreparationError {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileBehaviorProjection<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config_schema: Option<Value>,
    access_paths: Vec<ReusableAccessPathBehaviorProjection<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReusableAccessPathBehaviorProjection<'a> {
    key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config_schema: Option<Value>,
    #[serde(rename = "discovery")]
    discovery: &'a DiscoveryStep,
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    detail: Option<&'a DetailStep>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectSourceSpecializationProjection<'a> {
    access_paths: Vec<DirectAccessPathBehaviorProjection<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectAccessPathBehaviorProjection<'a> {
    key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config_schema: Option<Value>,
    #[serde(rename = "discovery", skip_serializing_if = "Option::is_none")]
    discovery: Option<&'a crate::profile_dsl::documents::DiscoveryStepFragment>,
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    detail: Option<&'a crate::profile_dsl::documents::DetailStepFragment>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceOwnedAccessPathBehaviorProjection<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config_schema: Option<Value>,
    #[serde(rename = "discovery")]
    discovery: &'a DiscoveryStep,
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    detail: Option<&'a DetailStep>,
}

#[derive(Serialize)]
#[serde(tag = "branch", rename_all = "snake_case")]
enum SelectedAccessPathProjection<'a> {
    ProfileAccessPath {
        #[serde(rename = "profileKey")]
        profile_key: &'a str,
        #[serde(rename = "pathKey")]
        path_key: &'a str,
    },
    SourceOwnedAccessPath {
        key: &'a str,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBindingsProjection<'a> {
    name: &'a str,
}

/// Prepares the complete canonical-v3 fingerprint set without persisting or
/// activating it. Projection internals remain closed to checks ownership.
///
/// `outcome` must be the exact result produced for `source` and
/// `resolved_base_profile` by the caller's single operation-local compilation.
/// This boundary performs structural coherence checks but deliberately does not
/// replay compilation or reconstruct the merge to prove that precondition.
#[doc(hidden)]
pub fn prepare_source_behavior_fingerprints(
    source: &SourceDocument,
    resolved_base_profile: Option<&SourceProfileDocument>,
    outcome: &CompileSourceOutcome,
) -> Result<Vec<CheckFingerprint>, SourceBehaviorFingerprintPreparationError> {
    validate_structural_coherence(source, resolved_base_profile, outcome)?;
    let mut fingerprints = Vec::new();

    match (&source.selected_access_path, outcome) {
        (
            SelectedAccessPath::ProfileAccessPath { .. },
            CompileSourceOutcome::Compiled {
                source: compiled, ..
            },
        ) => {
            let Some(base_profile) = resolved_base_profile else {
                return Err(inconsistent("base_source_profile"));
            };
            let CompiledSourceAccess::Profile { effective_profile } = &compiled.access else {
                return Err(inconsistent("effective_source_profile"));
            };
            push_component(
                &mut fingerprints,
                SOURCE_BEHAVIOR,
                "base_source_profile",
                &profile_projection(base_profile),
            )?;
            push_direct_component(&mut fingerprints, source)?;
            push_component(
                &mut fingerprints,
                SOURCE_BEHAVIOR,
                "effective_source_profile",
                &profile_projection(&effective_profile.document),
            )?;
            push_compiled_components(&mut fingerprints, source, compiled)?;
        }
        (
            SelectedAccessPath::SourceOwnedAccessPath { .. },
            CompileSourceOutcome::Compiled {
                source: compiled, ..
            },
        ) => {
            let CompiledSourceAccess::SourceOwned { access_path } = &compiled.access else {
                return Err(inconsistent("source_owned_access_path"));
            };
            push_component(
                &mut fingerprints,
                SOURCE_BEHAVIOR,
                "source_owned_access_path",
                &owned_projection(access_path),
            )?;
            push_compiled_components(&mut fingerprints, source, compiled)?;
        }
        (SelectedAccessPath::ProfileAccessPath { .. }, CompileSourceOutcome::Rejected { .. }) => {
            if let Some(base_profile) = resolved_base_profile {
                push_component(
                    &mut fingerprints,
                    SOURCE_BEHAVIOR,
                    "base_source_profile",
                    &profile_projection(base_profile),
                )?;
            }
            push_direct_component(&mut fingerprints, source)?;
            push_source_and_selector_components(&mut fingerprints, source)?;
        }
        (
            SelectedAccessPath::SourceOwnedAccessPath {
                source_config_schema,
                discovery,
                detail,
                ..
            },
            CompileSourceOutcome::Rejected { .. },
        ) => {
            let projection = SourceOwnedAccessPathBehaviorProjection {
                source_config_schema: executable_schema(source_config_schema.as_ref()),
                discovery,
                detail: detail.as_ref(),
            };
            push_component(
                &mut fingerprints,
                SOURCE_BEHAVIOR,
                "source_owned_access_path",
                &projection,
            )?;
            push_source_and_selector_components(&mut fingerprints, source)?;
        }
    }

    push_tail(&mut fingerprints)?;
    ensure_unique_identities(&fingerprints)?;
    Ok(fingerprints)
}

fn validate_structural_coherence(
    source: &SourceDocument,
    resolved_base_profile: Option<&SourceProfileDocument>,
    outcome: &CompileSourceOutcome,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    match &source.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            if resolved_base_profile.is_some_and(|profile| profile.key != *profile_key) {
                return Err(inconsistent("base_source_profile"));
            }
            if let CompileSourceOutcome::Compiled {
                source: compiled, ..
            } = outcome
            {
                let CompiledSourceAccess::Profile { effective_profile } = &compiled.access else {
                    return Err(inconsistent("effective_source_profile"));
                };
                let ExecutionPlanAccessPath::ProfileAccessPath {
                    profile_key: compiled_profile_key,
                    path_key: compiled_path_key,
                    ..
                } = &compiled.execution_plan.selected_access_path
                else {
                    return Err(inconsistent("selected_access_path"));
                };
                if compiled.execution_plan.source.key != source.key
                    || compiled.execution_plan.source.name != source.name
                    || effective_profile.document.key != *profile_key
                    || compiled_profile_key != profile_key
                    || compiled_path_key != path_key
                {
                    return Err(inconsistent("compiled_source"));
                }
            }
        }
        SelectedAccessPath::SourceOwnedAccessPath { key, .. } => {
            if resolved_base_profile.is_some() {
                return Err(inconsistent("base_source_profile"));
            }
            if let CompileSourceOutcome::Compiled {
                source: compiled, ..
            } = outcome
            {
                let CompiledSourceAccess::SourceOwned { access_path } = &compiled.access else {
                    return Err(inconsistent("source_owned_access_path"));
                };
                let ExecutionPlanAccessPath::SourceOwnedAccessPath {
                    key: compiled_key, ..
                } = &compiled.execution_plan.selected_access_path
                else {
                    return Err(inconsistent("selected_access_path"));
                };
                if compiled.execution_plan.source.key != source.key
                    || compiled.execution_plan.source.name != source.name
                    || access_path.key != *key
                    || compiled_key != key
                {
                    return Err(inconsistent("compiled_source"));
                }
            }
        }
    }
    Ok(())
}

fn push_compiled_components(
    fingerprints: &mut Vec<CheckFingerprint>,
    source: &SourceDocument,
    compiled: &CompiledSource,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    let filtered_provenance = filtered_provenance(&compiled.provenance);
    push_component(
        fingerprints,
        SOURCE_BEHAVIOR,
        "compiler_provenance",
        &filtered_provenance,
    )?;
    push_source_and_selector_components(fingerprints, source)?;
    if compiled
        .runtime_binding_dependencies
        .bindings
        .contains(&SourceRuntimeBinding::Name)
    {
        push_component(
            fingerprints,
            SOURCE_BEHAVIOR,
            "source_runtime_bindings",
            &RuntimeBindingsProjection { name: &source.name },
        )?;
    }
    Ok(())
}

fn push_source_and_selector_components(
    fingerprints: &mut Vec<CheckFingerprint>,
    source: &SourceDocument,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    push_component(
        fingerprints,
        SOURCE_BEHAVIOR,
        "source_config",
        &source.source_config,
    )?;
    let selector = match &source.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => SelectedAccessPathProjection::ProfileAccessPath {
            profile_key,
            path_key,
        },
        SelectedAccessPath::SourceOwnedAccessPath { key, .. } => {
            SelectedAccessPathProjection::SourceOwnedAccessPath { key }
        }
    };
    push_component(
        fingerprints,
        SOURCE_BEHAVIOR,
        "selected_access_path",
        &selector,
    )
}

fn push_direct_component(
    fingerprints: &mut Vec<CheckFingerprint>,
    source: &SourceDocument,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    let Some(access_paths) = source.access_paths.as_ref() else {
        return Ok(());
    };
    let access_paths = access_paths
        .iter()
        .filter(|fragment| direct_fragment_has_behavior(fragment))
        .map(|fragment| DirectAccessPathBehaviorProjection {
            key: &fragment.key,
            source_config_schema: executable_schema(fragment.source_config_schema.as_ref()),
            discovery: fragment.discovery.as_ref(),
            detail: fragment.detail.as_ref(),
        })
        .collect::<Vec<_>>();
    if access_paths.is_empty() {
        return Ok(());
    }
    push_component(
        fingerprints,
        SOURCE_BEHAVIOR,
        "direct_source_specialization",
        &DirectSourceSpecializationProjection { access_paths },
    )
}

fn direct_fragment_has_behavior(fragment: &AccessPathFragment) -> bool {
    fragment.source_config_schema.is_some()
        || fragment.discovery.is_some()
        || fragment.detail.is_some()
}

fn profile_projection(profile: &SourceProfileDocument) -> ProfileBehaviorProjection<'_> {
    ProfileBehaviorProjection {
        source_config_schema: executable_schema(profile.source_config_schema.as_ref()),
        access_paths: profile
            .access_paths
            .iter()
            .map(|path| ReusableAccessPathBehaviorProjection {
                key: &path.key,
                source_config_schema: executable_schema(path.source_config_schema.as_ref()),
                discovery: &path.discovery,
                detail: path.detail.as_ref(),
            })
            .collect(),
    }
}

fn owned_projection(
    access_path: &SourceOwnedAccessPath,
) -> SourceOwnedAccessPathBehaviorProjection<'_> {
    SourceOwnedAccessPathBehaviorProjection {
        source_config_schema: executable_schema(access_path.source_config_schema.as_ref()),
        discovery: &access_path.discovery,
        detail: access_path.detail.as_ref(),
    }
}

/// Removes the one admitted non-executable schema annotation at its constrained
/// property-schema location. serde_json's map representation recursively keeps
/// dynamic object keys ordered without changing semantic array order.
fn executable_schema(schema: Option<&JsonSchemaObject>) -> Option<Value> {
    let mut schema = schema.cloned()?;
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            if let Some(property) = property.as_object_mut() {
                property.remove("title");
            }
        }
    }
    Some(Value::Object(schema))
}

fn filtered_provenance(provenance: &CompiledSourceProvenance) -> CompiledSourceProvenance {
    fn include(entry: &ProvenanceEntry) -> bool {
        let metadata_name = matches!(
            entry.path.segments.last(),
            Some(ProvenancePathSegment::Field { name }) if name == "name"
        );
        let schema_title = matches!(
            entry.path.segments.last(),
            Some(ProvenancePathSegment::Field { name }) if name == "title"
        ) && entry.path.segments.iter().any(|segment| {
            matches!(segment, ProvenancePathSegment::Field { name } if name == "sourceConfigSchema")
        });
        !metadata_name && !schema_title
    }
    match provenance {
        CompiledSourceProvenance::Profile { entries } => CompiledSourceProvenance::Profile {
            entries: entries
                .iter()
                .filter(|entry| include(entry))
                .cloned()
                .collect(),
        },
        CompiledSourceProvenance::SourceOwned { entries } => {
            CompiledSourceProvenance::SourceOwned {
                entries: entries
                    .iter()
                    .filter(|entry| include(entry))
                    .cloned()
                    .collect(),
            }
        }
    }
}

fn push_tail(
    fingerprints: &mut Vec<CheckFingerprint>,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    push_component(
        fingerprints,
        "behavior_version",
        "profile_compiler",
        &"profile-compiler/v2",
    )?;
    push_component(
        fingerprints,
        "behavior_version",
        "profile_runtime",
        &"profile-runtime/v2",
    )?;
    push_component(
        fingerprints,
        "behavior_version",
        "immutable_globals",
        &"immutable-globals/v2",
    )?;
    push_component(
        fingerprints,
        "immutable_global_behavior",
        "source_live_check_cumulative_discovery_request_limit",
        &SOURCE_LIVE_CHECK_MAX_DISCOVERY_REQUESTS,
    )?;
    push_component(
        fingerprints,
        "immutable_global_behavior",
        "compiler_max_fallback_strategies",
        &MAX_FALLBACK_STRATEGIES,
    )?;
    push_component(
        fingerprints,
        "immutable_global_behavior",
        "security_forbidden_request_key_behavior",
        &forbidden_request_key_behavior(),
    )
}

fn push_component<T: Serialize>(
    fingerprints: &mut Vec<CheckFingerprint>,
    kind: &'static str,
    reference: &'static str,
    projection: &T,
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    let bytes =
        serde_json::to_vec(projection).map_err(|_| SourceBehaviorFingerprintPreparationError {
            kind: SourceBehaviorFingerprintPreparationErrorKind::Serialization,
            component_kind: kind,
            component_reference: reference,
        })?;
    fingerprints.push(CheckFingerprint::with_reference(
        kind,
        reference,
        format!("{:x}", Sha256::digest(bytes)),
    ));
    Ok(())
}

fn ensure_unique_identities(
    fingerprints: &[CheckFingerprint],
) -> Result<(), SourceBehaviorFingerprintPreparationError> {
    let mut identities = HashSet::new();
    for fingerprint in fingerprints {
        let reference = fingerprint
            .reference
            .as_deref()
            .expect("canonical fingerprints always carry a reference");
        if !identities.insert((fingerprint.kind.as_str(), reference)) {
            return Err(SourceBehaviorFingerprintPreparationError {
                kind: SourceBehaviorFingerprintPreparationErrorKind::DuplicateIdentity,
                component_kind: SOURCE_BEHAVIOR,
                component_reference: "duplicate_identity",
            });
        }
    }
    Ok(())
}

fn inconsistent(reference: &'static str) -> SourceBehaviorFingerprintPreparationError {
    SourceBehaviorFingerprintPreparationError {
        kind: SourceBehaviorFingerprintPreparationErrorKind::InconsistentCompilerOutcome,
        component_kind: SOURCE_BEHAVIOR,
        component_reference: reference,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_immutable_global_material_is_pinned() {
        let mut tail = Vec::new();
        push_tail(&mut tail).unwrap();
        assert_eq!(
            tail.iter()
                .map(|fingerprint| fingerprint.sha256.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "e02bd10379e8b2c45eeba9eb4f2c009ee0005bc335ebc095a963b323dee866ef",
                "87c21df84de6816bd31c901e0eacc2312eb87ecab4abecba966214ad09fb9f1c",
                "abc26b6b4b33c915142507eac77fbb317fa7d8a1b583ef40d53dec8edae84bef",
                "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
                "1a6562590ef19d1045d06c4055742d38288e9e6dcd71ccde5cee80f1d5a774eb",
                "069ca21296462f5f48b5831276c4983855de37a5c06b91b317ff7b11f3a853ed",
            ]
        );
    }

    #[test]
    fn productive_source_live_check_uses_canonical_preparation_without_activation_reprepare() {
        assert!(include_str!("source_live/mod.rs").contains("prepare_source_behavior_fingerprints"));
        assert!(!include_str!("source_live/activation.rs")
            .contains("prepare_source_behavior_fingerprints"));
    }

    #[test]
    fn serialization_failure_is_value_free_and_returns_no_partial_set() {
        struct FailingProjection;
        impl Serialize for FailingProjection {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("secret-value"))
            }
        }

        let mut fingerprints = Vec::new();
        let error = push_component(
            &mut fingerprints,
            SOURCE_BEHAVIOR,
            "source_config",
            &FailingProjection,
        )
        .unwrap_err();
        assert!(fingerprints.is_empty());
        assert_eq!(
            error.kind,
            SourceBehaviorFingerprintPreparationErrorKind::Serialization
        );
        assert_eq!(error.component_reference, "source_config");
        assert!(!error.to_string().contains("secret-value"));
    }

    #[test]
    fn duplicate_identity_is_rejected_without_returning_a_partial_set() {
        let fingerprints = vec![
            CheckFingerprint::with_reference(SOURCE_BEHAVIOR, "same", "a"),
            CheckFingerprint::with_reference(SOURCE_BEHAVIOR, "same", "b"),
        ];
        let error = ensure_unique_identities(&fingerprints).unwrap_err();
        assert_eq!(
            error.kind,
            SourceBehaviorFingerprintPreparationErrorKind::DuplicateIdentity
        );
        assert_eq!(error.component_reference, "duplicate_identity");
    }
}
