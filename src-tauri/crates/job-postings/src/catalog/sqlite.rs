use super::{
    queues, ApplicationState, Change, Counts, Error, Id, InterestState, Posting, PreparationState,
    Queue, ReadState, Source,
};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use std::collections::BTreeMap;

const POSTING_COLUMNS: &str = "id, title, company, locations_json, description_text,
    read_state, interest_state, preparation_state, application_state,
    first_seen_at, last_seen_at, created_at, updated_at";

pub(super) async fn list(pool: &SqlitePool, queue: Queue) -> Result<Vec<Posting>, Error> {
    let where_clause = queues::condition(queue)
        .map(|condition| format!("WHERE {condition}"))
        .unwrap_or_default();
    let rows = sqlx::query(&format!(
        "SELECT {POSTING_COLUMNS} FROM job_postings {where_clause}
         ORDER BY last_seen_at DESC, id DESC"
    ))
    .fetch_all(pool)
    .await?;
    hydrate(pool, rows).await
}

pub(super) async fn get(pool: &SqlitePool, id: Id) -> Result<Posting, Error> {
    let row = sqlx::query(&format!(
        "SELECT {POSTING_COLUMNS} FROM job_postings WHERE id = ?1"
    ))
    .bind(id.get())
    .fetch_optional(pool)
    .await?
    .ok_or(Error::NotFound(id))?;
    hydrate(pool, vec![row])
        .await?
        .pop()
        .ok_or(Error::NotFound(id))
}

pub(super) async fn counts(pool: &SqlitePool) -> Result<Counts, Error> {
    validate_primary_occurrences(pool).await?;
    let sql = format!(
        "SELECT
            COUNT(*) AS all_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS archive_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS applied_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS inbox_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS new_inbox_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS review_inbox_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS interested_count,
            COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0) AS preparation_count
         FROM job_postings",
        queues::ARCHIVE,
        queues::APPLIED,
        queues::INBOX,
        queues::NEW_INBOX,
        queues::REVIEW_INBOX,
        queues::INTERESTED,
        queues::PREPARATION,
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    Ok(Counts {
        inbox: row.try_get("inbox_count")?,
        interested: row.try_get("interested_count")?,
        preparation: row.try_get("preparation_count")?,
        applied: row.try_get("applied_count")?,
        archive: row.try_get("archive_count")?,
        all: row.try_get("all_count")?,
        new_inbox: row.try_get("new_inbox_count")?,
        review_inbox: row.try_get("review_inbox_count")?,
    })
}

async fn validate_primary_occurrences(pool: &SqlitePool) -> Result<(), Error> {
    let corrupt = sqlx::query(
        "SELECT posting.id,
                COALESCE(SUM(CASE WHEN source.is_primary = 1 THEN 1 ELSE 0 END), 0) AS primary_count
         FROM job_postings AS posting
         LEFT JOIN job_posting_sources AS source ON source.posting_id = posting.id
         GROUP BY posting.id
         HAVING COALESCE(SUM(CASE WHEN source.is_primary = 1 THEN 1 ELSE 0 END), 0) <> 1
         ORDER BY posting.id
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    if let Some(row) = corrupt {
        let id = Id::new(row.try_get("id")?);
        let count = row.try_get::<i64, _>("primary_count")?;
        return Err(Error::corrupt(
            id,
            format!("expected exactly one primary Posting occurrence, found {count}"),
        ));
    }
    Ok(())
}

pub(super) async fn change(pool: &SqlitePool, id: Id, change: Change) -> Result<Posting, Error> {
    get(pool, id).await?;
    let result = sqlx::query(
        "UPDATE job_postings
         SET read_state = COALESCE(?1, read_state),
             interest_state = COALESCE(?2, interest_state),
             preparation_state = COALESCE(?3, preparation_state),
             application_state = COALESCE(?4, application_state),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?5",
    )
    .bind(change.read_state.map(|state| state.as_str()))
    .bind(change.interest_state.map(|state| state.as_str()))
    .bind(change.preparation_state.map(|state| state.as_str()))
    .bind(change.application_state.map(|state| state.as_str()))
    .bind(id.get())
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(Error::NotFound(id));
    }
    get(pool, id).await
}

pub(super) async fn mark_read(pool: &SqlitePool, id: Id) -> Result<Posting, super::MarkReadError> {
    let result = sqlx::query(
        "UPDATE job_postings
         SET read_state = 'read', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1 AND read_state <> 'read'",
    )
    .bind(id.get())
    .execute(pool)
    .await
    .map_err(Error::from)
    .map_err(super::MarkReadError::Before)?;
    if result.rows_affected() == 0 {
        return get(pool, id).await.map_err(super::MarkReadError::Before);
    }
    get(pool, id).await.map_err(super::MarkReadError::After)
}

/// First-success cache: a concurrent winner is returned rather than overwritten.
pub(super) async fn cache_description(
    pool: &SqlitePool,
    id: Id,
    description: &str,
) -> Result<Posting, Error> {
    let result = sqlx::query(
        "UPDATE job_postings
         SET description_text = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?2 AND description_text IS NULL",
    )
    .bind(description)
    .bind(id.get())
    .execute(pool)
    .await?;
    if result.rows_affected() > 1 {
        return Err(Error::corrupt(
            id,
            "description compare-and-set changed more than one row",
        ));
    }
    get(pool, id).await
}

async fn hydrate(pool: &SqlitePool, rows: Vec<SqliteRow>) -> Result<Vec<Posting>, Error> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids = rows
        .iter()
        .map(|row| row.try_get::<i64, _>("id"))
        .collect::<Result<Vec<_>, _>>()?;
    let ids_json =
        serde_json::to_string(&ids).expect("serializing integer Posting IDs cannot fail");
    let source_rows = sqlx::query(
        "SELECT id, posting_id, source_key, source_name_snapshot, identity_kind,
                identity_value, provider_url, posting_meta_json, is_primary,
                first_seen_at, last_seen_at
         FROM job_posting_sources
         WHERE posting_id IN (SELECT value FROM json_each(?1))
         ORDER BY posting_id, id",
    )
    .bind(ids_json)
    .fetch_all(pool)
    .await?;
    let mut sources_by_posting: BTreeMap<i64, Vec<(Source, bool)>> = BTreeMap::new();
    for row in source_rows {
        let posting_id = row.try_get::<i64, _>("posting_id")?;
        let source = Source {
            id: row.try_get("id")?,
            source_key: row.try_get("source_key")?,
            source_name_snapshot: row.try_get("source_name_snapshot")?,
            url: row.try_get("provider_url")?,
            first_seen_at: row.try_get("first_seen_at")?,
            last_seen_at: row.try_get("last_seen_at")?,
            identity_kind: row.try_get("identity_kind")?,
            identity_value: row.try_get("identity_value")?,
            posting_meta_json: row.try_get("posting_meta_json")?,
        };
        sources_by_posting
            .entry(posting_id)
            .or_default()
            .push((source, row.try_get::<bool, _>("is_primary")?));
    }

    rows.into_iter()
        .map(|row| {
            let id = row.try_get::<i64, _>("id")?;
            posting_from_row(row, sources_by_posting.remove(&id))
        })
        .collect()
}

fn posting_from_row(
    row: SqliteRow,
    hydrated_sources: Option<Vec<(Source, bool)>>,
) -> Result<Posting, Error> {
    let id = Id::new(row.try_get("id")?);
    let hydrated_sources = hydrated_sources.unwrap_or_default();
    let primary = hydrated_sources
        .iter()
        .filter(|(_, is_primary)| *is_primary)
        .map(|(source, _)| source.clone())
        .collect::<Vec<_>>();
    if primary.len() != 1 {
        return Err(Error::corrupt(
            id,
            format!(
                "expected exactly one primary Posting occurrence, found {}",
                primary.len()
            ),
        ));
    }
    let read_state_value = row.try_get::<String, _>("read_state")?;
    let interest_state_value = row.try_get::<String, _>("interest_state")?;
    let preparation_state_value = row.try_get::<String, _>("preparation_state")?;
    let application_state_value = row.try_get::<String, _>("application_state")?;
    let read_state = ReadState::parse(&read_state_value)
        .ok_or_else(|| Error::corrupt(id, format!("unknown read state: {read_state_value}")))?;
    let interest_state = InterestState::parse(&interest_state_value).ok_or_else(|| {
        Error::corrupt(
            id,
            format!("unknown interest state: {interest_state_value}"),
        )
    })?;
    let preparation_state = PreparationState::parse(&preparation_state_value).ok_or_else(|| {
        Error::corrupt(
            id,
            format!("unknown preparation state: {preparation_state_value}"),
        )
    })?;
    let application_state = ApplicationState::parse(&application_state_value).ok_or_else(|| {
        Error::corrupt(
            id,
            format!("unknown application state: {application_state_value}"),
        )
    })?;
    let locations_json = row.try_get::<String, _>("locations_json")?;
    let locations = serde_json::from_str(&locations_json)
        .map_err(|error| Error::corrupt(id, format!("invalid locations: {error}")))?;
    Ok(Posting {
        id,
        title: row.try_get("title")?,
        company: row.try_get("company")?,
        locations,
        description_text: row.try_get("description_text")?,
        read_state,
        interest_state,
        preparation_state,
        application_state,
        primary_queue: queues::primary(interest_state, preparation_state, application_state),
        first_seen_at: row.try_get("first_seen_at")?,
        last_seen_at: row.try_get("last_seen_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        primary_source: primary.into_iter().next().expect("length checked"),
        sources: hydrated_sources
            .into_iter()
            .map(|(source, _)| source)
            .collect(),
    })
}
