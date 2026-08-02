use super::support::*;

#[test]
fn only_active_search_requests_can_run_and_non_active_requests_leave_last_run_empty() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let catalog = Catalog::new(pool.clone());
        let service = catalog.clone();

        for status in [Status::Draft, Status::Disabled] {
            let search_request = service
                .create(Input {
                    status,
                    include_rules: vec![text_rule("laser")],
                    exclude_rules: vec![],
                    locations: vec![],
                    radius_km: None,
                    source_keys: vec!["test_source".to_string()],
                })
                .await
                .unwrap();

            let error = catalog
                .begin_execution(search_request.id)
                .await
                .err()
                .unwrap()
                .to_string();

            assert!(error.contains("cannot run unless status is active"));
            let reloaded = crate::search::run::latest_summary(&pool, search_request.id)
                .await
                .unwrap();
            assert!(reloaded.at.is_none());
            assert!(reloaded.status.is_none());
            assert!(reloaded.error.is_none());
        }
    });
}

#[test]
fn execution_admission_uses_the_requests_derived_validation_issues() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let catalog = Catalog::new(pool.clone());
        let service = catalog.clone();
        let request = service
            .create(Input {
                status: Status::Draft,
                include_rules: vec![regex_rule("[")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["test_source".to_string()],
            })
            .await
            .unwrap();
        sqlx::query("UPDATE search_requests SET status = 'active' WHERE id = ?1")
            .bind(request.id.get())
            .execute(&pool)
            .await
            .unwrap();

        let error = catalog
            .begin_execution(request.id)
            .await
            .err()
            .unwrap()
            .to_string();

        assert!(error.contains("invalid_regex at /includeRules/0/value"));
        let reloaded = crate::search::run::latest_summary(&pool, request.id)
            .await
            .unwrap();
        assert!(reloaded.at.is_none());
    });
}

#[test]
fn search_run_holds_execution_lease_at_terminal_persistence_boundary_and_releases_after_return() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let catalog = Catalog::new(pool.clone());
        let temp_dir = tempfile::tempdir().unwrap();
        let request = catalog
            .create(Input {
                status: Status::Active,
                include_rules: vec![text_rule("engineer")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["missing_source".into()],
            })
            .await
            .unwrap();
        let observed = std::sync::Arc::new(std::sync::Mutex::new(None));
        let callback = {
            let catalog = catalog.clone();
            let observed = observed.clone();
            move || {
                let catalog = catalog.clone();
                let result = std::thread::spawn(move || {
                    tauri::async_runtime::block_on(async move { catalog.delete(request.id).await })
                })
                .join()
                .unwrap();
                *observed.lock().unwrap() = Some(result);
            }
        };
        let execution = catalog.begin_execution(request.id).await.unwrap();
        SearchRunService::new_with_result_artifact(
            &pool,
            &fixture_resolution_runtime([("unused", Ok(vec![]))]),
            SearchRunResultArtifact::Disabled,
            sources::installed::Store::new(temp_dir.path()),
        )
        .before_persistence(&callback)
        .run(execution)
        .await
        .unwrap();
        assert!(
            matches!(observed.lock().unwrap().take(), Some(Err(search_requests::Error::Busy { id })) if id == request.id)
        );
        catalog.delete(request.id).await.unwrap();
    });
}

#[test]
fn completed_run_persists_postings_and_records_last_run_success() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let catalog = Catalog::new(pool.clone());
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
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
        )
        .run(admit(&catalog, search_request.id).await)
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

        let reloaded = crate::search::run::latest_summary(&pool, search_request.id)
            .await
            .unwrap();
        assert_eq!(reloaded.at, Some(result.generated_at));
        assert_eq!(reloaded.status, Some(SearchRunStatus::Completed));
        assert!(reloaded.error.is_none());
    });
}
