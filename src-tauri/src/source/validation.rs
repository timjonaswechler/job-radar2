use serde::{Deserialize, Serialize};

use crate::profile_dsl::compiler::CompileSourceOutcome;
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::source::documents::{SourceDocument, SourceStatus};

/// Derived Source validation state. It is computed from the exact authoritative
/// compiler outcome and is never persisted as a Source lifecycle status.
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
    source: &SourceDocument,
    outcome: &CompileSourceOutcome,
) -> SourceValidationState {
    let (can_compile, mut diagnostics) = match outcome {
        CompileSourceOutcome::Compiled { diagnostics, .. }
            if !has_error_diagnostics(diagnostics) =>
        {
            (true, diagnostics.clone())
        }
        CompileSourceOutcome::Compiled { diagnostics, .. }
        | CompileSourceOutcome::Rejected { diagnostics } => (false, diagnostics.clone()),
    };
    let can_execute = source.status == SourceStatus::Active && can_compile;

    if !can_compile {
        diagnostics.push(Diagnostic {
            category: DiagnosticCategory::SourceValidation,
            code: "source_validation_failed".to_string(),
            message: format!(
                "Source `{}` cannot currently compile into an Execution Plan",
                source.key
            ),
            severity: DiagnosticSeverity::Error,
            path: "".to_string(),
            strategy_key: None,
            details: Some(serde_json::json!({
                "sourceKey": source.key,
                "diagnosticCodes": diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>()
            })),
        });
    }

    SourceValidationState {
        source_key: source.key.clone(),
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

fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
