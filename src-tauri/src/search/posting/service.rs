use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::path::Path;

use crate::{
    declarative::posting_detail::{
        PostingDetailExtractor, PostingDetailHttpClient, PostingDetailSource,
    },
    source::registry::SourceRegistrySnapshot,
};

use super::{
    ApplicationState, InterestState, JobPosting, JobPostingDetail, JobPostingQueueCounts,
    JobPostingQueueId, JobPostingSource, PostingDescriptionState, PreparationState, ReadState,
    UpdateJobPostingStateInput,
};

const ARCHIVE_QUEUE_CONDITION: &str = "(interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))";
const APPLIED_QUEUE_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND application_state IN ('submitted', 'in_process'))";
const INBOX_QUEUE_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const NEW_INBOX_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND read_state = 'unread'
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const REVIEW_INBOX_CONDITION: &str = "(NOT (interest_state = 'dismissed'
    OR application_state IN ('rejected_by_company', 'withdrawn_by_me', 'accepted'))
    AND read_state = 'read'
    AND interest_state = 'undecided'
    AND application_state = 'not_applied')";
const INTERESTED_QUEUE_CONDITION: &str = "(interest_state = 'interested'
    AND preparation_state = 'not_started'
    AND application_state = 'not_applied')";
const PREPARATION_QUEUE_CONDITION: &str = "(interest_state = 'interested'
    AND application_state = 'not_applied'
    AND preparation_state IN ('in_progress', 'ready'))";

pub struct JobPostingService<'a> {
    pool: &'a SqlitePool,
}

impl<'a> JobPostingService<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<JobPosting>, String> {
        self.list_for_queue(JobPostingQueueId::All).await
    }

    pub async fn list_for_queue(
        &self,
        queue_id: JobPostingQueueId,
    ) -> Result<Vec<JobPosting>, String> {
        let where_clause = queue_condition(queue_id)
            .map(|condition| format!("WHERE {condition}"))
            .unwrap_or_default();
        let sql = format!(
            "SELECT id, title, company, locations_json, description_text, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at, created_at, updated_at
             FROM job_postings
             {where_clause}
             ORDER BY last_seen_at DESC, id DESC"
        );
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(db_error)?;

        let mut postings = Vec::with_capacity(rows.len());
        for row in rows {
            postings.push(self.posting_from_row(row).await?);
        }

        Ok(postings)
    }

    pub async fn queue_counts(&self) -> Result<JobPostingQueueCounts, String> {
        let sql = format!(
            "SELECT
                COUNT(*) AS all_count,
                COALESCE(SUM(CASE WHEN {ARCHIVE_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS archive_count,
                COALESCE(SUM(CASE WHEN {APPLIED_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS applied_count,
                COALESCE(SUM(CASE WHEN {INBOX_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS inbox_count,
                COALESCE(SUM(CASE WHEN {NEW_INBOX_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS new_inbox_count,
                COALESCE(SUM(CASE WHEN {REVIEW_INBOX_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS review_inbox_count,
                COALESCE(SUM(CASE WHEN {INTERESTED_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS interested_count,
                COALESCE(SUM(CASE WHEN {PREPARATION_QUEUE_CONDITION}
                    THEN 1 ELSE 0 END), 0) AS preparation_count
             FROM job_postings"
        );
        let row = sqlx::query(&sql)
            .fetch_one(self.pool)
            .await
            .map_err(db_error)?;

        Ok(JobPostingQueueCounts {
            inbox: row.try_get("inbox_count").map_err(db_error)?,
            interested: row.try_get("interested_count").map_err(db_error)?,
            preparation: row.try_get("preparation_count").map_err(db_error)?,
            applied: row.try_get("applied_count").map_err(db_error)?,
            archive: row.try_get("archive_count").map_err(db_error)?,
            all: row.try_get("all_count").map_err(db_error)?,
            new_inbox: row.try_get("new_inbox_count").map_err(db_error)?,
            review_inbox: row.try_get("review_inbox_count").map_err(db_error)?,
        })
    }

    pub async fn get_posting_detail(
        &self,
        id: i64,
        app_data_dir: impl AsRef<Path>,
    ) -> Result<JobPostingDetail, String> {
        let snapshot = crate::source::registry::load_snapshot(app_data_dir);
        let extractor = PostingDetailExtractor::new_reqwest();
        self.get_posting_detail_with_extractor(id, &snapshot, &extractor)
            .await
    }

    pub(crate) async fn get_posting_detail_with_extractor<C>(
        &self,
        id: i64,
        snapshot: &SourceRegistrySnapshot,
        extractor: &PostingDetailExtractor<C>,
    ) -> Result<JobPostingDetail, String>
    where
        C: PostingDetailHttpClient + Send + Sync,
    {
        let mut posting = self.get(id).await?;
        if posting.read_state != ReadState::Read {
            sqlx::query(
                "UPDATE job_postings
                 SET read_state = 'read',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1",
            )
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(db_error)?;
            posting = self.get(id).await?;
        }

        if let Some(text) = posting.description_text.clone() {
            return Ok(JobPostingDetail {
                posting,
                description_state: PostingDescriptionState::Loaded { text },
            });
        }

        let candidates = detail_capable_sources(&posting, snapshot);
        if candidates.is_empty() {
            return Ok(JobPostingDetail {
                posting,
                description_state: PostingDescriptionState::Unsupported {
                    message: format!(
                        "job posting {id} has no stored source with postingDetail extraction"
                    ),
                },
            });
        }

        let mut failures = Vec::new();
        for (posting_source, execution_plan) in candidates {
            let posting_meta = if posting_source.posting_meta.is_empty() {
                None
            } else {
                Some(&posting_source.posting_meta)
            };
            match extractor
                .load_source_description_text(
                    &execution_plan,
                    PostingDetailSource {
                        source_key: &posting_source.source_key,
                        url: &posting_source.url,
                        posting_meta,
                    },
                )
                .await
            {
                Ok(detail) => {
                    sqlx::query(
                        "UPDATE job_postings
                         SET description_text = ?1,
                             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                         WHERE id = ?2",
                    )
                    .bind(&detail.description_text)
                    .bind(id)
                    .execute(self.pool)
                    .await
                    .map_err(db_error)?;
                    let posting = self.get(id).await?;
                    return Ok(JobPostingDetail {
                        posting,
                        description_state: PostingDescriptionState::Loaded {
                            text: detail.description_text,
                        },
                    });
                }
                Err(error) => failures.push(format!(
                    "{} ({}) failed: {}",
                    posting_source.source_key, posting_source.url, error
                )),
            }
        }

        Ok(JobPostingDetail {
            posting,
            description_state: PostingDescriptionState::Failed {
                message: format!(
                    "description loading failed for all detail-capable sources: {}",
                    failures.join("; ")
                ),
            },
        })
    }

    pub async fn update_state(
        &self,
        id: i64,
        input: UpdateJobPostingStateInput,
    ) -> Result<JobPosting, String> {
        if input.read_state.is_none()
            && input.interest_state.is_none()
            && input.preparation_state.is_none()
            && input.application_state.is_none()
        {
            return Err("no state fields supplied".to_string());
        }

        let current = self.get(id).await?;
        let read_state = input.read_state.unwrap_or(current.read_state);
        let interest_state = input.interest_state.unwrap_or(current.interest_state);
        let preparation_state = input.preparation_state.unwrap_or(current.preparation_state);
        let application_state = input.application_state.unwrap_or(current.application_state);

        sqlx::query(
            "UPDATE job_postings
             SET read_state = ?1,
                 interest_state = ?2,
                 preparation_state = ?3,
                 application_state = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?5",
        )
        .bind(read_state.as_str())
        .bind(interest_state.as_str())
        .bind(preparation_state.as_str())
        .bind(application_state.as_str())
        .bind(id)
        .execute(self.pool)
        .await
        .map_err(db_error)?;

        self.get(id).await
    }

    async fn get(&self, id: i64) -> Result<JobPosting, String> {
        let row = sqlx::query(
            "SELECT id, title, company, locations_json, description_text, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at, created_at, updated_at
             FROM job_postings
             WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(db_error)?;

        row.map(|row| self.posting_from_row(row))
            .ok_or_else(|| format!("job posting {id} not found"))?
            .await
    }

    async fn posting_from_row(&self, row: SqliteRow) -> Result<JobPosting, String> {
        let posting_id = row.try_get::<i64, _>("id").map_err(db_error)?;
        let primary_source_id = row
            .try_get::<Option<i64>, _>("primary_source_id")
            .map_err(db_error)?;
        let sources = self.sources_for_posting(posting_id).await?;
        let primary_source =
            primary_source_id.and_then(|id| sources.iter().find(|source| source.id == id).cloned());
        let read_state = row.try_get::<String, _>("read_state").map_err(db_error)?;
        let interest_state = row
            .try_get::<String, _>("interest_state")
            .map_err(db_error)?;
        let preparation_state = row
            .try_get::<String, _>("preparation_state")
            .map_err(db_error)?;
        let application_state = row
            .try_get::<String, _>("application_state")
            .map_err(db_error)?;

        Ok(JobPosting {
            id: posting_id,
            title: row.try_get("title").map_err(db_error)?,
            company: row.try_get("company").map_err(db_error)?,
            locations: locations_from_json(
                posting_id,
                &row.try_get::<String, _>("locations_json")
                    .map_err(db_error)?,
            )?,
            description_text: row.try_get("description_text").map_err(db_error)?,
            read_state: ReadState::try_from(read_state.as_str())?,
            interest_state: InterestState::try_from(interest_state.as_str())?,
            preparation_state: PreparationState::try_from(preparation_state.as_str())?,
            application_state: ApplicationState::try_from(application_state.as_str())?,
            first_seen_at: row.try_get("first_seen_at").map_err(db_error)?,
            last_seen_at: row.try_get("last_seen_at").map_err(db_error)?,
            created_at: row.try_get("created_at").map_err(db_error)?,
            updated_at: row.try_get("updated_at").map_err(db_error)?,
            primary_source,
            sources,
        })
    }

    async fn sources_for_posting(&self, posting_id: i64) -> Result<Vec<JobPostingSource>, String> {
        let rows = sqlx::query(
            "SELECT id, source_key, source_name_snapshot, url, posting_meta_json,
                    first_seen_at, last_seen_at
             FROM job_posting_sources
             WHERE posting_id = ?1
             ORDER BY id",
        )
        .bind(posting_id)
        .fetch_all(self.pool)
        .await
        .map_err(db_error)?;

        rows.into_iter().map(source_from_row).collect()
    }
}

fn queue_condition(queue_id: JobPostingQueueId) -> Option<&'static str> {
    match queue_id {
        JobPostingQueueId::All => None,
        JobPostingQueueId::Archive => Some(ARCHIVE_QUEUE_CONDITION),
        JobPostingQueueId::Applied => Some(APPLIED_QUEUE_CONDITION),
        JobPostingQueueId::Inbox => Some(INBOX_QUEUE_CONDITION),
        JobPostingQueueId::Interested => Some(INTERESTED_QUEUE_CONDITION),
        JobPostingQueueId::Preparation => Some(PREPARATION_QUEUE_CONDITION),
    }
}

fn detail_capable_sources(
    posting: &JobPosting,
    snapshot: &SourceRegistrySnapshot,
) -> Vec<(
    JobPostingSource,
    crate::source::registry::ResolvedSourceExecutionPlan,
)> {
    let mut candidates = Vec::new();
    if let Some(primary_source) = &posting.primary_source {
        push_detail_capable_source(&mut candidates, primary_source, snapshot);
    }

    for source in &posting.sources {
        if posting
            .primary_source
            .as_ref()
            .is_some_and(|primary_source| primary_source.id == source.id)
        {
            continue;
        }
        push_detail_capable_source(&mut candidates, source, snapshot);
    }

    candidates
}

fn push_detail_capable_source(
    candidates: &mut Vec<(
        JobPostingSource,
        crate::source::registry::ResolvedSourceExecutionPlan,
    )>,
    source: &JobPostingSource,
    snapshot: &SourceRegistrySnapshot,
) {
    if let Ok(execution_plan) = snapshot.resolve_source(&source.source_key) {
        if execution_plan.posting_detail().is_some() {
            candidates.push((source.clone(), execution_plan));
        }
    }
}

fn source_from_row(row: SqliteRow) -> Result<JobPostingSource, String> {
    let id = row.try_get("id").map_err(db_error)?;
    let posting_meta_json = row
        .try_get::<String, _>("posting_meta_json")
        .map_err(db_error)?;
    let posting_meta = serde_json::from_str(&posting_meta_json).map_err(|error| {
        format!("invalid posting_meta_json for job posting source {id}: {error}")
    })?;

    Ok(JobPostingSource {
        id,
        source_key: row.try_get("source_key").map_err(db_error)?,
        source_name_snapshot: row.try_get("source_name_snapshot").map_err(db_error)?,
        url: row.try_get("url").map_err(db_error)?,
        posting_meta,
        first_seen_at: row.try_get("first_seen_at").map_err(db_error)?,
        last_seen_at: row.try_get("last_seen_at").map_err(db_error)?,
    })
}

fn locations_from_json(posting_id: i64, json: &str) -> Result<Vec<String>, String> {
    serde_json::from_str(json)
        .map_err(|error| format!("invalid locations_json for job posting {posting_id}: {error}"))
}

fn db_error(error: sqlx::Error) -> String {
    error.to_string()
}
