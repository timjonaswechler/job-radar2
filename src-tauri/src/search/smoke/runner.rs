use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

use crate::search::run::{SearchRunResolutionRuntime, SearchRunResult, SearchRunService};
use search_requests::Catalog;

use super::request::get_or_create_smoke_search_request;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeSummary {
    pub search_request_id: i64,
    pub search_request_created: bool,
    pub result_path: String,
    pub result: SearchRunResult,
}

#[cfg(test)]
pub(crate) async fn run_search_run_smoke(
    pool: &SqlitePool,
    catalog: &Catalog,
    resolver: &SearchRunResolutionRuntime,
    result_path: impl Into<PathBuf>,
    installed_sources: sources::installed::Store,
    source_keys: Vec<String>,
) -> Result<SearchRunSmokeSummary, String> {
    run_search_run_smoke_with_options(
        pool,
        catalog,
        resolver,
        result_path,
        installed_sources,
        source_keys,
        false,
    )
    .await
}

pub(crate) async fn run_search_run_smoke_with_options(
    pool: &SqlitePool,
    catalog: &Catalog,
    resolver: &SearchRunResolutionRuntime,
    result_path: impl Into<PathBuf>,
    installed_sources: sources::installed::Store,
    source_keys: Vec<String>,
    allow_draft_sources: bool,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let (request, search_request_created) =
        get_or_create_smoke_search_request(catalog, source_keys).await?;
    let geo_db_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/geo_loc.sqlite");
    let geo_resolver = crate::geo::GeoDbResolver::connect(&geo_db_path).await?;
    let execution = catalog
        .begin_execution(request.id)
        .await
        .map_err(|error| error.to_string())?;
    let result = SearchRunService::new(pool, resolver, result_path.clone(), installed_sources)
        .with_geo_resolver(&geo_resolver)
        .allowing_draft_sources(allow_draft_sources)
        .run(execution)
        .await?;

    Ok(SearchRunSmokeSummary {
        search_request_id: request.id.get(),
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        result,
    })
}
