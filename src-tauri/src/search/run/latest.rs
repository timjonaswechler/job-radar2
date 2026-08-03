use std::collections::HashMap;

use search_requests::Id;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

use search_runs::Status;

// Stay well below SQLite's host-parameter limit while preserving a batched, non-N+1 query path.
const SQLITE_PROJECTION_CHUNK: usize = 500;

#[derive(Clone, Debug, Default)]
pub(crate) struct LatestSummary {
    pub(crate) at: Option<String>,
    pub(crate) status: Option<Status>,
    pub(crate) error: Option<String>,
}

pub(crate) async fn latest_summary(
    pool: &SqlitePool,
    request_id: Id,
) -> Result<LatestSummary, String> {
    Ok(latest_summaries(pool, &[request_id])
        .await?
        .remove(&request_id)
        .unwrap_or_default())
}

pub(crate) async fn latest_summaries(
    pool: &SqlitePool,
    request_ids: &[Id],
) -> Result<HashMap<Id, LatestSummary>, String> {
    let mut summaries = HashMap::with_capacity(request_ids.len());
    for ids in request_ids.chunks(SQLITE_PROJECTION_CHUNK) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT id, last_run_at, last_run_status, last_run_error FROM search_requests WHERE id IN (",
        );
        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(id.get());
        }
        separated.push_unseparated(")");
        let rows = query
            .build()
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())?;
        for row in rows {
            let raw_id: i64 = row.try_get("id").map_err(|error| error.to_string())?;
            let id = Id::new(raw_id).map_err(|error| error.to_string())?;
            let status: Option<String> = row
                .try_get("last_run_status")
                .map_err(|error| error.to_string())?;
            summaries.insert(
                id,
                LatestSummary {
                    at: row
                        .try_get("last_run_at")
                        .map_err(|error| error.to_string())?,
                    status: status.as_deref().map(Status::try_from).transpose()?,
                    error: row
                        .try_get("last_run_error")
                        .map_err(|error| error.to_string())?,
                },
            );
        }
    }
    Ok(summaries)
}
