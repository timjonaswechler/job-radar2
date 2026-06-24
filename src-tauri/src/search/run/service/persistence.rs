use std::path::{Path, PathBuf};

use sqlx::{Sqlite, SqlitePool, Transaction};

use super::super::{SearchRunResult, SearchRunStatus, SourceRunStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchRunResultArtifact {
    Disabled,
    WriteTo(PathBuf),
}

pub fn default_search_run_result_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap_or(manifest_dir.as_path())
        .join("search-run-result.json")
}

pub fn default_search_run_result_artifact() -> SearchRunResultArtifact {
    if cfg!(debug_assertions) {
        SearchRunResultArtifact::WriteTo(default_search_run_result_path())
    } else {
        SearchRunResultArtifact::Disabled
    }
}

pub(super) async fn generated_at_timestamp(pool: &SqlitePool) -> Result<String, String> {
    sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await
        .map_err(db_error)
}

pub(super) async fn update_search_request_last_run(
    transaction: &mut Transaction<'_, Sqlite>,
    result: &SearchRunResult,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE search_requests
         SET last_run_at = ?1,
             last_run_status = ?2,
             last_run_error = ?3
         WHERE id = ?4",
    )
    .bind(&result.generated_at)
    .bind(result.status.as_str())
    .bind(last_run_error_summary(result))
    .bind(result.search_request_id)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;

    Ok(())
}

fn last_run_error_summary(result: &SearchRunResult) -> Option<String> {
    if result.status == SearchRunStatus::Completed {
        return None;
    }

    let unsuccessful_source_runs = result
        .source_runs
        .iter()
        .filter(|source_run| source_run.status != SourceRunStatus::Completed)
        .collect::<Vec<_>>();

    if unsuccessful_source_runs.is_empty() {
        return Some(format!("search run {}", result.status.as_str()));
    }

    let details = unsuccessful_source_runs
        .iter()
        .take(3)
        .map(|source_run| {
            let message = source_run
                .error
                .as_deref()
                .unwrap_or_else(|| source_run.status.as_str());
            format!("{}: {message}", source_run.source_key)
        })
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if unsuccessful_source_runs.len() > 3 {
        "; ..."
    } else {
        ""
    };
    let noun = if unsuccessful_source_runs.len() == 1 {
        "source run"
    } else {
        "source runs"
    };
    let outcome = if result.status == SearchRunStatus::Cancelled {
        "cancelled"
    } else {
        "failed"
    };

    Some(format!(
        "{} {noun} {outcome}: {details}{suffix}",
        unsuccessful_source_runs.len()
    ))
}

pub(super) async fn write_search_run_result(
    path: &Path,
    result: &SearchRunResult,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| error.to_string())?;
        }
    }

    let json = serde_json::to_string_pretty(result).map_err(|error| error.to_string())?;
    tokio::fs::write(path, json)
        .await
        .map_err(|error| error.to_string())
}

pub(super) fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
