use serde::Serialize;
use std::path::PathBuf;

use search_requests::Catalog;

use super::request::get_or_create_smoke_search_request;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeSummary {
    pub search_request_id: i64,
    pub search_request_created: bool,
    pub result_path: String,
    pub result: search_runs::Outcome,
}

pub(crate) async fn run_search_run_smoke_with_options(
    catalog: &Catalog,
    runner: &search_runs::Runner,
    result_path: impl Into<PathBuf>,
    source_keys: Vec<String>,
    allow_draft_sources: bool,
    geo: &dyn geo::GeoResolver,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let (request, search_request_created) =
        get_or_create_smoke_search_request(catalog, source_keys).await?;
    let execution = catalog
        .begin_execution(request.id)
        .await
        .map_err(|error| error.to_string())?;
    let mut result = runner
        .run(
            execution,
            search_runs::Context {
                cancellation: None,
                geo: Some(geo),
                source_admission: if allow_draft_sources {
                    search_runs::SourceAdmission::DevelopmentSmokeAllowDraft
                } else {
                    search_runs::SourceAdmission::ActiveOnly
                },
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    crate::adapters::search_run_artifact::write(&result_path, &mut result).await;

    Ok(SearchRunSmokeSummary {
        search_request_id: request.id.get(),
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        result,
    })
}
