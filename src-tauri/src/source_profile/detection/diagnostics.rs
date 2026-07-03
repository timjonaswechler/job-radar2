use serde_json::Value;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};

use super::{
    SourceProposalDetectionResult, SourceProposalDetectionStatus, UnsupportedSourceProfile,
};

pub(super) fn detection_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    probe_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: probe_key.map(ToString::to_string),
        details: Some(details),
    }
}

pub(super) fn detection_warning(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    probe_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Warning,
        path: path.into(),
        strategy_key: probe_key.map(ToString::to_string),
        details: Some(details),
    }
}

pub(super) fn failed_result(diagnostics: Diagnostics) -> SourceProposalDetectionResult {
    failed_result_with_unsupported(diagnostics, Vec::new())
}

pub(super) fn failed_result_with_unsupported(
    diagnostics: Diagnostics,
    unsupported_profiles: Vec<UnsupportedSourceProfile>,
) -> SourceProposalDetectionResult {
    SourceProposalDetectionResult {
        status: SourceProposalDetectionStatus::Failed,
        proposal: None,
        proposals: Vec::new(),
        unsupported_profiles,
        diagnostics,
    }
}
