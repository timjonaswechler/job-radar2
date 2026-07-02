use serde::{Deserialize, Serialize};

use crate::profile_dsl::compiler::{compile_source_execution_plan, ProfileCompilerSnapshot};
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::source::documents::SourceStatus;

/// Derived Source validation state. This is prepared for compiler/registry
/// integration and must not be persisted as `SourceStatus::Invalid`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceValidationState {
    pub source_key: String,
    pub state: ValidationStateKind,
    pub can_compile: bool,
    pub can_execute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStateKind {
    Unknown,
    Valid,
    Invalid,
}

pub fn derive_source_validation_state(
    snapshot: &ProfileCompilerSnapshot,
    source_key: &str,
) -> SourceValidationState {
    let Some(source) = snapshot
        .sources
        .iter()
        .find(|source| source.key == source_key)
    else {
        return SourceValidationState {
            source_key: source_key.to_string(),
            state: ValidationStateKind::Invalid,
            can_compile: false,
            can_execute: false,
            diagnostics: vec![Diagnostic {
                category: DiagnosticCategory::SourceValidation,
                code: "source_not_found".to_string(),
                message: format!(
                    "Source `{source_key}` was not found while deriving validation state"
                ),
                severity: DiagnosticSeverity::Error,
                path: "".to_string(),
                strategy_key: None,
                details: Some(serde_json::json!({ "sourceKey": source_key })),
            }],
        };
    };

    let mut validation_snapshot = snapshot.clone();
    if let Some(validation_source) = validation_snapshot
        .sources
        .iter_mut()
        .find(|candidate| candidate.key == source_key)
    {
        validation_source.status = SourceStatus::Active;
    }

    let compile_result = compile_source_execution_plan(&validation_snapshot, source_key);
    let has_errors = compile_result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
    let can_compile = compile_result.execution_plan.is_some() && !has_errors;
    let can_execute = source.status == SourceStatus::Active && can_compile;
    let mut diagnostics = compile_result.diagnostics;
    if !can_compile {
        diagnostics.push(Diagnostic {
            category: DiagnosticCategory::SourceValidation,
            code: "source_validation_failed".to_string(),
            message: format!(
                "Source `{source_key}` cannot currently compile into an Execution Plan"
            ),
            severity: DiagnosticSeverity::Error,
            path: "".to_string(),
            strategy_key: None,
            details: Some(serde_json::json!({
                "sourceKey": source_key,
                "diagnosticCodes": diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>()
            })),
        });
    }

    SourceValidationState {
        source_key: source_key.to_string(),
        state: if can_compile {
            ValidationStateKind::Valid
        } else {
            ValidationStateKind::Invalid
        },
        can_compile,
        can_execute,
        diagnostics,
    }
}
