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

use crate::profile_dsl::documents::{JsonSchemaObject, PostingDetailStep, PostingDiscoveryStep};
use crate::profile_dsl::execution_plan::SourceExecutionPlan;
use crate::source::documents::{SourceDocument, SourceStatus};
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
    pub document: SourceProfileDocument,
}

/// The complete inline Access Path owned by one Source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SourceOwnedAccessPath {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub source_config_schema: Option<JsonSchemaObject>,
    pub posting_discovery: PostingDiscoveryStep,
    pub posting_detail: Option<PostingDetailStep>,
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
    pub execution_plan: SourceExecutionPlan,
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
    source: &SourceDocument,
    registry: &SourceProfileRegistrySnapshot,
) -> CompileSourceOutcome {
    let mut diagnostics = Vec::new();
    let compiled = resolution::compile_authoritative_source(source, registry, &mut diagnostics);

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

    match compile_source(source, &registry) {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } => {
            result.execution_plan = Some(source.execution_plan);
            result.diagnostics = diagnostics;
        }
        CompileSourceOutcome::Rejected { diagnostics } => {
            result.diagnostics = diagnostics;
        }
    }

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
