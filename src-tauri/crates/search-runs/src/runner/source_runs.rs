use search_resolution::{Resolution, ResolutionError};

use source_engine::definition::{CompiledSource, Diagnostics};

use super::super::{ResolutionSummary, SourceOutcome, SourceStatus, Status};
use super::errors::SourceFailure;

fn source_identity(source: &CompiledSource) -> (&str, &str) {
    (source.source_key(), source.source_name())
}

pub(super) fn completed(source: &CompiledSource, resolution: &Resolution) -> SourceOutcome {
    let (source_key, source_name) = source_identity(source);
    SourceOutcome {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceStatus::Completed,
        resolution: Some(ResolutionSummary::from(resolution)),
        diagnostics: resolution.diagnostics.clone(),
        error: None,
    }
}

pub(super) fn resolution_failed(source: &CompiledSource, error: ResolutionError) -> SourceOutcome {
    let (source_key, source_name) = source_identity(source);
    match error {
        ResolutionError::Cancelled => cancelled_for_source(source_key, source_name),
        ResolutionError::Failed {
            failure,
            diagnostics,
        } => SourceOutcome {
            source_key: source_key.to_string(),
            source_name: source_name.to_string(),
            status: SourceStatus::Failed,
            resolution: None,
            diagnostics,
            error: Some(format!("Candidate Resolution failed: {failure:?}")),
        },
    }
}

pub(super) fn failed_for_key(source_key: &str, error: SourceFailure) -> SourceOutcome {
    failed_for_source(source_key, "", error)
}

pub(super) fn cancelled_for_key(source_key: &str) -> SourceOutcome {
    cancelled_for_source(source_key, "")
}

pub(super) fn cancelled_for_source(source_key: &str, source_name: &str) -> SourceOutcome {
    SourceOutcome {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceStatus::Cancelled,
        resolution: None,
        diagnostics: Vec::new(),
        error: Some("search run cancelled".to_string()),
    }
}

pub(super) fn failed_for_source(
    source_key: &str,
    source_name: &str,
    error: SourceFailure,
) -> SourceOutcome {
    SourceOutcome {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceStatus::Failed,
        resolution: None,
        diagnostics: error.diagnostics(),
        error: Some(error.message()),
    }
}

pub(super) fn skipped_for_source(
    source_key: &str,
    source_name: &str,
    diagnostics: Diagnostics,
    summary: String,
) -> SourceOutcome {
    SourceOutcome {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        status: SourceStatus::Skipped,
        resolution: None,
        diagnostics,
        error: Some(summary),
    }
}

pub(super) fn overall_status(source_runs: &[SourceOutcome]) -> Status {
    if source_runs
        .iter()
        .any(|run| run.status == SourceStatus::Cancelled)
    {
        return Status::Cancelled;
    }
    let completed = source_runs
        .iter()
        .filter(|run| run.status == SourceStatus::Completed)
        .count();
    match (completed, source_runs.len().saturating_sub(completed)) {
        (0, _) => Status::Failed,
        (_, 0) => Status::Completed,
        _ => Status::CompletedWithErrors,
    }
}
