use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};

use super::super::SourceRunStatus;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum SourceExecutionError {
    Failed(String),
    Cancelled(String),
    FailedWithDiagnostics {
        message: String,
        diagnostics: Diagnostics,
    },
    CancelledWithDiagnostics {
        message: String,
        diagnostics: Diagnostics,
    },
}

impl SourceExecutionError {
    pub(super) fn status(&self) -> SourceRunStatus {
        match self {
            Self::Failed(_) | Self::FailedWithDiagnostics { .. } => SourceRunStatus::Failed,
            Self::Cancelled(_) | Self::CancelledWithDiagnostics { .. } => {
                SourceRunStatus::Cancelled
            }
        }
    }

    pub(super) fn message(&self) -> String {
        match self {
            Self::Failed(message)
            | Self::Cancelled(message)
            | Self::FailedWithDiagnostics { message, .. }
            | Self::CancelledWithDiagnostics { message, .. } => message.clone(),
        }
    }

    pub(super) fn diagnostics(&self) -> Diagnostics {
        match self {
            Self::FailedWithDiagnostics { diagnostics, .. }
            | Self::CancelledWithDiagnostics { diagnostics, .. } => diagnostics.clone(),
            Self::Failed(message) => vec![freeform_error_diagnostic(
                "source_execution_failed",
                message,
                DiagnosticCategory::Runtime,
            )],
            Self::Cancelled(message) => vec![freeform_error_diagnostic(
                "source_execution_cancelled",
                message,
                DiagnosticCategory::Runtime,
            )],
        }
    }
}

pub(super) fn freeform_error_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    category: DiagnosticCategory,
) -> Diagnostic {
    let message = message.into();
    Diagnostic {
        category,
        code: code.into(),
        message: message.clone(),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({ "message": message })),
    }
}
