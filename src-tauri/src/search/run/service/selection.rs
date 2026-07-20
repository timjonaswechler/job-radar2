use crate::{
    profile_dsl::{
        compiler::{compile_source, CompileSourceOutcome},
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    },
    source::documents::SourceStatus,
    source_profile::registry::SourceProfileRegistrySnapshot,
};

use super::{super::SourceExecutionSource, SourceExecutionError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SourceSelectionOptions {
    pub allow_draft_sources: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SelectedSearchRunSource {
    Resolved(Box<SourceExecutionSource>),
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

pub(super) fn resolve_selected_sources_with_options(
    snapshot: &SourceProfileRegistrySnapshot,
    source_keys: &[String],
    options: SourceSelectionOptions,
) -> Vec<SelectedSearchRunSource> {
    source_keys
        .iter()
        .map(|source_key| {
            let Some(source) = snapshot.source(source_key) else {
                let diagnostics = vec![source_validation_diagnostic(
                    "source_not_found",
                    format!("Selected Source `{source_key}` was not found in the Source Profile registry snapshot"),
                    "",
                    serde_json::json!({ "sourceKey": source_key }),
                )];
                return SelectedSearchRunSource::Missing {
                    source_key: source_key.clone(),
                    error: SourceExecutionError::FailedWithDiagnostics {
                        message: diagnostic_summary(&diagnostics),
                        diagnostics,
                    },
                };
            };

            let allow_draft_source = options.allow_draft_sources
                && source.document.status == SourceStatus::Draft;
            if source.document.status != SourceStatus::Active && !allow_draft_source {
                let status = serde_json::to_value(source.document.status)
                    .expect("SourceStatus should serialize to a stable diagnostic value");
                let diagnostics = vec![source_validation_diagnostic(
                    "source_not_active",
                    format!(
                        "Selected Source `{}` has status `{}` and was skipped",
                        source.document.key,
                        status.as_str().unwrap_or("unknown")
                    ),
                    "/status",
                    serde_json::json!({
                        "sourceKey": source.document.key,
                        "status": status,
                    }),
                )];
                return SelectedSearchRunSource::Skipped {
                    source_key: source.document.key.clone(),
                    source_name: source.document.name.clone(),
                    summary: diagnostic_summary(&diagnostics),
                    diagnostics,
                };
            }

            if !(source.validation_state.can_execute || allow_draft_source && source.validation_state.can_compile) {
                let diagnostics = source.validation_state.diagnostics.clone();
                return SelectedSearchRunSource::Failed {
                    source_key: source.document.key.clone(),
                    source_name: source.document.name.clone(),
                    error: SourceExecutionError::FailedWithDiagnostics {
                        message: diagnostic_summary(&diagnostics),
                        diagnostics,
                    },
                };
            }

            match compile_source(&source.document, snapshot) {
                CompileSourceOutcome::Compiled {
                    source: compiled,
                    diagnostics,
                } if !has_error_diagnostics(&diagnostics) => {
                    SelectedSearchRunSource::Resolved(Box::new(compiled.execution_plan.into()))
                }
                CompileSourceOutcome::Compiled { diagnostics, .. }
                | CompileSourceOutcome::Rejected { diagnostics } => {
                    SelectedSearchRunSource::Failed {
                        source_key: source.document.key.clone(),
                        source_name: source.document.name.clone(),
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

fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn diagnostic_summary(diagnostics: &Diagnostics) -> String {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .or_else(|| diagnostics.first())
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "Source could not be executed".to_string())
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
