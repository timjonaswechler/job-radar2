use serde::{de::DeserializeOwned, Serialize};
use sqlx::{sqlite::SqliteRow, Row};

use crate::search::run::SearchRunStatus;

use super::{SearchRequest, SearchRequestStatus};

pub(super) fn search_request_from_row(row: SqliteRow) -> Result<SearchRequest, String> {
    let status: String = row.try_get("status").map_err(db_error)?;
    let last_run_status: Option<String> = row.try_get("last_run_status").map_err(db_error)?;

    Ok(SearchRequest {
        id: row.try_get("id").map_err(db_error)?,
        status: SearchRequestStatus::try_from(status.as_str())?,
        include_rules: json_from_row(&row, "include_rules_json")?,
        exclude_rules: json_from_row(&row, "exclude_rules_json")?,
        locations: json_from_row(&row, "locations_json")?,
        radius_km: row.try_get("radius_km").map_err(db_error)?,
        source_keys: json_from_row(&row, "source_keys_json")?,
        validation_error: row.try_get("validation_error").map_err(db_error)?,
        last_run_at: row.try_get("last_run_at").map_err(db_error)?,
        last_run_status: last_run_status
            .as_deref()
            .map(SearchRunStatus::try_from)
            .transpose()?,
        last_run_error: row.try_get("last_run_error").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
        updated_at: row.try_get("updated_at").map_err(db_error)?,
    })
}

pub(super) fn json_to_string<T>(value: &T) -> Result<String, String>
where
    T: Serialize,
{
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn json_from_row<T>(row: &SqliteRow, column: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let json: String = row.try_get(column).map_err(db_error)?;
    serde_json::from_str(&json).map_err(|error| error.to_string())
}

pub(super) fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
