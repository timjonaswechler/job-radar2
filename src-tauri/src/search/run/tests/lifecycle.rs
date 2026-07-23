use super::support::*;

#[test]
fn only_active_search_requests_can_run_and_non_active_requests_leave_last_run_empty() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);
        let temp_dir = tempfile::tempdir().unwrap();
        let executor = fixture_resolution_runtime([("test_source", Ok(vec![]))]);

        for status in [
            SearchRequestStatus::Draft,
            SearchRequestStatus::Disabled,
            SearchRequestStatus::Invalid,
        ] {
            let search_request = service
                .create(CreateSearchRequestInput {
                    status,
                    include_rules: vec![text_rule("laser")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_keys: vec!["test_source".to_string()],
                })
                .await
                .unwrap();

            let error = SearchRunService::new(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir
                    .path()
                    .join(format!("{:?}-search-run-result.json", status)),
                temp_dir.path(),
            )
            .run(search_request.id)
            .await
            .unwrap_err();

            assert!(error.contains("cannot run unless status is active"));
            let reloaded = service.get(search_request.id).await.unwrap();
            assert!(reloaded.last_run_at.is_none());
            assert!(reloaded.last_run_status.is_none());
            assert!(reloaded.last_run_error.is_none());
        }
    });
}

#[test]
fn completed_run_persists_postings_and_records_last_run_success() {
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
        let executor = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &["Mainz"],
            )]),
        )]);

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

        assert_eq!(result.status, SearchRunStatus::Completed);
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
        let source_url: String = sqlx::query_scalar("SELECT url FROM job_posting_sources")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(source_url, "https://example.test/laser");

        let reloaded = SearchRequestService::new(&pool, &running_search_runs)
            .get(search_request.id)
            .await
            .unwrap();
        assert_eq!(reloaded.last_run_at, Some(result.generated_at));
        assert_eq!(reloaded.last_run_status, Some(SearchRunStatus::Completed));
        assert!(reloaded.last_run_error.is_none());
    });
}
