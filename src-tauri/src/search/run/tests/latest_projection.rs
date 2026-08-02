use super::support::*;

#[test]
fn latest_summary_batching_associates_rows_across_the_sqlite_chunk_boundary() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        sqlx::query(
            "WITH RECURSIVE sequence(value) AS (
                 SELECT 1
                 UNION ALL
                 SELECT value + 1 FROM sequence WHERE value < 501
             )
             INSERT INTO search_requests (
                 status, include_rules_json, exclude_rules_json, locations_json, source_keys_json
             )
             SELECT 'draft', '[]', '[]', '[]', '[]' FROM sequence",
        )
        .execute(&pool)
        .await
        .unwrap();

        let ids = sqlx::query_scalar::<_, i64>("SELECT id FROM search_requests ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|id| Id::new(id).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 501);

        sqlx::query(
            "UPDATE search_requests
             SET last_run_at = '2026-01-01T00:00:00Z', last_run_status = 'completed'
             WHERE id = ?1",
        )
        .bind(ids[499].get())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE search_requests
             SET last_run_at = '2026-01-02T00:00:00Z',
                 last_run_status = 'failed',
                 last_run_error = 'second chunk failure'
             WHERE id = ?1",
        )
        .bind(ids[500].get())
        .execute(&pool)
        .await
        .unwrap();

        let summaries = crate::search::run::latest_summaries(&pool, &ids)
            .await
            .unwrap();
        assert_eq!(summaries.len(), 501);

        let first_chunk = summaries.get(&ids[499]).unwrap();
        assert_eq!(first_chunk.at.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(first_chunk.status, Some(SearchRunStatus::Completed));
        assert!(first_chunk.error.is_none());

        let second_chunk = summaries.get(&ids[500]).unwrap();
        assert_eq!(second_chunk.at.as_deref(), Some("2026-01-02T00:00:00Z"));
        assert_eq!(second_chunk.status, Some(SearchRunStatus::Failed));
        assert_eq!(second_chunk.error.as_deref(), Some("second chunk failure"));
    });
}
