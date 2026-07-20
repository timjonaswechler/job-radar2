use super::support::*;

#[test]
fn pre_phase_cancellation_emits_only_the_source_diagnostic() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys =
            write_test_sources(temp_dir.path(), &[("cancel_source", "Cancel Source")]);
        let request =
            create_test_search_request(&pool, source_keys, vec![text_rule("laser")], vec![]).await;
        let cancellation = CancellationToken::new();
        let executor =
            CancellingRuntimeDiscoveryExecutor::new(RuntimeCancellationTiming::BeforePhase);

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .run_with_cancellation(request.id, Some(&cancellation))
        .await
        .unwrap();

        assert_eq!(result.source_runs[0].status, SourceRunStatus::Cancelled);
        assert_eq!(
            result.source_runs[0]
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["source_execution_cancelled"]
        );
    });
}

#[test]
fn active_phase_cancellation_emits_runtime_then_source_diagnostics() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys =
            write_test_sources(temp_dir.path(), &[("cancel_source", "Cancel Source")]);
        let request =
            create_test_search_request(&pool, source_keys, vec![text_rule("laser")], vec![]).await;
        let cancellation = CancellationToken::new();
        let executor =
            CancellingRuntimeDiscoveryExecutor::new(RuntimeCancellationTiming::DuringFetch);

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .run_with_cancellation(request.id, Some(&cancellation))
        .await
        .unwrap();

        assert_eq!(result.source_runs[0].status, SourceRunStatus::Cancelled);
        assert_eq!(result.source_runs[0].candidate_count, 0);
        assert_eq!(
            result.source_runs[0]
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["runtime_execution_cancelled", "source_execution_cancelled"]
        );
    });
}

#[test]
fn active_valid_source_compiles_and_executes_discovery_plan_through_runtime() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys =
            write_test_sources(temp_dir.path(), &[("runtime_source", "Runtime Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = RuntimeDiscoveryExecutor::new(
            json!({
                "jobs": [
                    {
                        "title": "Laser Engineer",
                        "company": "ACME",
                        "url": "https://example.test/laser",
                        "locations": ["Mainz"],
                        "jobId": "runtime-42"
                    },
                    {
                        "title": "Chemist",
                        "company": "ACME",
                        "url": "https://example.test/chemist",
                        "locations": ["Mainz"],
                        "jobId": "runtime-43"
                    }
                ]
            })
            .to_string(),
        );

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
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert!(result.source_runs[0].diagnostics.is_empty());
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer");
        assert_eq!(
            result.postings[0].sources[0].posting_meta["jobId"],
            "runtime-42"
        );

        let row: String = sqlx::query_scalar("SELECT posting_meta_json FROM job_posting_sources")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row, r#"{"jobId":"runtime-42"}"#);
    });
}

#[test]
fn missing_source_key_becomes_failed_source_run_and_valid_keys_continue() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["missing_source".to_string(), source_keys[0].clone()],
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
            )]),
        )]);
        let result_path = temp_dir.path().join("search-run-result.json");

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
        assert_eq!(result.source_runs[0].source_key, "missing_source");
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert!(result.source_runs[0]
            .error
            .as_deref()
            .unwrap()
            .contains("Selected Source `missing_source` was not found"));
        assert_eq!(
            result.source_runs[0].diagnostics[0].code,
            "source_not_found"
        );
        assert_eq!(
            serde_json::to_value(result.source_runs[0].diagnostics[0].category).unwrap(),
            json!("source_validation")
        );
        assert_eq!(result.source_runs[1].source_key, source_keys[0]);
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[1].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].sources[0].source_key, source_keys[0]);

        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(result_path).unwrap()).unwrap();
        assert!(result_json["sourceRuns"][0].get("sourceId").is_none());
        assert!(result_json["postings"][0]["sources"][0]
            .get("sourceId")
            .is_none());
    });
}

#[test]
fn runtime_diagnostics_are_stored_on_failed_source_run() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys =
            write_test_sources(temp_dir.path(), &[("runtime_source", "Runtime Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = RuntimeDiscoveryExecutor::new("{");

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
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(
                |diagnostic| serde_json::to_value(diagnostic.category).unwrap() == json!("runtime")
            ));
        assert!(result.source_runs[0].error.is_some());
    });
}

#[test]
fn invalid_derived_selected_source_fails_with_structured_diagnostics_and_valid_sources_continue() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let mut invalid_source: Value =
            serde_json::from_str(&source_json("invalid_source", "Invalid Source")).unwrap();
        invalid_source
            .as_object_mut()
            .unwrap()
            .remove("sourceSupport");
        write_json(
            temp_dir.path().join("sources/invalid_source.json"),
            &invalid_source.to_string(),
        );
        write_json(
            temp_dir.path().join("sources/valid_source.json"),
            &source_json("valid_source", "Valid Source"),
        );
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["invalid_source".to_string(), "valid_source".to_string()],
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            "valid_source",
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
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

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_source_support"));
        assert!(result.source_runs[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "source_validation_failed"));
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
    });
}

#[test]
fn draft_and_disabled_selected_sources_are_skipped_without_execution() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        write_json(
            temp_dir.path().join("sources/draft_source.json"),
            &source_json_with_status("draft_source", "Draft Source", "draft"),
        );
        write_json(
            temp_dir.path().join("sources/disabled_source.json"),
            &source_json_with_status("disabled_source", "Disabled Source", "disabled"),
        );
        write_json(
            temp_dir.path().join("sources/active_source.json"),
            &source_json("active_source", "Active Source"),
        );
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec![
                    "draft_source".to_string(),
                    "disabled_source".to_string(),
                    "active_source".to_string(),
                ],
            })
            .await
            .unwrap();
        let executor = FixtureSourceExecutor::new([(
            "active_source",
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
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

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Skipped);
        assert_eq!(
            result.source_runs[0].diagnostics[0].code,
            "source_not_active"
        );
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Skipped);
        assert_eq!(
            result.source_runs[1].diagnostics[0].code,
            "source_not_active"
        );
        assert_eq!(result.source_runs[2].status, SourceRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
    });
}

#[test]
fn registry_file_changes_after_run_start_do_not_change_execution_plans() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let temp_dir = tempfile::tempdir().unwrap();
        let app_data_dir = temp_dir.path().join("app-data");
        let profile_path = app_data_dir.join("source-profiles/mutable_profile.json");
        write_json(&profile_path, &mutable_profile_json("initial"));
        write_json(
            app_data_dir.join("sources/first_source.json"),
            &mutable_profile_source_json("first_source", "First Source"),
        );
        write_json(
            app_data_dir.join("sources/second_source.json"),
            &mutable_profile_source_json("second_source", "Second Source"),
        );
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![text_rule("laser")],
                exclude_rules: vec![],
                locations: vec![],
                radius_km: None,
                source_keys: vec!["first_source".to_string(), "second_source".to_string()],
            })
            .await
            .unwrap();
        let executor = RegistryMutatingPlanCaptureExecutor::new(profile_path.clone());

        let result = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            &app_data_dir,
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(
            executor.seen_discovery_markers(),
            vec![
                ("first_source".to_string(), "initial".to_string()),
                ("second_source".to_string(), "initial".to_string()),
            ]
        );
        assert!(std::fs::read_to_string(profile_path)
            .unwrap()
            .contains("changed"));
    });
}
