use crate::{
    profile_dsl::{compiler::CompiledSource, diagnostics::Diagnostics},
    search::candidate_resolution::{SourceResolution, SourceResolutionError},
};

use super::super::{SearchRunStatus, SourceResolutionSummary, SourceRunResult, SourceRunStatus};
use super::SourceExecutionError;

fn source_identity(source: &CompiledSource) -> (&str, &str) {
    (
        &source.execution_plan.source.key,
        &source.execution_plan.source.name,
    )
}

pub(super) fn source_run_completed(
    source: &CompiledSource,
    resolution: &SourceResolution,
) -> SourceRunResult {
    let (source_key, source_name) = source_identity(source);
    SourceRunResult {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceRunStatus::Completed,
        resolution: Some(SourceResolutionSummary::from(resolution)),
        diagnostics: resolution.diagnostics.clone(),
        error: None,
    }
}

pub(super) fn source_run_resolution_failed(
    source: &CompiledSource,
    error: SourceResolutionError,
) -> SourceRunResult {
    let (source_key, source_name) = source_identity(source);
    match error {
        SourceResolutionError::Cancelled => {
            source_run_cancelled_for_source(source_key, source_name)
        }
        SourceResolutionError::Failed {
            failure,
            diagnostics,
        } => SourceRunResult {
            source_key: source_key.to_string(),
            source_name: source_name.to_string(),
            status: SourceRunStatus::Failed,
            resolution: None,
            diagnostics,
            error: Some(format!("Candidate Resolution failed: {failure:?}")),
        },
    }
}

pub(super) fn source_run_failed_for_key(
    source_key: &str,
    error: SourceExecutionError,
) -> SourceRunResult {
    source_run_failed_for_source(source_key, "", error)
}

pub(super) fn source_run_cancelled_for_key(source_key: &str) -> SourceRunResult {
    source_run_cancelled_for_source(source_key, "")
}

pub(super) fn source_run_cancelled_for_source(
    source_key: &str,
    source_name: &str,
) -> SourceRunResult {
    SourceRunResult {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceRunStatus::Cancelled,
        resolution: None,
        diagnostics: Vec::new(),
        error: Some("search run cancelled".to_string()),
    }
}

pub(super) fn source_run_failed_for_source(
    source_key: &str,
    source_name: &str,
    error: SourceExecutionError,
) -> SourceRunResult {
    SourceRunResult {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: error.status(),
        resolution: None,
        diagnostics: error.diagnostics(),
        error: Some(error.message()),
    }
}

pub(super) fn source_run_skipped_for_source(
    source_key: &str,
    source_name: &str,
    diagnostics: Diagnostics,
    summary: String,
) -> SourceRunResult {
    SourceRunResult {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceRunStatus::Skipped,
        resolution: None,
        diagnostics,
        error: Some(summary),
    }
}

pub(super) fn overall_status(source_runs: &[SourceRunResult]) -> SearchRunStatus {
    if source_runs
        .iter()
        .any(|run| run.status == SourceRunStatus::Cancelled)
    {
        return SearchRunStatus::Cancelled;
    }
    let completed = source_runs
        .iter()
        .filter(|run| run.status == SourceRunStatus::Completed)
        .count();
    match (completed, source_runs.len().saturating_sub(completed)) {
        (0, _) => SearchRunStatus::Failed,
        (_, 0) => SearchRunStatus::Completed,
        _ => SearchRunStatus::CompletedWithErrors,
    }
}
