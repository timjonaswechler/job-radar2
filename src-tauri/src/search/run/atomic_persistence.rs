//! Dormant atomic persistence for terminal Search Runs and their Matches.
//!
//! The operation accepts only terminal facts and already cross-Source-merged posting rows. It
//! deliberately performs no Candidate conversion, matching, normalization, or merge work and has
//! no productive caller until the finalized-only Search Run flow is activated.

use sqlx::SqlitePool;

use crate::search::posting::{persist_merged_posting_in_transaction, validate_merged_postings};

use super::{NormalizedPosting, SearchRunStatus};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct AtomicSearchRunInput<'a> {
    pub(crate) search_request_id: i64,
    pub(crate) status: SearchRunStatus,
    pub(crate) generated_at: &'a str,
    pub(crate) last_run_error: Option<&'a str>,
    pub(crate) postings: &'a [NormalizedPosting],
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) async fn persist_atomic_search_run(
    pool: &SqlitePool,
    input: AtomicSearchRunInput<'_>,
) -> Result<i64, String> {
    validate_input(&input)?;

    let mut transaction = pool.begin().await.map_err(db_error)?;
    let inserted_run = sqlx::query(
        "INSERT INTO search_runs (search_request_id, status, generated_at)
         VALUES (?1, ?2, ?3)",
    )
    .bind(input.search_request_id)
    .bind(input.status.as_str())
    .bind(input.generated_at)
    .execute(&mut *transaction)
    .await
    .map_err(db_error)?;
    let search_run_id = inserted_run.last_insert_rowid();

    for posting in input.postings {
        let posting_id =
            persist_merged_posting_in_transaction(&mut transaction, posting, input.generated_at)
                .await?;
        sqlx::query(
            "INSERT INTO matches (search_run_id, job_posting_id)
             VALUES (?1, ?2)",
        )
        .bind(search_run_id)
        .bind(posting_id)
        .execute(&mut *transaction)
        .await
        .map_err(db_error)?;
    }

    let metadata_update = sqlx::query(
        "UPDATE search_requests
         SET last_run_at = ?1,
             last_run_status = ?2,
             last_run_error = ?3
         WHERE id = ?4",
    )
    .bind(input.generated_at)
    .bind(input.status.as_str())
    .bind(input.last_run_error)
    .bind(input.search_request_id)
    .execute(&mut *transaction)
    .await
    .map_err(db_error)?;
    if metadata_update.rows_affected() != 1 {
        return Err(format!(
            "search request {} not found while updating terminal run metadata",
            input.search_request_id
        ));
    }

    transaction.commit().await.map_err(db_error)?;
    Ok(search_run_id)
}

fn validate_input(input: &AtomicSearchRunInput<'_>) -> Result<(), String> {
    match input.status {
        SearchRunStatus::Completed | SearchRunStatus::CompletedWithErrors => {
            validate_merged_postings(input.postings)
        }
        SearchRunStatus::Failed | SearchRunStatus::Cancelled if input.postings.is_empty() => Ok(()),
        SearchRunStatus::Failed | SearchRunStatus::Cancelled => Err(format!(
            "{} Search Run cannot persist posting or Match input",
            input.status.as_str()
        )),
    }
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
