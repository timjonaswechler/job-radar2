use super::support::*;

#[test]
fn partial_source_failure_completes_with_errors_and_records_failed_source_error() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let result_path = temp_dir.path().join("search-run-result.json");
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Laser Engineer",
                    "ACME",
                    "https://source-one.test/laser",
                    &["Mainz"],
                )]),
            ),
            (
                source_keys[1].clone(),
                Err(SourceExecutionError::Failed(
                    "Candidate Resolution failed: DiscoveryExecution".to_string(),
                )),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            result_path.clone(),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Failed);
        assert_eq!(
            result.source_runs[1].error.as_deref(),
            Some("Candidate Resolution failed: DiscoveryExecution")
        );
        let posting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(posting_count, 1);
        let reloaded = SearchRequestService::new(&pool, &running_search_runs)
            .get(search_request.id)
            .await
            .unwrap();
        assert_eq!(reloaded.last_run_at, Some(result.generated_at.clone()));
        assert_eq!(
            reloaded.last_run_status,
            Some(SearchRunStatus::CompletedWithErrors)
        );
        let last_run_error = reloaded.last_run_error.unwrap();
        assert!(last_run_error.contains("source_two"));
        assert!(last_run_error.contains("Candidate Resolution failed: DiscoveryExecution"));

        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(result_path).unwrap()).unwrap();
        assert_eq!(result_json["status"], "completed_with_errors");
        assert_eq!(
            result_json["sourceRuns"][1]["error"],
            "Candidate Resolution failed: DiscoveryExecution"
        );
    });
}

#[test]
fn total_source_failure_produces_failed_result_without_postings() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[("source_one", "Source One"), ("source_two", "Source Two")],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let executor = fixture_resolution_runtime([
            (
                source_keys[0].clone(),
                Err(SourceExecutionError::Failed("first failed".to_string())),
            ),
            (
                source_keys[1].clone(),
                Err(SourceExecutionError::Failed("second failed".to_string())),
            ),
        ]);
        let running_search_runs = RunningSearchRuns::default();

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Failed);
        assert!(result.postings.is_empty());
        assert!(result
            .source_runs
            .iter()
            .all(|source_run| source_run.status == SourceRunStatus::Failed));
        let posting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(posting_count, 0);
        let reloaded = SearchRequestService::new(&pool, &running_search_runs)
            .get(search_request.id)
            .await
            .unwrap();
        assert_eq!(reloaded.last_run_at, Some(result.generated_at));
        assert_eq!(reloaded.last_run_status, Some(SearchRunStatus::Failed));
        let last_run_error = reloaded.last_run_error.unwrap();
        assert!(last_run_error.contains("source_one"));
        assert!(last_run_error.contains("Candidate Resolution failed"));
        assert!(last_run_error.contains("source_two"));
    });
}

#[test]
fn persistence_failure_rolls_back_last_run_update() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let inserted_posting = sqlx::query(
            "INSERT INTO job_postings (
               title, company, locations_json, first_seen_at, last_seen_at
             )
             VALUES ('Existing Laser Engineer', 'ACME', '{}', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO job_posting_sources (
               posting_id, source_key, source_name_snapshot, url, first_seen_at, last_seen_at
             )
             VALUES (?1, 'test_source', 'Test Source', 'https://example.test/laser', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')",
        )
        .bind(inserted_posting.last_insert_rowid())
        .execute(&pool)
        .await
        .unwrap();
        let executor = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &["Mainz"],
            )]),
        )]);

        let artifact_path = temp_dir.path().join("search-run-result.json");
        let error = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            artifact_path.clone(),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap_err();

        assert!(error.contains("invalid type"));
        let posting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_postings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(posting_count, 1);
        let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job_posting_sources")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(source_count, 1);
        let reloaded = SearchRequestService::new(&pool, &running_search_runs)
            .get(search_request.id)
            .await
            .unwrap();
        assert!(reloaded.last_run_at.is_none());
        assert!(reloaded.last_run_status.is_none());
        assert!(reloaded.last_run_error.is_none());
        assert!(
            !artifact_path.exists(),
            "DB failure must not write an artifact"
        );
    });
}
