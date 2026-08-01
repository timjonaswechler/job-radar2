use source_profile_dsl::definition::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use sources::installed::{Snapshot, SourceStatus};

use super::SourceExecutionError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SourceSelectionOptions {
    pub allow_draft_sources: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SelectedSearchRunSource<'a> {
    Resolved(&'a source_profile_dsl::definition::CompiledSource),
    Missing {
        source_key: String,
        error: SourceExecutionError,
    },
    Failed {
        source_key: String,
        source_name: String,
        error: SourceExecutionError,
    },
    Skipped {
        source_key: String,
        source_name: String,
        diagnostics: Diagnostics,
        summary: String,
    },
}

pub(super) fn resolve_selected_sources_with_options<'a>(
    snapshot: &'a Snapshot,
    source_keys: &[String],
    options: SourceSelectionOptions,
) -> Vec<SelectedSearchRunSource<'a>> {
    source_keys
        .iter()
        .map(|source_key| {
            let Some(source) = snapshot.source(source_key) else {
                let diagnostics = vec![source_validation_diagnostic(
                    "source_not_found",
                    format!(
                        "Selected Source `{source_key}` was not found in the installed Snapshot"
                    ),
                    "",
                    serde_json::json!({"sourceKey": source_key}),
                )];
                return SelectedSearchRunSource::Missing {
                    source_key: source_key.clone(),
                    error: SourceExecutionError::FailedWithDiagnostics {
                        message: diagnostic_summary(&diagnostics),
                        diagnostics,
                    },
                };
            };
            let document = source.document();
            let validation = source.validation();
            let allow_draft = options.allow_draft_sources && document.status == SourceStatus::Draft;
            if document.status != SourceStatus::Active && !allow_draft {
                let status =
                    serde_json::to_value(document.status).expect("Source Status serializes");
                let diagnostics = vec![source_validation_diagnostic(
                    "source_not_active",
                    format!(
                        "Selected Source `{}` has status `{}` and was skipped",
                        document.key,
                        status.as_str().unwrap_or("unknown")
                    ),
                    "/status",
                    serde_json::json!({"sourceKey": document.key, "status": status}),
                )];
                return SelectedSearchRunSource::Skipped {
                    source_key: document.key.clone(),
                    source_name: document.name.clone(),
                    summary: diagnostic_summary(&diagnostics),
                    diagnostics,
                };
            }
            if !(validation.can_execute || allow_draft && validation.can_compile) {
                let diagnostics = validation.diagnostics.clone();
                return SelectedSearchRunSource::Failed {
                    source_key: document.key.clone(),
                    source_name: document.name.clone(),
                    error: SourceExecutionError::FailedWithDiagnostics {
                        message: diagnostic_summary(&diagnostics),
                        diagnostics,
                    },
                };
            }
            match source.compiled() {
                Some(compiled) => SelectedSearchRunSource::Resolved(compiled),
                None => {
                    let diagnostics = source.preparation_diagnostics().to_vec();
                    SelectedSearchRunSource::Failed {
                        source_key: document.key.clone(),
                        source_name: document.name.clone(),
                        error: SourceExecutionError::FailedWithDiagnostics {
                            message: diagnostic_summary(&diagnostics),
                            diagnostics,
                        },
                    }
                }
            }
        })
        .collect()
}

fn diagnostic_summary(diagnostics: &Diagnostics) -> String {
    diagnostics
        .iter()
        .find(|item| item.severity == DiagnosticSeverity::Error)
        .or_else(|| diagnostics.first())
        .map(|item| item.message.clone())
        .unwrap_or_else(|| "Source could not be executed".into())
}
fn source_validation_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::SourceValidation,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: None,
        details: Some(details),
    }
}
