//! Profile Compiler for resolving concrete Sources into typed Execution Plans.
//! The compiler performs semantic validation and intentionally performs no
//! network, browser, parser, selector, extractor, transform, pagination, or
//! runtime execution. It collects semantic, boundedness, and security
//! diagnostics before producing executable plans.

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
mod support;
mod templates;

use crate::profile_dsl::execution_plan::SourceExecutionPlan;
use crate::source::documents::{SourceDocument, SourceStatus};
use crate::source_profile::documents::SourceProfileDocument;

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

    result.execution_plan =
        resolution::compile_selected_access_path(snapshot, source, &mut result.diagnostics);

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
