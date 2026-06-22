use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

use crate::{
    search::request::RunningSearchRuns,
    search::run::{SearchRunResult, SearchRunService, SourceExecutor},
};

use super::request::{get_or_create_smoke_search_request, smoke_source_keys};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeSummary {
    pub search_request_id: i64,
    pub search_request_created: bool,
    pub result_path: String,
    pub result: SearchRunResult,
}

pub(crate) async fn run_schott_stepstone_smoke(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_executor: &dyn SourceExecutor,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let source_keys = smoke_source_keys();
    let (search_request, search_request_created) =
        get_or_create_smoke_search_request(pool, running_search_runs, source_keys).await?;

    let result = SearchRunService::new(
        pool,
        running_search_runs,
        source_executor,
        result_path.clone(),
        source_registry_app_data_dir,
    )
    .run(search_request.id)
    .await?;

    Ok(SearchRunSmokeSummary {
        search_request_id: search_request.id,
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        result,
    })
}
