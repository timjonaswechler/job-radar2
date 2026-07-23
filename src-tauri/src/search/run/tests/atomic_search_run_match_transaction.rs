use super::support::*;
use crate::search::run::{
    persist_atomic_search_run, AtomicSearchRunInput, NormalizedPosting, PostingSource,
    SearchRunStatus,
};

#[test]
fn atomic_search_run_match_transaction_schema_has_exact_relationships() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;

        let search_run_columns = sqlx::query("PRAGMA table_info(search_runs)")
            .fetch_all(&pool)
            .await
            .unwrap();
        let search_run_column_names = search_run_columns
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert_eq!(
            search_run_column_names,
            [
                "id",
                "search_request_id",
                "status",
                "generated_at",
                "created_at"
            ]
        );
        for required in ["search_request_id", "status", "generated_at", "created_at"] {
            assert_eq!(
                search_run_columns
                    .iter()
                    .find(|row| row.get::<String, _>("name") == required)
                    .unwrap()
                    .get::<i64, _>("notnull"),
                1,
                "search_runs.{required} must be required"
            );
        }
        assert_eq!(
            search_run_columns
                .iter()
                .find(|row| row.get::<String, _>("name") == "created_at")
                .unwrap()
                .get::<Option<String>, _>("dflt_value"),
            Some("strftime('%Y-%m-%dT%H:%M:%fZ', 'now')".to_string())
        );
        let search_run_id = search_run_columns
            .iter()
            .find(|row| row.get::<String, _>("name") == "id")
            .unwrap();
        assert_eq!(search_run_id.get::<i64, _>("pk"), 1);
        assert_eq!(search_run_id.get::<String, _>("type"), "INTEGER");
        let search_runs_sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_runs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(search_runs_sql.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(search_runs_sql.contains(
            "CHECK(status IN('completed', 'completed_with_errors', 'failed', 'cancelled'))"
        ));

        let match_columns = sqlx::query("PRAGMA table_info(matches)")
            .fetch_all(&pool)
            .await
            .unwrap();
        let match_column_names = match_columns
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert_eq!(
            match_column_names,
            ["id", "search_run_id", "job_posting_id", "created_at"]
        );
        for required in ["search_run_id", "job_posting_id", "created_at"] {
            assert_eq!(
                match_columns
                    .iter()
                    .find(|row| row.get::<String, _>("name") == required)
                    .unwrap()
                    .get::<i64, _>("notnull"),
                1,
                "matches.{required} must be required"
            );
        }
        assert_eq!(
            match_columns
                .iter()
                .find(|row| row.get::<String, _>("name") == "created_at")
                .unwrap()
                .get::<Option<String>, _>("dflt_value"),
            Some("strftime('%Y-%m-%dT%H:%M:%fZ', 'now')".to_string())
        );
        let match_id = match_columns
            .iter()
            .find(|row| row.get::<String, _>("name") == "id")
            .unwrap();
        assert_eq!(match_id.get::<i64, _>("pk"), 1);
        assert_eq!(match_id.get::<String, _>("type"), "INTEGER");
        let matches_sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'matches'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(matches_sql.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"));

        let search_run_foreign_keys = sqlx::query("PRAGMA foreign_key_list(search_runs)")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(search_run_foreign_keys.len(), 1);
        assert_eq!(
            search_run_foreign_keys[0].get::<String, _>("table"),
            "search_requests"
        );
        assert_eq!(
            search_run_foreign_keys[0].get::<String, _>("from"),
            "search_request_id"
        );
        assert_eq!(search_run_foreign_keys[0].get::<String, _>("to"), "id");
        assert_eq!(
            search_run_foreign_keys[0].get::<String, _>("on_delete"),
            "CASCADE"
        );

        let match_foreign_keys = sqlx::query("PRAGMA foreign_key_list(matches)")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(match_foreign_keys.len(), 2);
        assert!(match_foreign_keys.iter().any(|row| {
            row.get::<String, _>("table") == "search_runs"
                && row.get::<String, _>("from") == "search_run_id"
                && row.get::<String, _>("to") == "id"
                && row.get::<String, _>("on_delete") == "CASCADE"
        }));
        assert!(match_foreign_keys.iter().any(|row| {
            row.get::<String, _>("table") == "job_postings"
                && row.get::<String, _>("from") == "job_posting_id"
                && row.get::<String, _>("to") == "id"
                && row.get::<String, _>("on_delete") == "CASCADE"
        }));

        let search_run_indexes = sqlx::query("PRAGMA index_list(search_runs)")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(search_run_indexes
            .iter()
            .any(|row| row.get::<String, _>("name") == "idx_search_runs_search_request_id"));
        let search_run_index_columns =
            sqlx::query("PRAGMA index_info('idx_search_runs_search_request_id')")
                .fetch_all(&pool)
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.get::<String, _>("name"))
                .collect::<Vec<_>>();
        assert_eq!(search_run_index_columns, ["search_request_id"]);

        let match_indexes = sqlx::query("PRAGMA index_list(matches)")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(match_indexes
            .iter()
            .any(|row| row.get::<String, _>("name") == "idx_matches_job_posting_id"));
        let posting_index_columns = sqlx::query("PRAGMA index_info('idx_matches_job_posting_id')")
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert_eq!(posting_index_columns, ["job_posting_id"]);
        let unique_match_index = match_indexes
            .iter()
            .find(|row| row.get::<i64, _>("unique") == 1)
            .unwrap()
            .get::<String, _>("name");
        let unique_match_columns =
            sqlx::query(&format!("PRAGMA index_info('{unique_match_index}')"))
                .fetch_all(&pool)
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.get::<String, _>("name"))
                .collect::<Vec<_>>();
        assert_eq!(unique_match_columns, ["search_run_id", "job_posting_id"]);
    });
}

#[test]
fn atomic_search_run_match_transaction_persists_successful_terminal_run() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let postings = vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![
                source("source_a", "Source A", "https://a.example/jobs/laser"),
                source("source_b", "Source B", "https://b.example/jobs/laser"),
            ],
        )];

        let search_run_id = persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:00:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap();

        let run = sqlx::query(
            "SELECT search_request_id, status, generated_at FROM search_runs WHERE id = ?1",
        )
        .bind(search_run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(run.get::<i64, _>("search_request_id"), search_request_id);
        assert_eq!(run.get::<String, _>("status"), "completed");
        assert_eq!(
            run.get::<String, _>("generated_at"),
            "2026-07-23T12:00:00.000Z"
        );

        let posting_id: i64 = sqlx::query_scalar("SELECT id FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        let matched_posting_id: i64 =
            sqlx::query_scalar("SELECT job_posting_id FROM matches WHERE search_run_id = ?1")
                .bind(search_run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(matched_posting_id, posting_id);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 2);

        let metadata = sqlx::query(
            "SELECT last_run_at, last_run_status, last_run_error
             FROM search_requests WHERE id = ?1",
        )
        .bind(search_request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            metadata.get::<String, _>("last_run_at"),
            "2026-07-23T12:00:00.000Z"
        );
        assert_eq!(metadata.get::<String, _>("last_run_status"), "completed");
        assert_eq!(metadata.get::<Option<String>, _>("last_run_error"), None);
    });
}

#[test]
fn atomic_search_run_match_transaction_accepts_completed_with_errors() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let postings = vec![posting(
            "Data Engineer",
            "ACME GmbH",
            &["Berlin"],
            vec![source(
                "source_a",
                "Source A",
                "https://a.example/jobs/data",
            )],
        )];

        persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::CompletedWithErrors,
                generated_at: "2026-07-23T12:01:00.000Z",
                last_run_error: Some("one Source failed"),
                postings: &postings,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM search_runs")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "completed_with_errors"
        );
        assert_eq!(table_count(&pool, "matches").await, 1);
        assert_eq!(
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT last_run_error FROM search_requests WHERE id = ?1"
            )
            .bind(search_request_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            Some("one Source failed".to_string())
        );
    });
}

#[test]
fn atomic_search_run_match_transaction_failed_and_cancelled_exclude_postings() {
    tauri::async_runtime::block_on(async {
        for status in [SearchRunStatus::Failed, SearchRunStatus::Cancelled] {
            let pool = migrated_pool().await;
            let search_request_id = insert_search_request(&pool).await;

            persist_atomic_search_run(
                &pool,
                AtomicSearchRunInput {
                    search_request_id,
                    status,
                    generated_at: "2026-07-23T12:02:00.000Z",
                    last_run_error: Some("terminal failure"),
                    postings: &[],
                },
            )
            .await
            .unwrap();

            assert_eq!(table_count(&pool, "search_runs").await, 1);
            assert_eq!(table_count(&pool, "job_postings").await, 0);
            assert_eq!(table_count(&pool, "job_posting_sources").await, 0);
            assert_eq!(table_count(&pool, "matches").await, 0);

            let invalid_postings = vec![posting(
                "Forbidden Posting",
                "ACME GmbH",
                &[],
                vec![source(
                    "source_a",
                    "Source A",
                    "https://a.example/jobs/forbidden",
                )],
            )];
            let error = persist_atomic_search_run(
                &pool,
                AtomicSearchRunInput {
                    search_request_id,
                    status,
                    generated_at: "2026-07-23T12:03:00.000Z",
                    last_run_error: Some("must not replace metadata"),
                    postings: &invalid_postings,
                },
            )
            .await
            .unwrap_err();
            assert!(error.contains("cannot persist posting or Match input"));
            assert_eq!(table_count(&pool, "search_runs").await, 1);
            assert_eq!(table_count(&pool, "job_postings").await, 0);
            assert_eq!(table_count(&pool, "matches").await, 0);
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT last_run_at FROM search_requests WHERE id = ?1"
                )
                .bind(search_request_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                "2026-07-23T12:02:00.000Z"
            );
        }
    });
}

#[test]
fn atomic_search_run_match_transaction_duplicate_match_rolls_back_everything() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let duplicate = posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source(
                "source_a",
                "Source A",
                "https://a.example/jobs/laser",
            )],
        );
        let postings = vec![duplicate.clone(), duplicate];

        let error = persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:04:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap_err();

        assert!(error.contains("UNIQUE constraint failed: matches.search_run_id"));
        for table in [
            "search_runs",
            "job_postings",
            "job_posting_sources",
            "matches",
        ] {
            assert_eq!(
                table_count(&pool, table).await,
                0,
                "{table} was not rolled back"
            );
        }
        let metadata = sqlx::query(
            "SELECT last_run_at, last_run_status, last_run_error
             FROM search_requests WHERE id = ?1",
        )
        .bind(search_request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(metadata.get::<Option<String>, _>("last_run_at"), None);
        assert_eq!(metadata.get::<Option<String>, _>("last_run_status"), None);
        assert_eq!(metadata.get::<Option<String>, _>("last_run_error"), None);
    });
}

#[test]
fn atomic_search_run_match_transaction_rerun_reuses_durable_posting() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let postings = vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source(
                "source_a",
                "Source A",
                "https://a.example/jobs/laser",
            )],
        )];

        persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:05:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "UPDATE job_postings
             SET read_state = 'read', interest_state = 'interested',
                 preparation_state = 'in_progress', application_state = 'submitted'",
        )
        .execute(&pool)
        .await
        .unwrap();
        persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-24T12:05:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap();

        assert_eq!(table_count(&pool, "search_runs").await, 2);
        assert_eq!(table_count(&pool, "matches").await, 2);
        assert_eq!(table_count(&pool, "job_postings").await, 1);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT job_posting_id) FROM matches")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        let manual_states = sqlx::query(
            "SELECT read_state, interest_state, preparation_state, application_state
             FROM job_postings",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(manual_states.get::<String, _>("read_state"), "read");
        assert_eq!(
            manual_states.get::<String, _>("interest_state"),
            "interested"
        );
        assert_eq!(
            manual_states.get::<String, _>("preparation_state"),
            "in_progress"
        );
        assert_eq!(
            manual_states.get::<String, _>("application_state"),
            "submitted"
        );
    });
}

#[test]
fn atomic_search_run_match_transaction_validation_and_metadata_failures_roll_back() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let postings = vec![
            posting(
                "Valid Posting",
                "ACME GmbH",
                &[],
                vec![source(
                    "source_a",
                    "Source A",
                    "https://a.example/jobs/valid",
                )],
            ),
            posting(
                " ",
                "ACME GmbH",
                &[],
                vec![source(
                    "source_a",
                    "Source A",
                    "https://a.example/jobs/invalid",
                )],
            ),
        ];
        let error = persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:06:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap_err();
        assert!(error.contains("posting title is empty"));
        assert_eq!(table_count(&pool, "search_runs").await, 0);
        assert_eq!(table_count(&pool, "job_postings").await, 0);

        sqlx::query(
            "CREATE TRIGGER reject_last_run_update
             BEFORE UPDATE OF last_run_at ON search_requests
             BEGIN
               SELECT RAISE(ABORT, 'metadata update rejected');
             END",
        )
        .execute(&pool)
        .await
        .unwrap();
        let valid_postings = vec![posting(
            "Valid Posting",
            "ACME GmbH",
            &[],
            vec![source(
                "source_a",
                "Source A",
                "https://a.example/jobs/valid",
            )],
        )];
        let error = persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:07:00.000Z",
                last_run_error: None,
                postings: &valid_postings,
            },
        )
        .await
        .unwrap_err();
        assert!(error.contains("metadata update rejected"));
        for table in [
            "search_runs",
            "job_postings",
            "job_posting_sources",
            "matches",
        ] {
            assert_eq!(
                table_count(&pool, table).await,
                0,
                "{table} was not rolled back"
            );
        }
    });
}

#[test]
fn atomic_search_run_match_transaction_enforces_parents_and_cascades() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let missing_request_error = persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id: 999,
                status: SearchRunStatus::Failed,
                generated_at: "2026-07-23T12:08:00.000Z",
                last_run_error: Some("missing request"),
                postings: &[],
            },
        )
        .await
        .unwrap_err();
        assert!(missing_request_error.contains("FOREIGN KEY constraint failed"));
        assert_eq!(table_count(&pool, "search_runs").await, 0);

        let search_request_id = insert_search_request(&pool).await;
        let postings = vec![posting(
            "Laser Engineer",
            "ACME GmbH",
            &["Mainz"],
            vec![source(
                "source_a",
                "Source A",
                "https://a.example/jobs/laser",
            )],
        )];
        persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:09:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap();

        sqlx::query("DELETE FROM search_requests WHERE id = ?1")
            .bind(search_request_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(table_count(&pool, "search_runs").await, 0);
        assert_eq!(table_count(&pool, "matches").await, 0);
        assert_eq!(table_count(&pool, "job_postings").await, 1);
        assert_eq!(table_count(&pool, "job_posting_sources").await, 1);

        let second_request_id = insert_search_request(&pool).await;
        persist_atomic_search_run(
            &pool,
            AtomicSearchRunInput {
                search_request_id: second_request_id,
                status: SearchRunStatus::Completed,
                generated_at: "2026-07-23T12:10:00.000Z",
                last_run_error: None,
                postings: &postings,
            },
        )
        .await
        .unwrap();
        let posting_id: i64 = sqlx::query_scalar("SELECT id FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM job_postings WHERE id = ?1")
            .bind(posting_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(table_count(&pool, "matches").await, 0);
        assert_eq!(table_count(&pool, "search_runs").await, 1);
    });
}

#[test]
fn atomic_search_run_match_transaction_database_rejects_unknown_status_and_missing_match_parents() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let search_request_id = insert_search_request(&pool).await;
        let unknown_status = sqlx::query(
            "INSERT INTO search_runs (search_request_id, status, generated_at)
             VALUES (?1, 'unknown', '2026-07-23T12:11:00.000Z')",
        )
        .bind(search_request_id)
        .execute(&pool)
        .await
        .unwrap_err();
        assert!(unknown_status
            .to_string()
            .contains("CHECK constraint failed"));

        let search_run_id = sqlx::query(
            "INSERT INTO search_runs (search_request_id, status, generated_at)
             VALUES (?1, 'failed', '2026-07-23T12:12:00.000Z')",
        )
        .bind(search_request_id)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let missing_posting =
            sqlx::query("INSERT INTO matches (search_run_id, job_posting_id) VALUES (?1, 999)")
                .bind(search_run_id)
                .execute(&pool)
                .await
                .unwrap_err();
        assert!(missing_posting
            .to_string()
            .contains("FOREIGN KEY constraint failed"));

        let posting_id = sqlx::query(
            "INSERT INTO job_postings (title, company) VALUES ('Laser Engineer', 'ACME GmbH')",
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let missing_run =
            sqlx::query("INSERT INTO matches (search_run_id, job_posting_id) VALUES (999, ?1)")
                .bind(posting_id)
                .execute(&pool)
                .await
                .unwrap_err();
        assert!(missing_run
            .to_string()
            .contains("FOREIGN KEY constraint failed"));
    });
}

async fn insert_search_request(pool: &SqlitePool) -> i64 {
    sqlx::query("INSERT INTO search_requests (status) VALUES ('active')")
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

fn posting(
    title: &str,
    company: &str,
    locations: &[&str],
    sources: Vec<PostingSource>,
) -> NormalizedPosting {
    NormalizedPosting {
        title: title.to_string(),
        company: company.to_string(),
        url: sources
            .first()
            .map(|source| source.url.clone())
            .unwrap_or_default(),
        locations: locations.iter().map(|value| (*value).to_string()).collect(),
        sources,
    }
}

fn source(source_key: &str, source_name: &str, url: &str) -> PostingSource {
    PostingSource {
        source_key: source_key.to_string(),
        source_name: source_name.to_string(),
        url: url.to_string(),
    }
}

async fn table_count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}
