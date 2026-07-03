use serde::Serialize;
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Mutex};

use crate::{
    search::request::RunningSearchRuns,
    search::run::{
        BoxedSourceExecutionFuture, SearchRunResult, SearchRunService, SourceCandidate,
        SourceExecutionInput, SourceExecutor,
    },
};

use super::request::get_or_create_smoke_search_request;
#[cfg(test)]
use super::request::smoke_source_keys;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeSummary {
    pub search_request_id: i64,
    pub search_request_created: bool,
    pub result_path: String,
    pub candidates_path: String,
    pub result: SearchRunResult,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SmokeSourceCandidates {
    pub source_key: String,
    pub source_name: String,
    pub candidates: Vec<SourceCandidate>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeCandidatesArtifact {
    pub search_request_id: i64,
    pub generated_at: String,
    pub result_path: String,
    pub sources: Vec<SmokeSourceCandidates>,
}

struct RecordingSourceExecutor<'a> {
    inner: &'a dyn SourceExecutor,
    sources: Mutex<Vec<SmokeSourceCandidates>>,
}

impl<'a> RecordingSourceExecutor<'a> {
    fn new(inner: &'a dyn SourceExecutor) -> Self {
        Self {
            inner,
            sources: Mutex::new(Vec::new()),
        }
    }

    fn recorded_sources(&self) -> Result<Vec<SmokeSourceCandidates>, String> {
        self.sources
            .lock()
            .map(|sources| sources.clone())
            .map_err(|_| "smoke candidates recorder lock was poisoned".to_string())
    }
}

impl SourceExecutor for RecordingSourceExecutor<'_> {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        let source_key = input.source.key.clone();
        let source_name = input.source.name.clone();
        Box::pin(async move {
            let output = self.inner.execute(input).await?;
            self.sources
                .lock()
                .map_err(|_| {
                    crate::search::run::SourceExecutionError::Failed(
                        "smoke candidates recorder lock was poisoned".to_string(),
                    )
                })?
                .push(SmokeSourceCandidates {
                    source_key,
                    source_name,
                    candidates: output.candidates.clone(),
                });
            Ok(output)
        })
    }
}

#[cfg(test)]
pub(crate) async fn run_schott_smoke(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_executor: &dyn SourceExecutor,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
) -> Result<SearchRunSmokeSummary, String> {
    run_search_run_smoke(
        pool,
        running_search_runs,
        source_executor,
        result_path,
        source_registry_app_data_dir,
        smoke_source_keys(),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn run_search_run_smoke(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_executor: &dyn SourceExecutor,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
    source_keys: Vec<String>,
) -> Result<SearchRunSmokeSummary, String> {
    run_search_run_smoke_with_options(
        pool,
        running_search_runs,
        source_executor,
        result_path,
        source_registry_app_data_dir,
        source_keys,
        false,
    )
    .await
}

pub(crate) async fn run_search_run_smoke_with_options(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_executor: &dyn SourceExecutor,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
    source_keys: Vec<String>,
    allow_draft_sources: bool,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let candidates_path = candidates_path_for_result_path(&result_path);
    let (search_request, search_request_created) =
        get_or_create_smoke_search_request(pool, running_search_runs, source_keys).await?;

    let recording_executor = RecordingSourceExecutor::new(source_executor);
    let result = SearchRunService::new(
        pool,
        running_search_runs,
        &recording_executor,
        result_path.clone(),
        source_registry_app_data_dir,
    )
    .allowing_draft_sources(allow_draft_sources)
    .run(search_request.id)
    .await?;

    write_candidates_artifact(
        &candidates_path,
        SearchRunSmokeCandidatesArtifact {
            search_request_id: search_request.id,
            generated_at: result.generated_at.clone(),
            result_path: result_path.to_string_lossy().to_string(),
            sources: recording_executor.recorded_sources()?,
        },
    )?;

    Ok(SearchRunSmokeSummary {
        search_request_id: search_request.id,
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        candidates_path: candidates_path.to_string_lossy().to_string(),
        result,
    })
}

fn candidates_path_for_result_path(result_path: &std::path::Path) -> PathBuf {
    result_path.with_file_name("search-run-candidates.json")
}

fn write_candidates_artifact(
    path: &std::path::Path,
    artifact: SearchRunSmokeCandidatesArtifact,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&artifact).map_err(|error| error.to_string())?;
    std::fs::write(path, format!("{json}\n")).map_err(|error| error.to_string())
}
