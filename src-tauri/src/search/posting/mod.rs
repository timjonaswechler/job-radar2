pub(crate) mod matching;
mod service;

#[cfg(test)]
mod tests;
mod types;

pub use service::JobPostingService;
pub use types::{
    ApplicationState, InterestState, JobPosting, JobPostingDetail, JobPostingQueueCounts,
    JobPostingQueueId, JobPostingSource, PostingDescriptionState, PreparationState, ReadState,
    UpdateJobPostingStateInput,
};

use std::collections::HashSet;

use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::search::{
    normalization::normalized_text_key,
    posting::matching::same_job_posting,
    run::{NormalizedPosting, PostingSource, SearchRunResult},
};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct JobPostingImportService<'a> {
    pool: &'a SqlitePool,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'a> JobPostingImportService<'a> {
    pub(crate) fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn import_search_run_result(
        &self,
        result: &SearchRunResult,
    ) -> Result<(), String> {
        let mut transaction = self.pool.begin().await.map_err(db_error)?;
        import_search_run_result_in_transaction(&mut transaction, result).await?;
        transaction.commit().await.map_err(db_error)
    }
}

pub(crate) async fn import_search_run_result_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    result: &SearchRunResult,
) -> Result<(), String> {
    validate_import_result(result)?;

    for posting in &result.postings {
        match find_existing_posting(transaction, posting).await? {
            Some(posting_id) => {
                update_existing_posting(transaction, posting_id, posting, result).await?;
            }
            None => {
                insert_new_posting(transaction, posting, result).await?;
            }
        }
    }

    Ok(())
}

fn validate_import_result(result: &SearchRunResult) -> Result<(), String> {
    for posting in &result.postings {
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

async fn find_existing_posting(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &NormalizedPosting,
) -> Result<Option<i64>, String> {
    if let Some(posting_id) = find_posting_by_source_url(transaction, posting).await? {
        return Ok(Some(posting_id));
    }

    find_posting_by_dedupe(transaction, posting).await
}

async fn find_posting_by_source_url(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &NormalizedPosting,
) -> Result<Option<i64>, String> {
    for source in &posting.sources {
        let rows = sqlx::query(
            "SELECT posting_id, posting_meta_json
             FROM job_posting_sources
             WHERE source_key = ?1 AND url = ?2
             ORDER BY id",
        )
        .bind(&source.source_key)
        .bind(&source.url)
        .fetch_all(&mut **transaction)
        .await
        .map_err(db_error)?;

        for row in rows {
            let existing_posting_meta_json = row
                .try_get::<String, _>("posting_meta_json")
                .map_err(db_error)?;
            if posting_meta_identity_matches(&existing_posting_meta_json, source)? {
                return row
                    .try_get::<i64, _>("posting_id")
                    .map(Some)
                    .map_err(db_error);
            }
        }
    }

    Ok(None)
}

fn posting_meta_identity_matches(
    existing_posting_meta_json: &str,
    source: &PostingSource,
) -> Result<bool, String> {
    let existing_posting_meta: crate::search::run::PostingMeta =
        serde_json::from_str(existing_posting_meta_json).map_err(json_error)?;

    Ok(existing_posting_meta == source.posting_meta)
}

async fn find_posting_by_dedupe(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &NormalizedPosting,
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
            &title,
            &company,
            &locations,
            &posting.title,
            &posting.company,
            &posting.locations,
        ) {
            return Ok(Some(id));
        }
    }

    Ok(None)
}

async fn insert_new_posting(
    transaction: &mut Transaction<'_, Sqlite>,
    posting: &NormalizedPosting,
    result: &SearchRunResult,
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
    .bind(&result.generated_at)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;
    let posting_id = inserted_posting.last_insert_rowid();

    let mut primary_source_id = None;
    for source in &posting.sources {
        let source_id =
            upsert_posting_source(transaction, posting_id, source, &result.generated_at).await?;
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
    posting: &NormalizedPosting,
    result: &SearchRunResult,
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
    .bind(&result.generated_at)
    .bind(posting_id)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;

    for source in &posting.sources {
        upsert_posting_source(transaction, posting_id, source, &result.generated_at).await?;
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

    let posting_meta_json = serde_json::to_string(&source.posting_meta).map_err(json_error)?;

    if let Some(source_id) = existing_source_id {
        sqlx::query(
            "UPDATE job_posting_sources
             SET source_name_snapshot = ?1,
                 posting_meta_json = ?2,
                 last_seen_at = ?3
             WHERE id = ?4",
        )
        .bind(&source.source_name)
        .bind(&posting_meta_json)
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
    .bind(&posting_meta_json)
    .bind(seen_at)
    .execute(&mut **transaction)
    .await
    .map_err(db_error)?;

    Ok(inserted_source.last_insert_rowid())
}

fn merge_locations(mut existing_locations: Vec<String>, new_locations: &[String]) -> Vec<String> {
    let mut existing_location_keys = existing_locations
        .iter()
        .map(|location| normalized_text_key(location))
        .collect::<HashSet<_>>();

    for location in new_locations {
        if existing_location_keys.insert(normalized_text_key(location)) {
            existing_locations.push(location.clone());
        }
    }

    existing_locations
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
