use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use super::{
    ApplicationState, InterestState, JobPosting, JobPostingSource, PreparationState, ReadState,
    UpdateJobPostingStateInput,
};

pub struct JobPostingService<'a> {
    pool: &'a SqlitePool,
}

impl<'a> JobPostingService<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<JobPosting>, String> {
        let rows = sqlx::query(
            "SELECT id, title, company, locations_json, primary_source_id,
                    read_state, interest_state, preparation_state, application_state,
                    first_seen_at, last_seen_at, created_at, updated_at
             FROM job_postings
             ORDER BY last_seen_at DESC, id DESC",
        )
        .fetch_all(self.pool)
        .await
        .map_err(db_error)?;

        let mut postings = Vec::with_capacity(rows.len());
        for row in rows {
            postings.push(self.posting_from_row(row).await?);
        }

        Ok(postings)
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
            "SELECT id, title, company, locations_json, primary_source_id,
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
            "SELECT id, source_key, source_name_snapshot, url, first_seen_at, last_seen_at
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

fn source_from_row(row: SqliteRow) -> Result<JobPostingSource, String> {
    Ok(JobPostingSource {
        id: row.try_get("id").map_err(db_error)?,
        source_key: row.try_get("source_key").map_err(db_error)?,
        source_name_snapshot: row.try_get("source_name_snapshot").map_err(db_error)?,
        url: row.try_get("url").map_err(db_error)?,
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
