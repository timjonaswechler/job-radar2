use crate::profile_dsl::diagnostics::Diagnostics;

use super::{
    super::{
        PostingSource, SearchRunStatus, SourceExecutionSource, SourceRunResult, SourceRunStatus,
    },
    SourceExecutionError,
};

pub(super) fn posting_source(source: &SourceExecutionSource, url: Option<String>) -> PostingSource {
    PostingSource {
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        url: url.unwrap_or_default(),
        posting_meta: Default::default(),
    }
}

pub(super) fn source_run_completed(
    source: &SourceExecutionSource,
    candidate_count: usize,
    diagnostics: Diagnostics,
) -> SourceRunResult {
    SourceRunResult {
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        status: SourceRunStatus::Completed,
        candidate_count,
        matched_count: 0,
        diagnostics,
        error: None,
    }
}

pub(super) fn source_run_failed(
    source: &SourceExecutionSource,
    error: SourceExecutionError,
) -> SourceRunResult {
    SourceRunResult {
        source_key: source.key.clone(),
        source_name: source.name.clone(),
        status: error.status(),
        candidate_count: 0,
        matched_count: 0,
        diagnostics: error.diagnostics(),
        error: Some(error.message()),
    }
}

pub(super) fn source_run_failed_for_key(
    source_key: &str,
    error: SourceExecutionError,
) -> SourceRunResult {
    source_run_failed_for_source(source_key, "", error)
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
        candidate_count: 0,
        matched_count: 0,
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
        candidate_count: 0,
        matched_count: 0,
        diagnostics,
        error: Some(summary),
    }
}

pub(super) fn overall_status(source_runs: &[SourceRunResult]) -> SearchRunStatus {
    if source_runs
        .iter()
        .all(|source_run| source_run.status == SourceRunStatus::Cancelled)
    {
        return SearchRunStatus::Cancelled;
    }

    let completed_count = source_runs
        .iter()
        .filter(|source_run| source_run.status == SourceRunStatus::Completed)
        .count();
    let failed_or_cancelled_or_skipped_count = source_runs.len().saturating_sub(completed_count);

    match (completed_count, failed_or_cancelled_or_skipped_count) {
        (0, _) => SearchRunStatus::Failed,
        (_, 0) => SearchRunStatus::Completed,
        _ => SearchRunStatus::CompletedWithErrors,
    }
}
