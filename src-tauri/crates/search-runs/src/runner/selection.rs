use source_engine::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics};
use sources::installed::{Snapshot, SourceStatus};

use super::{errors::SourceFailure, SourceAdmission};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Selected<'a> {
    Resolved(&'a source_engine::definition::CompiledSource),
    Missing {
        source_key: String,
        error: SourceFailure,
    },
    Failed {
        source_key: String,
        source_name: String,
        error: SourceFailure,
    },
    Skipped {
        source_key: String,
        source_name: String,
        diagnostics: Diagnostics,
        summary: String,
    },
}

pub(super) fn resolve_selected_sources<'a>(
    snapshot: &'a Snapshot,
    source_keys: &[String],
    options: SourceAdmission,
) -> Vec<Selected<'a>> {
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
                return Selected::Missing {
                    source_key: source_key.clone(),
                    error: SourceFailure::new(diagnostic_summary(&diagnostics), diagnostics),
                };
            };
            let document = source.document();
            let validation = source.validation();
            let allow_draft = options == SourceAdmission::DevelopmentSmokeAllowDraft
                && document.status == SourceStatus::Draft;
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
                return Selected::Skipped {
                    source_key: document.key.clone(),
                    source_name: document.name.clone(),
                    summary: diagnostic_summary(&diagnostics),
                    diagnostics,
                };
            }
            if !(validation.can_execute || allow_draft && validation.can_compile) {
                let diagnostics = validation.diagnostics.clone();
                return Selected::Failed {
                    source_key: document.key.clone(),
                    source_name: document.name.clone(),
                    error: SourceFailure::new(diagnostic_summary(&diagnostics), diagnostics),
                };
            }
            match source.compiled() {
                Some(compiled) => Selected::Resolved(compiled),
                None => {
                    let diagnostics = source.preparation_diagnostics().to_vec();
                    Selected::Failed {
                        source_key: document.key.clone(),
                        source_name: document.name.clone(),
                        error: SourceFailure::new(diagnostic_summary(&diagnostics), diagnostics),
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
