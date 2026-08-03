mod errors;
mod merging;
mod selection;
mod source_runs;
mod sqlite;

use std::{fmt, sync::Arc};

use geo::GeoResolver;
use search_requests::Execution;
use search_resolution::{
    CandidateDiagnosticSummary, Requirements, Resolution, ResolutionCompletion, ResolutionCounts,
    ResolutionError, Resolver,
};
use serde::{Deserialize, Serialize};
use source_engine::{
    definition::Diagnostics,
    execution::{BrowserAcquisition, PhaseUsage, ProfileHttpClient, RuntimeCancellation},
};
use sqlx::SqlitePool;

use merging::{finalized_merge_input, merge_postings};
use selection::{resolve_selected_sources, Selected};
use source_runs::{
    cancelled_for_key, cancelled_for_source, completed, failed_for_key, failed_for_source,
    overall_status, resolution_failed, skipped_for_source,
};
use sqlite::{commit, Commit};

/// Executes one admitted immutable Search Request through one atomic terminal commit.
pub struct Runner {
    pool: SqlitePool,
    installed: sources::installed::Store,
    http: Arc<dyn ProfileHttpClient + Send + Sync>,
    browser: Arc<dyn BrowserAcquisition>,
}

impl Runner {
    pub fn new(
        pool: SqlitePool,
        installed: sources::installed::Store,
        http: Arc<dyn ProfileHttpClient + Send + Sync>,
        browser: Arc<dyn BrowserAcquisition>,
    ) -> Self {
        Self {
            pool,
            installed,
            http,
            browser,
        }
    }

    pub async fn run(&self, execution: Execution, context: Context<'_>) -> Result<Outcome, Error> {
        let request = execution.snapshot();
        let search_request_id = execution.id().get();
        let requirements = match request.radius_km {
            Some(radius) => {
                let resolver = context.geo.ok_or_else(|| {
                    Error::Requirements(
                        "Search Request radius requires an available GeoResolver".to_string(),
                    )
                })?;
                Requirements::compile_with_geo(
                    &request.include_rules,
                    &request.exclude_rules,
                    &request.locations,
                    Some(radius),
                    resolver,
                )
                .await
                .map_err(|error| Error::Requirements(error.to_string()))?
            }
            None => Requirements::compile(
                &request.include_rules,
                &request.exclude_rules,
                &request.locations,
                None,
            )
            .map_err(|failure| {
                Error::Requirements(format!(
                    "Search Request matching requirements are invalid: {failure:?}"
                ))
            })?,
        };

        let installed = self.installed.clone();
        let snapshot = tokio::task::spawn_blocking(move || installed.snapshot())
            .await
            .map_err(|error| Error::InstalledState(error.to_string()))?
            .map_err(|error| Error::InstalledState(error.to_string()))?;
        let selected =
            resolve_selected_sources(&snapshot, &request.source_keys, context.source_admission);
        let never_cancelled = NeverCancelled;
        let cancellation = context.cancellation.unwrap_or(&never_cancelled);
        let mut source_runs = Vec::with_capacity(selected.len());
        let mut finalized = Vec::new();

        for selected_source in &selected {
            if cancellation.is_cancelled() {
                source_runs.push(cancelled_selected(selected_source));
                continue;
            }
            let source = match selected_source {
                Selected::Resolved(source) => *source,
                Selected::Missing { source_key, error } => {
                    source_runs.push(failed_for_key(source_key, error.clone()));
                    continue;
                }
                Selected::Failed {
                    source_key,
                    source_name,
                    error,
                } => {
                    source_runs.push(failed_for_source(source_key, source_name, error.clone()));
                    continue;
                }
                Selected::Skipped {
                    source_key,
                    source_name,
                    diagnostics,
                    summary,
                } => {
                    source_runs.push(skipped_for_source(
                        source_key,
                        source_name,
                        diagnostics.clone(),
                        summary.clone(),
                    ));
                    continue;
                }
            };

            match self.resolve(source, &requirements, cancellation).await {
                Ok(resolution) => {
                    finalized.extend(
                        resolution.finalized.iter().map(|candidate| {
                            finalized_merge_input(candidate, source.source_name())
                        }),
                    );
                    source_runs.push(completed(source, &resolution));
                }
                Err(error) => source_runs.push(resolution_failed(source, error)),
            }
        }

        let source_status = overall_status(&source_runs);
        let merged = if matches!(
            source_status,
            Status::Completed | Status::CompletedWithErrors
        ) {
            merge_postings(finalized)
        } else {
            Vec::new()
        };
        let generated_at = generated_at(&self.pool).await.map_err(Error::Storage)?;
        // The final cancellation observation is the terminal linearization point. The commit and
        // returned Outcome are non-cancellable and authoritative after this observation.
        let cancelled = context
            .cancellation
            .is_some_and(RuntimeCancellation::is_cancelled);
        let status = if cancelled {
            Status::Cancelled
        } else {
            source_status
        };
        let postings = if cancelled {
            &[][..]
        } else {
            merged.as_slice()
        };
        let outcome = Outcome {
            search_request_id,
            status,
            generated_at,
            diagnostics: Vec::new(),
            source_runs,
            matched_posting_count: postings.len(),
        };
        let last_run_error = last_run_error(&outcome);
        commit(
            &self.pool,
            Commit {
                search_request_id,
                status,
                generated_at: &outcome.generated_at,
                last_run_error: last_run_error.as_deref(),
                postings,
            },
        )
        .await
        .map_err(Error::Storage)?;
        Ok(outcome)
    }

    async fn resolve(
        &self,
        source: &source_engine::definition::CompiledSource,
        requirements: &Requirements<'_>,
        cancellation: &dyn RuntimeCancellation,
    ) -> Result<Resolution, ResolutionError> {
        Resolver::new(self.http.as_ref(), self.browser.as_ref())
            .resolve(source, requirements, cancellation)
            .await
    }
}

pub struct Context<'a> {
    pub cancellation: Option<&'a dyn RuntimeCancellation>,
    pub geo: Option<&'a dyn GeoResolver>,
    pub source_admission: SourceAdmission,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceAdmission {
    #[default]
    ActiveOnly,
    DevelopmentSmokeAllowDraft,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

impl Status {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::CompletedWithErrors => "completed_with_errors",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for Status {
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
pub enum SourceStatus {
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

impl SourceStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub search_request_id: i64,
    pub status: Status,
    pub generated_at: String,
    pub diagnostics: Diagnostics,
    pub source_runs: Vec<SourceOutcome>,
    pub matched_posting_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Requirements(String),
    InstalledState(String),
    Storage(String),
}

impl fmt::Display for Error {
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
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOutcome {
    pub source_key: String,
    pub source_name: String,
    pub status: SourceStatus,
    pub resolution: Option<ResolutionSummary>,
    pub diagnostics: Diagnostics,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionSummary {
    pub completion: ResolutionCompletion,
    pub counts: ResolutionCounts,
    pub usage: PhaseUsage,
    pub candidate_diagnostics: CandidateDiagnosticSummary,
}

impl From<&Resolution> for ResolutionSummary {
    fn from(resolution: &Resolution) -> Self {
        Self {
            completion: resolution.completion.clone(),
            counts: resolution.counts,
            usage: resolution.usage,
            candidate_diagnostics: resolution.candidate_diagnostics.clone(),
        }
    }
}

struct NeverCancelled;
impl RuntimeCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

async fn generated_at(pool: &SqlitePool) -> Result<String, String> {
    sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())
}

fn last_run_error(outcome: &Outcome) -> Option<String> {
    if outcome.status == Status::Completed {
        return None;
    }
    let unsuccessful = outcome
        .source_runs
        .iter()
        .filter(|source| source.status != SourceStatus::Completed)
        .collect::<Vec<_>>();
    if unsuccessful.is_empty() {
        return Some(format!("search run {}", outcome.status.as_str()));
    }
    let details = unsuccessful
        .iter()
        .take(3)
        .map(|source| {
            let message = source
                .error
                .as_deref()
                .unwrap_or_else(|| source.status.as_str());
            format!("{}: {message}", source.source_key)
        })
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if unsuccessful.len() > 3 { "; ..." } else { "" };
    let noun = if unsuccessful.len() == 1 {
        "source run"
    } else {
        "source runs"
    };
    let result = if outcome.status == Status::Cancelled {
        "cancelled"
    } else {
        "failed"
    };
    Some(format!(
        "{} {noun} {result}: {details}{suffix}",
        unsuccessful.len()
    ))
}

fn cancelled_selected(selected: &Selected<'_>) -> SourceOutcome {
    match selected {
        Selected::Resolved(source) => {
            cancelled_for_source(source.source_key(), source.source_name())
        }
        Selected::Missing { source_key, .. } => cancelled_for_key(source_key),
        Selected::Failed {
            source_key,
            source_name,
            ..
        }
        | Selected::Skipped {
            source_key,
            source_name,
            ..
        } => cancelled_for_source(source_key, source_name),
    }
}
