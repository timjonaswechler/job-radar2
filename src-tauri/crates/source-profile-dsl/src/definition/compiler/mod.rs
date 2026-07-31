//! Profile Compiler for resolving one authoritative Source into a closed,
//! immutable compiled result.
//!
//! [`compile_source`] accepts the Source directly and uses an immutable
//! [`SourceProfileLookup`] only for Source Profile lookup; a registry Source with
//! the same key is never consulted. Source lifecycle admission belongs to callers. Profile
//! access materializes and completely validates an inspectable Effective Source
//! Profile before Source Config validation, Access Path resolution, and plan
//! construction. Source-owned access remains a distinct branch. Rejection
//! exposes only ordered diagnostics, never a partial profile, path, or plan.
//!
//! The compiler performs no network, browser, parser, selector, extractor,
//! transform, pagination, or runtime execution. Runtime receives only the typed
//! [`SourceExecutionPlan`].

use serde::{Deserialize, Serialize};

use crate::definition::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
mod boundedness;
mod capabilities;
mod keys;
mod provenance;
mod resolution;
mod source_config;
mod specialization;
mod support;
mod templates;
mod values;

pub use crate::definition::primitives::fetch::http::http_fetch_security_behavior as forbidden_request_key_behavior;
pub use boundedness::MAX_FALLBACK_STRATEGIES;
pub use provenance::{
    CompiledSourceProvenance, ProvenanceEntry, ProvenanceOrigin, ProvenancePath,
    ProvenancePathSegment,
};

use crate::definition::documents::{DetailStep, DiscoveryStep, JsonSchemaObject};
use crate::definition::execution_plan::SourceExecutionPlan;
use crate::definition::profile::SourceProfileDocument;
use crate::detection::{compile_detection_plan, CompiledDetectionPlan};
use crate::execution::occurrence::{DetailField, DetailFieldCapabilities};
use crate::source::documents::{SelectedAccessPath, SourceConfig, SourceDocument};

/// Immutable Source Profile lookup used by compilation.
pub trait SourceProfileLookup {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument>;
}

/// Narrow immutable compiler input, independent of registry loading and lifecycle state.
#[derive(Clone, Copy, Debug)]
pub struct ProfileCompilerInput<'a> {
    profiles: &'a [SourceProfileDocument],
}

impl<'a> ProfileCompilerInput<'a> {
    pub const fn new(profiles: &'a [SourceProfileDocument]) -> Self {
        Self { profiles }
    }
}

impl SourceProfileLookup for ProfileCompilerInput<'_> {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles.iter().find(|profile| profile.key == key)
    }
}

/// The complete profile after Direct Source Specialization has been applied
/// and the whole profile has been validated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EffectiveSourceProfile {
    pub document: SourceProfileDocument,
}

/// The complete inline Access Path owned by one Source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SourceOwnedAccessPath {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub source_config_schema: Option<JsonSchemaObject>,
    pub discovery: DiscoveryStep,
    pub detail: Option<DetailStep>,
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum CompiledSourceAccess {
    Profile {
        effective_profile: EffectiveSourceProfile,
    },
    SourceOwned {
        access_path: SourceOwnedAccessPath,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRuntimeBinding {
    Name,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRuntimeBindingDependencies {
    pub bindings: Vec<SourceRuntimeBinding>,
}

impl SourceRuntimeBindingDependencies {
    pub(super) fn insert(&mut self, binding: SourceRuntimeBinding) {
        if !self.bindings.contains(&binding) {
            self.bindings.push(binding);
            self.bindings.sort();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSource {
    pub(crate) access: CompiledSourceAccess,
    pub(crate) execution_plan: SourceExecutionPlan,
    pub(crate) provenance: CompiledSourceProvenance,
    pub(crate) runtime_binding_dependencies: SourceRuntimeBindingDependencies,
    /// Validated runtime bindings owned by this authoritative compiled Source.
    /// Values stay out of generic serialization because Source Config may contain
    /// sensitive access material.
    #[serde(skip)]
    pub(crate) source_config: SourceConfig,
}

impl CompiledSource {
    pub fn source_key(&self) -> &str {
        &self.execution_plan.source.key
    }

    pub fn source_name(&self) -> &str {
        &self.execution_plan.source.name
    }

    pub fn effective_profile(&self) -> Option<&SourceProfileDocument> {
        match &self.access {
            CompiledSourceAccess::Profile { effective_profile } => {
                Some(&effective_profile.document)
            }
            CompiledSourceAccess::SourceOwned { .. } => None,
        }
    }

    pub fn source_owned_access_path(&self) -> Option<&SourceOwnedAccessPath> {
        match &self.access {
            CompiledSourceAccess::Profile { .. } => None,
            CompiledSourceAccess::SourceOwned { access_path } => Some(access_path),
        }
    }

    pub fn provenance(&self) -> &CompiledSourceProvenance {
        &self.provenance
    }

    pub fn runtime_binding_dependencies(&self) -> &SourceRuntimeBindingDependencies {
        &self.runtime_binding_dependencies
    }

    pub fn selected_profile_access_path(&self) -> Option<(&str, &str)> {
        match &self.execution_plan.selected_access_path {
            crate::definition::execution_plan::ExecutionPlanAccessPath::ProfileAccessPath {
                profile_key,
                path_key,
                ..
            } => Some((profile_key, path_key)),
            crate::definition::execution_plan::ExecutionPlanAccessPath::SourceOwnedAccessPath {
                ..
            } => None,
        }
    }

    pub fn selected_source_owned_access_path(&self) -> Option<&str> {
        match &self.execution_plan.selected_access_path {
            crate::definition::execution_plan::ExecutionPlanAccessPath::ProfileAccessPath {
                ..
            } => None,
            crate::definition::execution_plan::ExecutionPlanAccessPath::SourceOwnedAccessPath {
                key,
                ..
            } => Some(key),
        }
    }

    /// Canonical finite Detail fields that at least one compiled Strategy can produce.
    ///
    /// Capabilities are derived only from typed executable output expressions. They
    /// are not authored promises and therefore cannot contain dynamic field names.
    pub fn detail_capabilities(&self) -> DetailFieldCapabilities {
        let Some(detail) = &self.execution_plan.detail else {
            return DetailFieldCapabilities::default();
        };
        DetailFieldCapabilities::new(detail.strategies.iter().flat_map(|strategy| {
            let fields = &strategy.extract.fields;
            [
                fields.title.as_ref().map(|_| DetailField::Title),
                fields.company.as_ref().map(|_| DetailField::Company),
                fields.locations.as_ref().map(|_| DetailField::Locations),
                fields
                    .description_text
                    .as_ref()
                    .map(|_| DetailField::DescriptionText),
            ]
            .into_iter()
            .flatten()
        }))
    }
}

/// Closed result of compiling one authoritative Source.
///
/// A rejection cannot expose an Effective Source Profile, selected Access Path,
/// or Execution Plan. Source lifecycle admission intentionally belongs to callers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum CompileSourceOutcome {
    Compiled {
        source: CompiledSource,
        diagnostics: Diagnostics,
    },
    Rejected {
        diagnostics: Diagnostics,
    },
}

pub fn compile_source(
    source: &SourceDocument,
    profiles: &impl SourceProfileLookup,
) -> CompileSourceOutcome {
    compile_source_with_profile_validation(source, profiles, true)
}

/// Compiles against a Profile set whose owner has already validated and
/// prepared Profile Detection material.
pub fn compile_source_with_admitted_profiles(
    source: &SourceDocument,
    profiles: &impl SourceProfileLookup,
) -> CompileSourceOutcome {
    compile_source_with_profile_validation(source, profiles, false)
}

fn compile_source_with_profile_validation(
    source: &SourceDocument,
    profiles: &impl SourceProfileLookup,
    validate_detection: bool,
) -> CompileSourceOutcome {
    let mut diagnostics = Vec::new();
    let compiled = build_compiled_source(source, profiles, validate_detection, &mut diagnostics);

    if has_error_diagnostics(&diagnostics) {
        return CompileSourceOutcome::Rejected { diagnostics };
    }

    match compiled {
        Some(source) => CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        },
        None => {
            diagnostics.push(compiler_error(
                "compiled_source_invariant_violation",
                "Compilation produced neither an error Diagnostic nor a compiled Source",
                "",
                serde_json::json!({ "sourceKey": source.key }),
            ));
            CompileSourceOutcome::Rejected { diagnostics }
        }
    }
}

fn build_compiled_source(
    source: &SourceDocument,
    profiles: &impl SourceProfileLookup,
    validate_detection: bool,
    diagnostics: &mut Diagnostics,
) -> Option<CompiledSource> {
    match &source.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            let Some(base_profile) = profiles.profile(profile_key) else {
                diagnostics.push(compiler_error(
                    "source_profile_not_found",
                    format!(
                        "Source `{}` references missing Source Profile `{profile_key}`",
                        source.key
                    ),
                    "/selectedAccessPath/profileKey",
                    serde_json::json!({ "sourceKey": source.key, "profileKey": profile_key }),
                ));
                return None;
            };
            let (effective_profile, recorded_provenance) =
                specialization::specialize_profile_with_provenance(
                    base_profile,
                    source.access_paths.as_deref(),
                    diagnostics,
                )?;
            let resolved = resolution::compile_materialized_profile_access_path(
                source,
                &effective_profile,
                profile_key,
                path_key,
                validate_detection,
                diagnostics,
            )?;
            let execution_plan = resolved.execution_plan;
            validate_provenance(&recorded_provenance, diagnostics)?;
            let provenance = recorded_provenance.value;
            Some(CompiledSource {
                access: CompiledSourceAccess::Profile {
                    effective_profile: EffectiveSourceProfile {
                        document: effective_profile,
                    },
                },
                execution_plan,
                provenance,
                runtime_binding_dependencies: resolved.runtime_binding_dependencies,
                source_config: source.source_config.clone(),
            })
        }
        SelectedAccessPath::SourceOwnedAccessPath {
            key,
            name,
            description,
            source_config_schema,
            discovery,
            detail,
            diagnostics: access_diagnostics,
        } => {
            let resolved = resolution::compile_source_owned_access_path(source, diagnostics)?;
            let execution_plan = resolved.execution_plan;
            let recorded_provenance =
                provenance::source_owned_provenance(&source.selected_access_path);
            validate_provenance(&recorded_provenance, diagnostics)?;
            let provenance = recorded_provenance.value;
            Some(CompiledSource {
                access: CompiledSourceAccess::SourceOwned {
                    access_path: SourceOwnedAccessPath {
                        key: key.clone(),
                        name: name.clone(),
                        description: description.clone(),
                        source_config_schema: source_config_schema.clone(),
                        discovery: discovery.clone(),
                        detail: detail.clone(),
                        diagnostics: access_diagnostics.clone(),
                    },
                },
                execution_plan,
                provenance,
                runtime_binding_dependencies: resolved.runtime_binding_dependencies,
                source_config: source.source_config.clone(),
            })
        }
    }
}

fn validate_provenance(
    provenance: &provenance::RecordedProvenance,
    diagnostics: &mut Diagnostics,
) -> Option<()> {
    if let Err((reason, path)) = provenance::validate_unique_complete(provenance) {
        diagnostics.push(compiler_error(
            "compiler/compiled_provenance_invariant_violation",
            "Compiled provenance did not uniquely cover every execution-relevant terminal",
            "",
            serde_json::json!({ "reason": reason, "provenancePath": path }),
        ));
        return None;
    }
    Some(())
}

pub fn validate_source_profile_document(profile: &SourceProfileDocument) -> Diagnostics {
    let mut diagnostics = Vec::new();
    resolution::validate_source_profile_document(profile, &mut diagnostics);
    diagnostics
}

/// Validates one authored Source Profile and prepares its optional Detection
/// material in the same pass.
pub fn prepare_source_profile_document(
    profile: &SourceProfileDocument,
) -> Result<Option<CompiledDetectionPlan>, Diagnostics> {
    let mut diagnostics = Vec::new();
    resolution::validate_source_profile_document_without_detection(profile, &mut diagnostics);
    let detection = if profile.detection.is_some() {
        match compile_detection_plan(profile) {
            Ok(plan) => Some(plan),
            Err(detection_diagnostics) => {
                diagnostics.extend(detection_diagnostics);
                None
            }
        }
    } else {
        None
    };
    if has_error_diagnostics(&diagnostics) {
        Err(diagnostics)
    } else {
        Ok(detection)
    }
}

pub(super) fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

pub(super) fn compiler_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Compiler,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(details),
    }
}

#[cfg(test)]
mod provenance_invariant_tests {
    use super::*;

    #[test]
    fn recorder_faults_become_the_closed_invariant_diagnostic() {
        for reason in ["duplicate_path", "missing_path"] {
            let mut diagnostics = Vec::new();
            assert!(
                validate_provenance(&provenance::invariant_fault(reason), &mut diagnostics,)
                    .is_none()
            );
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                diagnostics[0].code,
                "compiler/compiled_provenance_invariant_violation"
            );
            assert_eq!(diagnostics[0].path, "");
            assert_eq!(diagnostics[0].details.as_ref().unwrap()["reason"], reason);
            assert!(diagnostics[0].details.as_ref().unwrap()["provenancePath"].is_object());
        }
    }
}
