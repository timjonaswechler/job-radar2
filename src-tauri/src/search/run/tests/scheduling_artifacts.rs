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
        let executor = fixture_resolution_runtime([
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
                        path: "/discovery/strategies/0".to_string(),
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
fn two_source_success_and_source_detail_abort_persist_only_successful_source_state() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[
                ("successful_source", "Successful Source"),
                ("aborted_source", "Aborted Source"),
            ],
        );
        write_json(
            temp_dir.path().join("sources/aborted_source.json"),
            &source_json_with_detail("aborted_source", "Aborted Source"),
        );
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let limits = crate::search::run::production_resolution_ceilings();
        let successful_occurrence = occurrence(
            &source_keys[0],
            candidate(
                "Laser Engineer",
                "ACME",
                "https://successful.test/laser",
                &["Mainz"],
            ),
        );
        let aborted_occurrence = occurrence(
            &source_keys[1],
            candidate(
                "Blocked Engineer",
                "",
                "https://aborted.test/blocked",
                &["Mainz"],
            ),
        );
        let detail_snapshot = crate::profile_dsl::runtime::SourceDetailRequestSnapshot::new(
            &source_keys[1],
            aborted_occurrence.identity.clone(),
            crate::profile_dsl::runtime::RequestedDetailFields::new([
                crate::profile_dsl::runtime::DetailField::Company,
            ])
            .unwrap(),
        );
        let source = |key: &str,
                      occurrences: Vec<crate::profile_dsl::occurrence::PostingOccurrence>,
                      detail| {
            crate::search::run::ScriptedResolutionSource {
                discovery:
                    crate::search::candidate_resolution::ScriptedSourceDiscoveryExecution::new(
                        key,
                        [
                            crate::search::candidate_resolution::ScriptedDiscoveryBatch {
                                expected_continuation: None,
                                expected_maximum: limits.max_batch_size,
                                expected_limits: limits.phase,
                                occurrences,
                                exhausted: true,
                                remaining: Some(0),
                                continuation: None,
                                continuation_source_key: None,
                                complete_budget_report:
                                    crate::profile_dsl::runtime::PhaseExecutionReport {
                                        usage: Default::default(),
                                        completion:
                                            crate::profile_dsl::runtime::PhaseCompletion::Accepted,
                                    },
                                diagnostics: Vec::new(),
                            },
                        ],
                    ),
                detail,
            }
        };
        let runtime = SearchRunResolutionRuntime::scripted([
            (
                source_keys[0].clone(),
                source(
                    &source_keys[0],
                    vec![successful_occurrence],
                    crate::profile_dsl::runtime::ScriptedSourceDetailExecution::new([]),
                ),
            ),
            (
                source_keys[1].clone(),
                source(
                    &source_keys[1],
                    vec![aborted_occurrence],
                    crate::profile_dsl::runtime::ScriptedSourceDetailExecution::new([(
                        detail_snapshot,
                        Ok(crate::profile_dsl::runtime::SourceDetailOutcome::SourceExecutionFailed {
                            typed_failure: crate::profile_dsl::runtime::SourceDetailFailure::PhaseExecution {
                                failure: crate::profile_dsl::runtime::PhaseExecutionFailure::Internal,
                            },
                            complete_budget_report: Some(crate::profile_dsl::runtime::PhaseExecutionReport {
                                usage: Default::default(),
                                completion: crate::profile_dsl::runtime::PhaseCompletion::ExecutionFailed,
                            }),
                            diagnostics: Vec::new(),
                        }),
                    )]),
                ),
            ),
        ]);

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &RunningSearchRuns::default(),
            &runtime,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .run(request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::CompletedWithErrors);
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[1].status, SourceRunStatus::Failed);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer");
        assert_eq!(result.postings[0].sources.len(), 1);
        assert_eq!(result.postings[0].sources[0].source_key, source_keys[0]);

        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT jp.title, jps.source_key, m.search_run_id FROM job_postings jp JOIN job_posting_sources jps ON jps.posting_id = jp.id JOIN matches m ON m.job_posting_id = jp.id ORDER BY jp.id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "Laser Engineer");
        assert_eq!(rows[0].1, source_keys[0]);
        let search_run_id: i64 = sqlx::query_scalar("SELECT id FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows[0].2, search_run_id);
        for table in ["job_postings", "job_posting_sources", "matches"] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                1,
                "unexpected {table} count"
            );
        }
    });
}

#[test]
fn partial_resolution_persists_only_its_committed_finalized_output() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys =
            write_test_sources(temp_dir.path(), &[("partial_source", "Partial Source")]);
        write_json(
            temp_dir.path().join("sources/partial_source.json"),
            &source_json_with_detail("partial_source", "Partial Source"),
        );
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let limits = crate::search::run::production_resolution_ceilings();
        let mut usage = crate::profile_dsl::runtime::PhaseUsage::default();
        usage.pages = limits.phase.max_pages;
        let runtime = SearchRunResolutionRuntime::scripted([(
            source_keys[0].clone(),
            crate::search::run::ScriptedResolutionSource {
                discovery:
                    crate::search::candidate_resolution::ScriptedSourceDiscoveryExecution::new(
                        &source_keys[0],
                        [
                            crate::search::candidate_resolution::ScriptedDiscoveryBatch {
                                expected_continuation: None,
                                expected_maximum: limits.max_batch_size,
                                expected_limits: limits.phase,
                                occurrences: vec![
                                    occurrence(
                                        &source_keys[0],
                                        candidate(
                                            "Laser Engineer",
                                            "ACME",
                                            "https://example.test/finalized-before-budget",
                                            &["Mainz"],
                                        ),
                                    ),
                                    occurrence(
                                        &source_keys[0],
                                        candidate(
                                            "Current Engineer",
                                            "",
                                            "https://example.test/unresolved-at-budget",
                                            &["Mainz"],
                                        ),
                                    ),
                                    occurrence(
                                        &source_keys[0],
                                        candidate(
                                            "Later Engineer",
                                            "ACME",
                                            "https://example.test/skipped-after-budget",
                                            &["Mainz"],
                                        ),
                                    ),
                                ],
                                exhausted: false,
                                remaining: Some(1),
                                continuation: Some("next".to_string()),
                                continuation_source_key: None,
                                complete_budget_report:
                                    crate::profile_dsl::runtime::PhaseExecutionReport {
                                        usage,
                                        completion:
                                            crate::profile_dsl::runtime::PhaseCompletion::Accepted,
                                    },
                                diagnostics: Vec::new(),
                            },
                        ],
                    ),
                detail: crate::profile_dsl::runtime::ScriptedSourceDetailExecution::new([]),
            },
        )]);

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &RunningSearchRuns::default(),
            &runtime,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .run(request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert!(matches!(
            result.source_runs[0]
                .resolution
                .as_ref()
                .unwrap()
                .completion,
            crate::search::candidate_resolution::ResolutionCompletion::Partial {
                limit_reached: crate::search::candidate_resolution::ResolutionLimitDimension::Pages
            }
        ));
        let summary = result.source_runs[0].resolution.as_ref().unwrap();
        assert_eq!(summary.counts.finalized, 1);
        assert_eq!(summary.counts.unresolved, 1);
        assert_eq!(summary.counts.budget_skipped, 1);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].title, "Laser Engineer");
        assert_eq!(result.postings[0].sources.len(), 1);
        assert_eq!(
            result.postings[0].sources[0].url,
            "https://example.test/finalized-before-budget"
        );

        let persisted: Vec<(String, String)> = sqlx::query_as(
            "SELECT jp.title, jps.url FROM job_postings jp JOIN job_posting_sources jps ON jps.posting_id = jp.id ORDER BY jp.id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            persisted,
            vec![(
                "Laser Engineer".to_string(),
                "https://example.test/finalized-before-budget".to_string()
            )]
        );
        for table in ["job_postings", "job_posting_sources", "matches"] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                1,
                "unexpected {table} count"
            );
        }
    });
}

#[test]
fn token_driven_background_cancellation_and_sql_terminal_state_agree() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("token_source", "Token Source")]);
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let runtime = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/must-not-persist",
                &["Mainz"],
            )]),
        )]);
        let scheduler = crate::background_tasks::BackgroundTaskScheduler::new(
            crate::background_tasks::BackgroundTaskSchedulerConfig::default(),
        );
        let pool_for_task = pool.clone();
        let app_data_dir = temp_dir.path().to_path_buf();
        let task = scheduler
            .schedule(
                crate::background_tasks::BackgroundTaskSpec::search_run(),
                move |context| async move {
                    let token_for_boundary = context.cancellation_token.clone();
                    let cancel_after_resolution = move || token_for_boundary.cancel();
                    let result = SearchRunService::new_with_result_artifact(
                        &pool_for_task,
                        &RunningSearchRuns::default(),
                        &runtime,
                        SearchRunResultArtifact::Disabled,
                        app_data_dir,
                    )
                    .after_source_resolution(&cancel_after_resolution)
                    .run_with_cancellation(request.id, Some(&context.cancellation_token))
                    .await
                    .unwrap();
                    if result.status == SearchRunStatus::Cancelled
                        || context.cancellation_token.is_cancelled()
                    {
                        crate::background_tasks::BackgroundTaskCompletion::Cancelled {
                            error: Some("Search Run cancelled".to_string()),
                            result: serde_json::to_value(result).ok(),
                            diagnostics: Vec::new(),
                        }
                    } else {
                        crate::background_tasks::BackgroundTaskCompletion::Succeeded {
                            result: serde_json::to_value(result).unwrap(),
                        }
                    }
                },
            )
            .unwrap();

        let finished = wait_for_background_task_state(
            &scheduler,
            &task.task_id,
            crate::background_tasks::BackgroundTaskState::Cancelled,
        )
        .await;
        assert_eq!(
            finished.result.as_ref().unwrap()["status"],
            json!("cancelled")
        );
        assert_eq!(
            finished.result.as_ref().unwrap()["sourceRuns"][0]["status"],
            json!("completed"),
            "the real token is cancelled only after Source resolution completes"
        );
        for table in ["job_postings", "job_posting_sources", "matches"] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                0,
                "unexpected {table} rows after token cancellation"
            );
        }
        let run_status: String = sqlx::query_scalar("SELECT status FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(run_status, "cancelled");
    });
}

#[test]
fn cancellation_after_earlier_source_resolution_persists_metadata_without_candidate_rows() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(
            temp_dir.path(),
            &[
                ("completed_source", "Completed Source"),
                ("cancelled_source", "Cancelled Source"),
            ],
        );
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("engineer")],
            vec![],
        )
        .await;
        let limits = crate::search::run::production_resolution_ceilings();
        let completed = crate::search::candidate_resolution::ScriptedDiscoveryOutcome::Batch(
            crate::search::candidate_resolution::ScriptedDiscoveryBatch {
                expected_continuation: None,
                expected_maximum: limits.max_batch_size,
                expected_limits: limits.phase,
                occurrences: vec![occurrence(
                    &source_keys[0],
                    candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://example.test/committed",
                        &["Mainz"],
                    ),
                )],
                exhausted: true,
                remaining: Some(0),
                continuation: None,
                continuation_source_key: None,
                complete_budget_report: crate::profile_dsl::runtime::PhaseExecutionReport {
                    usage: Default::default(),
                    completion: crate::profile_dsl::runtime::PhaseCompletion::Accepted,
                },
                diagnostics: Vec::new(),
            },
        );
        let cancelled = crate::search::candidate_resolution::ScriptedDiscoveryOutcome::Cancelled {
            expected_continuation: None,
            expected_maximum: limits.max_batch_size,
            expected_limits: limits.phase,
            complete_budget_report: crate::profile_dsl::runtime::PhaseExecutionReport {
                usage: Default::default(),
                completion: crate::profile_dsl::runtime::PhaseCompletion::Cancelled {
                    reason: crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                },
            },
            diagnostics: Vec::new(),
        };
        let runtime = SearchRunResolutionRuntime::scripted(
            [(source_keys[0].clone(), completed), (source_keys[1].clone(), cancelled)]
                .into_iter()
                .map(|(key, outcome)| {
                    let source = crate::search::run::ScriptedResolutionSource {
                        discovery: crate::search::candidate_resolution::ScriptedSourceDiscoveryExecution::new_outcomes(&key, [outcome]),
                        detail: crate::profile_dsl::runtime::ScriptedSourceDetailExecution::new([]),
                    };
                    (key, source)
                }),
        );

        let result = SearchRunService::new_with_result_artifact(
            &pool,
            &RunningSearchRuns::default(),
            &runtime,
            SearchRunResultArtifact::Disabled,
            temp_dir.path(),
        )
        .run(request.id)
        .await
        .unwrap();

        assert_eq!(result.status, SearchRunStatus::Cancelled);
        assert!(result.postings.is_empty());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_runs")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matches")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM job_postings")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        let metadata: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT last_run_status, last_run_error FROM search_requests WHERE id = ?1",
        )
        .bind(request.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(metadata.0.as_deref(), Some("cancelled"));
        assert!(metadata
            .1
            .as_deref()
            .is_some_and(|value| value.contains("cancelled")));
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
        let executor = fixture_resolution_runtime([(
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

        let first_executor = fixture_resolution_runtime([(
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
        assert!(!first_contents.contains("First Engineer"));
        assert!(!first_contents.contains("stale result"));
        let first_json: Value = serde_json::from_str(&first_contents).unwrap();
        assert_eq!(first_json["postingCount"], 1);

        let second_executor = fixture_resolution_runtime([(
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
        assert!(!second_contents.contains("First Engineer"));
        assert!(!second_contents.contains("Second Engineer"));
        let result_json: Value = serde_json::from_str(&second_contents).unwrap();
        assert_eq!(result_json["postingCount"], 1);
        assert!(result_json.get("postings").is_none());
    });
}

#[test]
fn post_commit_artifact_failure_keeps_atomic_search_run_rows() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let resolver = fixture_resolution_runtime([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &["Mainz"],
            )]),
        )]);
        let artifact_directory = temp_dir.path().join("artifact-is-a-directory");
        std::fs::create_dir(&artifact_directory).unwrap();
        let result = SearchRunService::new(
            &pool,
            &RunningSearchRuns::default(),
            &resolver,
            &artifact_directory,
            temp_dir.path(),
        )
        .run(request.id)
        .await
        .unwrap();
        assert_eq!(result.status, SearchRunStatus::Completed);
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "search_run_result_artifact_write_failed" }));
        for table in ["search_runs", "matches", "job_postings"] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 1, "unexpected {table} count");
        }
    });
}
