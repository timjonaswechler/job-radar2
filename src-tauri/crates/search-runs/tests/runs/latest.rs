use search_requests::Id;
use search_runs::{History, Status};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

#[tokio::test]
async fn latest_returns_the_stored_projection_for_one_search_request() {
    let pool = migrated_pool().await;
    let id = insert_request(&pool).await;
    sqlx::query(
        "UPDATE search_requests \
         SET last_run_at = '2026-01-02T03:04:05Z', \
             last_run_status = 'completed_with_errors', \
             last_run_error = 'one Source failed' \
         WHERE id = ?1",
    )
    .bind(id.get())
    .execute(&pool)
    .await
    .unwrap();

    let latest = History::new(pool).latest(id).await.unwrap();

    assert_eq!(latest.at.as_deref(), Some("2026-01-02T03:04:05Z"));
    assert_eq!(latest.status, Some(Status::CompletedWithErrors));
    assert_eq!(latest.error.as_deref(), Some("one Source failed"));
}

#[tokio::test]
async fn latest_defaults_when_the_request_or_its_summary_is_missing() {
    let pool = migrated_pool().await;
    let never_run = insert_request(&pool).await;
    let missing = Id::new(9_999).unwrap();
    let history = History::new(pool);

    assert_eq!(history.latest(never_run).await.unwrap(), Default::default());
    assert_eq!(history.latest(missing).await.unwrap(), Default::default());
}

#[tokio::test]
async fn latest_many_uses_no_storage_for_empty_input() {
    let pool = migrated_pool().await;
    sqlx::query("DROP TABLE search_requests")
        .execute(&pool)
        .await
        .unwrap();

    assert!(History::new(pool)
        .latest_many(&[])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn latest_many_maps_each_request_across_the_five_hundred_id_chunk_boundary() {
    let pool = migrated_pool().await;
    let mut ids = Vec::new();
    for _ in 0..502 {
        ids.push(insert_request(&pool).await);
    }
    for (id, at) in [
        (ids[0], "first"),
        (ids[499], "boundary"),
        (ids[501], "last"),
    ] {
        sqlx::query(
            "UPDATE search_requests SET last_run_at=?1,last_run_status='completed' WHERE id=?2",
        )
        .bind(at)
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    }

    let latest = History::new(pool).latest_many(&ids).await.unwrap();

    assert_eq!(latest.len(), ids.len());
    assert_eq!(latest[&ids[0]].at.as_deref(), Some("first"));
    assert_eq!(latest[&ids[499]].at.as_deref(), Some("boundary"));
    assert_eq!(latest[&ids[500]], Default::default());
    assert_eq!(latest[&ids[501]].at.as_deref(), Some("last"));
}

#[tokio::test]
async fn latest_distinguishes_a_corrupt_status_from_unavailable_storage() {
    let pool = migrated_pool().await;
    let id = insert_request(&pool).await;
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE search_requests SET last_run_status='unknown' WHERE id=?1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    let history = History::new(pool.clone());

    assert!(matches!(
        history.latest(id).await,
        Err(search_runs::HistoryError::CorruptStoredRow { id: Some(found), .. }) if found == id
    ));

    sqlx::query("DROP TABLE search_requests")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        history.latest(id).await,
        Err(search_runs::HistoryError::StorageUnavailable { .. })
    ));
}

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    pool
}

async fn insert_request(pool: &SqlitePool) -> Id {
    let raw = sqlx::query("INSERT INTO search_requests (status) VALUES ('active')")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid();
    Id::new(raw).unwrap()
}
