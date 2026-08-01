#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use source_engine::definition::{
    compile_source_with_admitted_profiles, CompileSourceOutcome, Diagnostic, DiagnosticCategory,
    DiagnosticSeverity, Diagnostics,
};

#[cfg(test)]
static PREPARATION_CALLS: AtomicUsize = AtomicUsize::new(0);

use super::{
    limits::MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT,
    snapshot::{Profiles, ResolvedBehaviorView, ValidationState, ValidationStateKind},
    sources::{SourceDocument, SourceStatus},
};

pub(super) fn prepare(
    document: &SourceDocument,
    profiles: &Profiles,
) -> (
    CompileSourceOutcome,
    ValidationState,
    Option<ResolvedBehaviorView>,
) {
    // This is deliberately the sole productive Profile Compiler caller outside
    // the engine crate. Every operation-local Snapshot retains this exact result.
    #[cfg(test)]
    PREPARATION_CALLS.fetch_add(1, Ordering::Relaxed);
    let mut outcome =
        compile_source_with_admitted_profiles(&document.behavior_input(), &profiles.lookup());
    bound_outcome_diagnostics(&mut outcome, &document.key);
    let validation = validation_state(document, &outcome);
    let resolved = resolved_view(&outcome);
    (outcome, validation, resolved)
}

fn validation_state(document: &SourceDocument, outcome: &CompileSourceOutcome) -> ValidationState {
    let (can_compile, mut diagnostics) = match outcome {
        CompileSourceOutcome::Compiled { diagnostics, .. } if !has_errors(diagnostics) => {
            (true, diagnostics.clone())
        }
        CompileSourceOutcome::Compiled { diagnostics, .. }
        | CompileSourceOutcome::Rejected { diagnostics } => (false, diagnostics.clone()),
    };
    if !can_compile {
        diagnostics.push(Diagnostic {
            category: DiagnosticCategory::SourceValidation,
            code: "source_validation_failed".into(),
            message: format!("Source `{}` cannot currently compile into an Execution Plan", document.key),
            severity: DiagnosticSeverity::Error,
            path: "".into(),
            strategy_key: None,
            details: Some(serde_json::json!({
                "sourceKey": document.key,
                "diagnosticCodes": diagnostics.iter().map(|item| item.code.clone()).collect::<Vec<_>>()
            })),
        });
    }
    bound_diagnostics(&mut diagnostics, &document.key);
    ValidationState {
        source_key: document.key.clone(),
        state: if can_compile {
            ValidationStateKind::Valid
        } else {
            ValidationStateKind::Invalid
        },
        can_compile,
        can_execute: document.status == SourceStatus::Active && can_compile,
        diagnostics,
    }
}

fn bound_outcome_diagnostics(outcome: &mut CompileSourceOutcome, key: &str) {
    let diagnostics = match outcome {
        CompileSourceOutcome::Compiled { diagnostics, .. }
        | CompileSourceOutcome::Rejected { diagnostics } => diagnostics,
    };
    bound_diagnostics(diagnostics, key);
}

fn bound_diagnostics(diagnostics: &mut Diagnostics, key: &str) {
    if diagnostics.len() <= MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT {
        return;
    }
    diagnostics.truncate(MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT);
    diagnostics[MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT - 1] = Diagnostic {
        category: DiagnosticCategory::Registry,
        code: "source_diagnostics_truncated".into(),
        message: "Source Diagnostics were truncated".into(),
        severity: DiagnosticSeverity::Error,
        path: "".into(),
        strategy_key: None,
        details: Some(
            serde_json::json!({"sourceKey": key, "maximum": MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT}),
        ),
    };
}

fn resolved_view(outcome: &CompileSourceOutcome) -> Option<ResolvedBehaviorView> {
    let CompileSourceOutcome::Compiled { source, .. } = outcome else {
        return None;
    };
    if let Some(profile) = source.materialized_profile() {
        let (_, path_key) = source.selected_profile_access_path()?;
        let path = profile
            .access_paths
            .iter()
            .find(|path| path.key == path_key)?;
        return Some(ResolvedBehaviorView {
            access_path_name: path.name.clone(),
            profile_source_config_schema: profile.source_config_schema.clone(),
            access_path_source_config_schema: path.source_config_schema.clone(),
            support: Some(profile.support.clone()),
            capabilities: capabilities(path.detail.is_some()),
        });
    }
    let path = source.source_owned_access_path()?;
    Some(ResolvedBehaviorView {
        access_path_name: path.name.clone(),
        profile_source_config_schema: None,
        access_path_source_config_schema: path.source_config_schema.clone(),
        support: None,
        capabilities: capabilities(path.detail.is_some()),
    })
}

fn capabilities(detail: bool) -> Vec<String> {
    let mut result = vec!["discovery".to_string()];
    if detail {
        result.push("detail".to_string());
    }
    result
}

#[cfg(test)]
pub(super) fn reset_preparation_calls() {
    PREPARATION_CALLS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(super) fn preparation_calls() -> usize {
    PREPARATION_CALLS.load(Ordering::Relaxed)
}

fn has_errors(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|item| item.severity == DiagnosticSeverity::Error)
}
