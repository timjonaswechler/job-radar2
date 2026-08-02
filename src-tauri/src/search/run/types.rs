use search_resolution::{
    CandidateDiagnosticSummary, ResolutionCompletion, ResolutionCounts, SourceResolution,
};
use serde::{Deserialize, Serialize};
use std::fmt;

use source_engine::{definition::Diagnostics, execution::PhaseUsage};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRunStatus {
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

impl SearchRunStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::CompletedWithErrors => "completed_with_errors",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for SearchRunStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "completed" => Ok(Self::Completed),
            "completed_with_errors" => Ok(Self::CompletedWithErrors),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("unknown search run status: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRunStatus {
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

impl SourceRunStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }
}

/// Authoritative terminal outcome of one committed Search Run.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRunOutcome {
    pub search_request_id: i64,
    pub status: SearchRunStatus,
    pub generated_at: String,
    pub diagnostics: Diagnostics,
    pub source_runs: Vec<SourceRunResult>,
    pub matched_posting_count: usize,
}

/// Top-level failures that occur before a terminal Search Run can be committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchRunError {
    Requirements(String),
    InstalledState(String),
    Storage(String),
}

impl fmt::Display for SearchRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requirements(message) => {
                write!(formatter, "Search Run requirements failed: {message}")
            }
            Self::InstalledState(message) => {
                write!(formatter, "Search Run installed state failed: {message}")
            }
            Self::Storage(message) => write!(formatter, "Search Run storage failed: {message}"),
        }
    }
}

impl std::error::Error for SearchRunError {}

/// Source Run outcome for one selected Source.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRunResult {
    pub source_key: String,
    pub source_name: String,
    pub status: SourceRunStatus,
    pub resolution: Option<SourceResolutionSummary>,
    pub diagnostics: Diagnostics,
    pub error: Option<String>,
}

/// Bounded, non-authoritative projection of one Candidate Resolution.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResolutionSummary {
    pub completion: ResolutionCompletion,
    pub counts: ResolutionCounts,
    pub remaining: Option<u64>,
    pub usage: PhaseUsage,
    pub candidate_diagnostics: CandidateDiagnosticSummary,
}

impl From<&SourceResolution> for SourceResolutionSummary {
    fn from(resolution: &SourceResolution) -> Self {
        Self {
            completion: resolution.completion.clone(),
            counts: resolution.counts,
            remaining: resolution.remaining,
            usage: resolution.report.usage,
            candidate_diagnostics: resolution.candidate_diagnostics.clone(),
        }
    }
}
