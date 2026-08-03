//! Atomic terminal Search Run, Posting/source, Match, and latest-projection persistence.

use search_resolution::{merge_unique_locations, same_job_posting, PostingComparison};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use super::Status;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Posting {
    pub(super) title: String,
    pub(super) company: String,
    pub(super) locations: Vec<String>,
    pub(super) sources: Vec<PostingSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PostingSource {
    pub(super) source_key: String,
    pub(super) source_name: String,
    pub(super) url: String,
}

pub(super) struct Commit<'a> {
    pub(super) search_request_id: i64,
    pub(super) status: Status,
    pub(super) generated_at: &'a str,
    pub(super) last_run_error: Option<&'a str>,
    pub(super) postings: &'a [Posting],
}

pub(super) async fn commit(pool: &SqlitePool, input: Commit<'_>) -> Result<i64, String> {
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

fn validate_input(input: &Commit<'_>) -> Result<(), String> {
    match input.status {
        Status::Completed | Status::CompletedWithErrors => validate_merged_postings(input.postings),
        Status::Failed | Status::Cancelled if input.postings.is_empty() => Ok(()),
        Status::Failed | Status::Cancelled => Err(format!(
            "{} Search Run cannot persist posting or Match input",
            input.status.as_str()
        )),
    }
}

fn validate_merged_postings(postings: &[Posting]) -> Result<(), String> {
    for posting in postings {
        if posting.sources.is_empty() {
            return Err("posting has no sources".to_string());
        }
        if posting.title.trim().is_empty() {
            return Err("posting title is empty".to_string());
        }
        if posting.company.trim().is_empty() {
            return Err("posting company is empty".to_string());
        }
        if posting
            .sources
            .iter()
            .any(|source| source.url.trim().is_empty())
        {
            return Err("posting source url is empty".to_string());
        }
    }

    Ok(())
}

async fn persist_merged_posting_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &Posting,
    seen_at: &str,
) -> Result<i64, String> {
    match find_existing_posting(transaction, posting).await? {
        Some(posting_id) => {
            update_existing_posting(transaction, posting_id, posting, seen_at).await?;
            Ok(posting_id)
        }
        None => insert_new_posting(transaction, posting, seen_at).await,
    }
}

async fn find_existing_posting(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &Posting,
) -> Result<Option<i64>, String> {
    if let Some(posting_id) = find_posting_by_source_url(transaction, posting).await? {
        return Ok(Some(posting_id));
    }

    find_posting_by_dedupe(transaction, posting).await
}

async fn find_posting_by_source_url(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &Posting,
) -> Result<Option<i64>, String> {
    for source in &posting.sources {
        let rows = sqlx::query(
            "SELECT posting_id
             FROM job_posting_sources
             WHERE source_key = ?1 AND url = ?2
             ORDER BY id",
        )
        .bind(&source.source_key)
        .bind(&source.url)
        .fetch_all(&mut **transaction)
        .await
        .map_err(db_error)?;

        if let Some(row) = rows.first() {
            return row
                .try_get::<i64, _>("posting_id")
                .map(Some)
                .map_err(db_error);
        }
    }

    Ok(None)
}

async fn find_posting_by_dedupe(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &Posting,
) -> Result<Option<i64>, String> {
    let rows = sqlx::query(
        "SELECT id, title, company, locations_json
         FROM job_postings
         ORDER BY id",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(db_error)?;

    for row in rows {
        let id = row.try_get::<i64, _>("id").map_err(db_error)?;
        let title = row.try_get::<String, _>("title").map_err(db_error)?;
        let company = row.try_get::<String, _>("company").map_err(db_error)?;
        let locations_json = row
            .try_get::<String, _>("locations_json")
            .map_err(db_error)?;
        let locations = locations_from_json(&locations_json)?;

        if same_job_posting(
            PostingComparison {
                title: &title,
                company: &company,
                locations: &locations,
            },
            PostingComparison {
                title: &posting.title,
                company: &posting.company,
                locations: &posting.locations,
            },
        ) {
            return Ok(Some(id));
        }
    }

    Ok(None)
}

async fn insert_new_posting(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &Posting,
    seen_at: &str,
) -> Result<i64, String> {
    let locations_json = serde_json::to_string(&posting.locations).map_err(json_error)?;
    let inserted_posting = sqlx::query(
        "INSERT INTO job_postings (
           title, company, locations_json, first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )
    .bind(&posting.title)
    .bind(&posting.company)
    .bind(locations_json)
    .bind(seen_at)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;
    let posting_id = inserted_posting.last_insert_rowid();

    let mut primary_source_id = None;
    for source in &posting.sources {
        let source_id = upsert_posting_source(transaction, posting_id, source, seen_at).await?;
        if primary_source_id.is_none() {
            primary_source_id = Some(source_id);
        }
    }

    sqlx::query("UPDATE job_postings SET primary_source_id = ?1 WHERE id = ?2")
        .bind(primary_source_id)
        .bind(posting_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;

    Ok(posting_id)
}

async fn update_existing_posting(
    transaction: &mut Transaction<'_, Sqlite>,
    posting_id: i64,
    posting: &Posting,
    seen_at: &str,
) -> Result<(), String> {
    let existing_locations_json = sqlx::query_scalar::<_, String>(
        "SELECT locations_json
         FROM job_postings
         WHERE id = ?1",
    )
    .bind(posting_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(db_error)?;
    let existing_locations = locations_from_json(&existing_locations_json)?;
    let merged_locations = merge_locations(existing_locations, &posting.locations);
    let merged_locations_json = serde_json::to_string(&merged_locations).map_err(json_error)?;

    sqlx::query(
        "UPDATE job_postings
         SET locations_json = ?1,
             last_seen_at = ?2
         WHERE id = ?3",
    )
    .bind(merged_locations_json)
    .bind(seen_at)
    .bind(posting_id)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;

    for source in &posting.sources {
        upsert_posting_source(transaction, posting_id, source, seen_at).await?;
    }

    Ok(())
}

async fn upsert_posting_source(
    transaction: &mut Transaction<'_, Sqlite>,
    posting_id: i64,
    source: &PostingSource,
    seen_at: &str,
) -> Result<i64, String> {
    let existing_source_id = sqlx::query_scalar::<_, i64>(
        "SELECT id
         FROM job_posting_sources
         WHERE posting_id = ?1 AND source_key = ?2 AND url = ?3",
    )
    .bind(posting_id)
    .bind(&source.source_key)
    .bind(&source.url)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(db_error)?;

    let posting_meta_json = "{}";

    if let Some(source_id) = existing_source_id {
        sqlx::query(
            "UPDATE job_posting_sources
             SET source_name_snapshot = ?1,
                 posting_meta_json = ?2,
                 last_seen_at = ?3
             WHERE id = ?4",
        )
        .bind(&source.source_name)
        .bind(posting_meta_json)
        .bind(seen_at)
        .bind(source_id)
        .execute(&mut **transaction)
        .await
        .map_err(db_error)?;

        return Ok(source_id);
    }

    let inserted_source = sqlx::query(
        "INSERT INTO job_posting_sources (
           posting_id, source_key, source_name_snapshot, url, posting_meta_json, first_seen_at, last_seen_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
    )
    .bind(posting_id)
    .bind(&source.source_key)
    .bind(&source.source_name)
    .bind(&source.url)
    .bind(posting_meta_json)
    .bind(seen_at)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;

    Ok(inserted_source.last_insert_rowid())
}

fn merge_locations(existing_locations: Vec<String>, new_locations: &[String]) -> Vec<String> {
    merge_unique_locations(existing_locations, new_locations)
}

fn locations_from_json(json: &str) -> Result<Vec<String>, String> {
    serde_json::from_str(json).map_err(json_error)
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}

fn json_error(error: serde_json::Error) -> String {
    error.to_string()
}
