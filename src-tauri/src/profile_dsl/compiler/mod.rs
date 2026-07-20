//! Profile Compiler for resolving one authoritative Source into a closed,
//! immutable compiled result.
//!
//! [`compile_source`] accepts the Source directly and uses an immutable registry
//! snapshot only for Source Profile lookup; a registry Source with the same key
//! is never consulted. Source lifecycle admission belongs to callers. Profile
//! access materializes and completely validates an inspectable Effective Source
//! Profile before Source Config validation, Access Path resolution, and plan
//! construction. Source-owned access remains a distinct branch. Rejection
//! exposes only ordered diagnostics, never a partial profile, path, or plan.
//!
//! The compiler performs no network, browser, parser, selector, extractor,
//! transform, pagination, or runtime execution. Runtime receives only the typed
//! [`SourceExecutionPlan`].

use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
mod boundedness;
mod capabilities;
mod keys;
mod overrides;
mod resolution;
mod security;
mod source_config;
mod specialization;
mod support;
mod templates;

use crate::profile_dsl::documents::JsonSchemaObject;
use crate::profile_dsl::execution_plan::SourceExecutionPlan;
use crate::profile_dsl::policy::{
    PolicyPostingDetailStep, PolicyPostingDiscoveryStep, PolicySelectedAccessPath,
    PolicySourceDocument, PolicySourceExecutionPlan, PolicySourceProfileDocument,
    PolicySourceProfileRegistrySnapshot,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
use crate::source_profile::documents::SourceProfileDocument;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ProfileCompilerSnapshot {
    pub profiles: Vec<SourceProfileDocument>,
    pub sources: Vec<SourceDocument>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompileSourceExecutionPlanResult {
    pub source_key: String,
    pub execution_plan: Option<SourceExecutionPlan>,
    pub diagnostics: Diagnostics,
}

/// The complete profile after direct Source fragments or legacy Source Overrides
/// have been applied and the whole profile has been validated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EffectiveSourceProfile {
    pub document: PolicySourceProfileDocument,
}

/// The complete inline Access Path owned by one Source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SourceOwnedAccessPath {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub source_config_schema: Option<JsonSchemaObject>,
    pub posting_discovery: PolicyPostingDiscoveryStep,
    pub posting_detail: Option<PolicyPostingDetailStep>,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompiledSource {
    pub access: CompiledSourceAccess,
    pub execution_plan: PolicySourceExecutionPlan,
}

/// Closed result of compiling one authoritative Source.
///
/// A rejection cannot expose an Effective Source Profile, selected Access Path,
/// or Execution Plan. Source lifecycle admission intentionally belongs to callers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    source: &PolicySourceDocument,
    registry: &PolicySourceProfileRegistrySnapshot,
) -> CompileSourceOutcome {
    let mut diagnostics = Vec::new();
    let compiled = compile_policy_source(source, registry, &mut diagnostics);

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

fn compile_policy_source(
    source: &PolicySourceDocument,
    registry: &PolicySourceProfileRegistrySnapshot,
    diagnostics: &mut Diagnostics,
) -> Option<CompiledSource> {
    match &source.selected_access_path {
        PolicySelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            let Some(base_profile) = registry
                .profiles
                .iter()
                .find(|profile| profile.key == *profile_key)
            else {
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
            let effective_profile = specialization::specialize_policy_profile(
                base_profile,
                source.access_paths.as_deref(),
                diagnostics,
            )?;
            let legacy_source = legacy_source(source);
            let legacy_registry = legacy_registry(effective_profile.legacy());
            let plan = resolution::compile_legacy_source_execution_plan(
                &legacy_source,
                &legacy_registry,
                diagnostics,
            )?;
            let selected_path = effective_profile
                .access_paths
                .iter()
                .find(|path| path.key == *path_key)
                .expect("successful plan compilation must resolve the selected final Access Path");
            let execution_plan = PolicySourceExecutionPlan::from_legacy(
                plan,
                selected_path.posting_discovery.policy,
                selected_path
                    .posting_detail
                    .as_ref()
                    .map(|step| step.policy),
            );
            Some(CompiledSource {
                access: CompiledSourceAccess::Profile {
                    effective_profile: EffectiveSourceProfile {
                        document: effective_profile,
                    },
                },
                execution_plan,
            })
        }
        PolicySelectedAccessPath::SourceOwnedAccessPath {
            key,
            name,
            description,
            source_config_schema,
            posting_discovery,
            posting_detail,
            diagnostics: access_diagnostics,
        } => {
            let legacy_source = legacy_source(source);
            let plan = resolution::compile_legacy_source_execution_plan(
                &legacy_source,
                &SourceProfileRegistrySnapshot::default(),
                diagnostics,
            )?;
            let execution_plan = PolicySourceExecutionPlan::from_legacy(
                plan,
                posting_discovery.policy,
                posting_detail.as_ref().map(|step| step.policy),
            );
            Some(CompiledSource {
                access: CompiledSourceAccess::SourceOwned {
                    access_path: SourceOwnedAccessPath {
                        key: key.clone(),
                        name: name.clone(),
                        description: description.clone(),
                        source_config_schema: source_config_schema.clone(),
                        posting_discovery: posting_discovery.clone(),
                        posting_detail: posting_detail.clone(),
                        diagnostics: access_diagnostics.clone(),
                    },
                },
                execution_plan,
            })
        }
    }
}

fn legacy_source(source: &PolicySourceDocument) -> SourceDocument {
    let selected_access_path = match &source.selected_access_path {
        PolicySelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => SelectedAccessPath::ProfileAccessPath {
            profile_key: profile_key.clone(),
            path_key: path_key.clone(),
        },
        PolicySelectedAccessPath::SourceOwnedAccessPath {
            key,
            name,
            description,
            source_config_schema,
            posting_discovery,
            posting_detail,
            diagnostics,
        } => SelectedAccessPath::SourceOwnedAccessPath {
            key: key.clone(),
            name: name.clone(),
            description: description.clone(),
            source_config_schema: source_config_schema.clone(),
            posting_discovery: posting_discovery.legacy(),
            posting_detail: posting_detail.as_ref().map(PolicyPostingDetailStep::legacy),
            diagnostics: diagnostics.clone(),
        },
    };
    SourceDocument {
        schema_version: source.schema_version,
        key: source.key.clone(),
        name: source.name.clone(),
        status: source.status,
        source_config: source.source_config.clone(),
        selected_access_path,
        access_paths: None,
        source_overrides: None,
        source_support: source.source_support.clone(),
        diagnostics: source.diagnostics.clone(),
    }
}

fn legacy_registry(profile: SourceProfileDocument) -> SourceProfileRegistrySnapshot {
    SourceProfileRegistrySnapshot {
        profiles: vec![crate::source_profile::registry::RegistrySourceProfile {
            origin: "dormant_policy_compiler".to_string(),
            path: String::new(),
            document: profile,
        }],
        sources: Vec::new(),
        diagnostics: Vec::new(),
    }
}

pub(crate) fn validate_source_profile_document(profile: &SourceProfileDocument) -> Diagnostics {
    let mut diagnostics = Vec::new();
    resolution::validate_source_profile_document(profile, &mut diagnostics);
    diagnostics
}

pub fn compile_source_execution_plan(
    snapshot: &ProfileCompilerSnapshot,
    source_key: &str,
) -> CompileSourceExecutionPlanResult {
    let mut result = CompileSourceExecutionPlanResult {
        source_key: source_key.to_string(),
        execution_plan: None,
        diagnostics: Vec::new(),
    };

    let Some(source) = snapshot
        .sources
        .iter()
        .find(|source| source.key == source_key)
    else {
        result.diagnostics.push(compiler_error(
            "source_not_found",
            format!("Source `{source_key}` was not found in the compiler snapshot"),
            "",
            serde_json::json!({ "sourceKey": source_key }),
        ));
        return result;
    };

    if source.status != SourceStatus::Active {
        let status = serde_json::to_value(source.status)
            .expect("SourceStatus should serialize to a stable diagnostic value");
        result.diagnostics.push(compiler_error(
            "source_not_executable",
            format!(
                "Source `{}` has status `{}` and cannot be compiled into an executable plan",
                source.key,
                status.as_str().unwrap_or("unknown")
            ),
            "/status",
            serde_json::json!({
                "sourceKey": source.key,
                "status": status,
            }),
        ));
        return result;
    }

    let registry = SourceProfileRegistrySnapshot {
        profiles: snapshot
            .profiles
            .iter()
            .cloned()
            .map(
                |document| crate::source_profile::registry::RegistrySourceProfile {
                    origin: "legacy_compiler_snapshot".to_string(),
                    path: String::new(),
                    document,
                },
            )
            .collect(),
        sources: Vec::new(),
        diagnostics: Vec::new(),
    };

    let mut diagnostics = Vec::new();
    result.execution_plan =
        resolution::compile_legacy_source_execution_plan(source, &registry, &mut diagnostics);
    result.diagnostics = diagnostics;

    result
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
