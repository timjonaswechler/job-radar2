use super::support::*;

#[test]
fn scheduled_search_run_preserves_source_outcomes_and_structured_diagnostics() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[
                ("valid_source", "Valid Source"),
                ("failing_source", "Failing Source"),
            ],
        );
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let running_search_runs = std::sync::Arc::new(RunningSearchRuns::default());
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Laser Engineer",
                    "ACME",
                    "https://example.test/laser",
                    &[],
                )]),
            ),
            (
                source_keys[1].clone(),
                Err(SourceExecutionError::FailedWithDiagnostics {
                    message: "fixture runtime failure".to_string(),
                    diagnostics: vec![crate::profile_dsl::diagnostics::Diagnostic {
                        category: crate::profile_dsl::diagnostics::DiagnosticCategory::Runtime,
                        code: "fixture_runtime_failure".to_string(),
                        message: "Fixture runtime failure".to_string(),
                        severity: crate::profile_dsl::diagnostics::DiagnosticSeverity::Error,
                        path: "/postingDiscovery/strategies/0".to_string(),
                        strategy_key: Some("json_api".to_string()),
                        details: Some(json!({ "fixture": true })),
                    }],
                }),
            ),
        ]);
        let scheduler = crate::background_tasks::BackgroundTaskScheduler::new(
            crate::background_tasks::BackgroundTaskSchedulerConfig::default(),
        );
        let pool_for_task = pool.clone();
        let app_data_dir = temp_dir.path().to_path_buf();
        let running_for_task = running_search_runs.clone();

        let task = scheduler
            .schedule(
                crate::background_tasks::BackgroundTaskSpec::search_run(),
                move |_context| async move {
                    let result = SearchRunService::new_with_result_artifact(
                        &pool_for_task,
                        running_for_task.as_ref(),
                        &executor,
                        SearchRunResultArtifact::Disabled,
                        app_data_dir,
                    )
                    .run(search_request.id)
                    .await;

                    match result {
                        Ok(result) => {
                            crate::background_tasks::BackgroundTaskCompletion::Succeeded {
                                result: serde_json::to_value(result).unwrap(),
                            }
                        }
                        Err(error) => crate::background_tasks::BackgroundTaskCompletion::Failed {
                            error,
                            diagnostics: Vec::new(),
                        },
                    }
                },
            )
            .unwrap();

        let finished = wait_for_background_task_state(
            &scheduler,
            &task.task_id,
            crate::background_tasks::BackgroundTaskState::Succeeded,
        )
        .await;
        let result = finished.result.expect("scheduled Search Run stores result");

        assert_eq!(result["status"], json!("completed_with_errors"));
        assert_eq!(result["sourceRuns"][0]["status"], json!("completed"));
        assert_eq!(result["sourceRuns"][1]["status"], json!("failed"));
        assert_eq!(
            result["sourceRuns"][1]["diagnostics"][0]["code"],
            json!("fixture_runtime_failure")
        );
        assert_eq!(result["postings"][0]["title"], json!("Laser Engineer"));
    });
}

#[test]
fn disabled_search_run_result_artifact_does_not_write_json() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let result_path = temp_dir.path().join("disabled-search-run-result.json");
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &[],
            )]),
        )]);
        let running_search_runs = RunningSearchRuns::default();

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &running_search_runs,
            &executor,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert!(!result_path.exists());
    });
}

#[test]
fn each_run_overwrites_search_run_result_json() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let result_path = temp_dir.path().join("search-run-result.json");
        std::fs::write(&result_path, "stale result").unwrap();
        let running_search_runs = RunningSearchRuns::default();

        let first_executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "First Engineer",
                "ACME",
                "https://example.test/first",
                &[],
            )]),
        )]);
        SearchRunService::new(
            &pool,
            &running_search_runs,
            &first_executor,
            result_path.clone(),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();
        let first_contents = std::fs::read_to_string(&result_path).unwrap();
        assert!(first_contents.contains("First Engineer"));
        assert!(!first_contents.contains("stale result"));

        let second_executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Second Engineer",
                "ACME",
                "https://example.test/second",
                &[],
            )]),
        )]);
        SearchRunService::new(
            &pool,
            &running_search_runs,
            &second_executor,
            result_path.clone(),
            temp_dir.path(),
        )
        .run(search_request.id)
        .await
        .unwrap();

        let second_contents = std::fs::read_to_string(&result_path).unwrap();
        assert!(second_contents.contains("Second Engineer"));
        assert!(!second_contents.contains("First Engineer"));
        let result_json: Value = serde_json::from_str(&second_contents).unwrap();
        assert_eq!(result_json["postings"][0]["title"], "Second Engineer");
    });
}
