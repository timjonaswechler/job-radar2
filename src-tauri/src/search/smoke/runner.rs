use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

use crate::{
    search::request::RunningSearchRuns,
    search::run::{SearchRunResolutionRuntime, SearchRunResult, SearchRunService},
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
    pub result: SearchRunResult,
}

#[cfg(test)]
pub(crate) async fn run_schott_smoke(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    resolver: &SearchRunResolutionRuntime,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
) -> Result<SearchRunSmokeSummary, String> {
    run_search_run_smoke(
        pool,
        running_search_runs,
        resolver,
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
    resolver: &SearchRunResolutionRuntime,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
    source_keys: Vec<String>,
) -> Result<SearchRunSmokeSummary, String> {
    run_search_run_smoke_with_options(
        pool,
        running_search_runs,
        resolver,
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
    resolver: &SearchRunResolutionRuntime,
    result_path: impl Into<PathBuf>,
    source_registry_app_data_dir: impl Into<PathBuf>,
    source_keys: Vec<String>,
    allow_draft_sources: bool,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let (request, search_request_created) =
        get_or_create_smoke_search_request(pool, running_search_runs, source_keys).await?;
    let geo_db_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
    let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path).await?;
    let result = SearchRunService::new(
        pool,
        running_search_runs,
        resolver,
        result_path.clone(),
        source_registry_app_data_dir,
    )
    .with_geo_resolver(&geo_resolver)
    .allowing_draft_sources(allow_draft_sources)
    .run(request.id)
    .await?;

    Ok(SearchRunSmokeSummary {
        search_request_id: request.id,
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        result,
    })
}
