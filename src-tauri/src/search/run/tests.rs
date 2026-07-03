use super::*;
use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRuleInput,
};
use serde_json::{json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Mutex,
};

struct FixtureSourceExecutor {
    responses: Mutex<HashMap<String, Result<Vec<SourceCandidate>, SourceExecutionError>>>,
}

impl FixtureSourceExecutor {
    fn new<K: ToString>(
        responses: impl IntoIterator<Item = (K, Result<Vec<SourceCandidate>, SourceExecutionError>)>,
    ) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(key, response)| (key.to_string(), response))
                    .collect(),
            ),
        }
    }
}

impl SourceExecutor for FixtureSourceExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            self.responses
                .lock()
                .unwrap()
                .get(&input.source.key)
                .cloned()
                .unwrap_or_else(|| {
                    Err(SourceExecutionError::Failed(format!(
                        "missing fixture response for source {}",
                        input.source.key
                    )))
                })
                .map(Into::into)
        })
    }
}

struct RuntimePostingDiscoveryExecutor {
    response_body: String,
}

impl RuntimePostingDiscoveryExecutor {
    fn new(response_body: impl Into<String>) -> Self {
        Self {
            response_body: response_body.into(),
        }
    }
}

impl SourceExecutor for RuntimePostingDiscoveryExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            let fetcher = FixturePostingDiscoveryFetcher {
                response_body: self.response_body.clone(),
            };
            let result = crate::profile_dsl::runtime::execute_posting_discovery_with_fetcher(
                &input.source.execution_plan,
                &fetcher,
            )
            .await;
            if result.candidates.is_empty()
                && result.diagnostics.iter().any(|diagnostic| {
                    diagnostic.severity
                        == crate::profile_dsl::diagnostics::DiagnosticSeverity::Error
                })
            {
                return Err(SourceExecutionError::FailedWithDiagnostics {
                    message: result
                        .diagnostics
                        .first()
                        .map(|diagnostic| diagnostic.message.clone())
                        .unwrap_or_else(|| "postingDiscovery failed".to_string()),
                    diagnostics: result.diagnostics,
                });
            }

            Ok(crate::search::run::SourceExecutionOutput {
                candidates: result
                    .candidates
                    .into_iter()
                    .map(|candidate| SourceCandidate {
                        title: candidate.title,
                        company: candidate.company,
                        url: candidate.url,
                        locations: candidate.locations,
                        posting_meta: candidate.posting_meta,
                    })
                    .collect(),
                diagnostics: result.diagnostics,
            })
        })
    }
}

struct FixturePostingDiscoveryFetcher {
    response_body: String,
}

impl crate::profile_dsl::runtime::PostingDiscoveryFetcher for FixturePostingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        _request: crate::profile_dsl::runtime::PostingDiscoveryFetchRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        crate::profile_dsl::runtime::PostingDiscoveryFetchResponse,
                        crate::profile_dsl::runtime::PostingDiscoveryFetchError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            Ok(crate::profile_dsl::runtime::PostingDiscoveryFetchResponse {
                body: self.response_body.clone(),
            })
        })
    }
}

struct RegistryMutatingPlanCaptureExecutor {
    profile_path: PathBuf,
    seen_discovery_markers: Mutex<Vec<(String, String)>>,
}

impl RegistryMutatingPlanCaptureExecutor {
    fn new(profile_path: PathBuf) -> Self {
        Self {
            profile_path,
            seen_discovery_markers: Mutex::new(Vec::new()),
        }
    }

    fn seen_discovery_markers(&self) -> Vec<(String, String)> {
        self.seen_discovery_markers.lock().unwrap().clone()
    }
}

impl SourceExecutor for RegistryMutatingPlanCaptureExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            let marker = input
                .source
                .execution_plan
                .posting_discovery
                .strategies
                .first()
                .and_then(|strategy| strategy.description.as_deref())
                .unwrap_or("missing")
                .to_string();
            self.seen_discovery_markers
                .lock()
                .unwrap()
                .push((input.source.key.clone(), marker));

            if input.source.key == "first_source" {
                std::fs::write(&self.profile_path, mutable_profile_json("changed"))
                    .map_err(|error| SourceExecutionError::Failed(error.to_string()))?;
            }

            Ok(vec![candidate(
                "Laser Engineer",
                input.source.name.as_str(),
                &format!("https://example.test/{}/laser", input.source.key),
                &[],
            )]
            .into())
        })
    }
}

#[test]
fn only_active_search_requests_can_run_and_non_active_requests_leave_last_run_empty() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let running_search_runs = RunningSearchRuns::default();
        let service = SearchRequestService::new(&pool, &running_search_runs);
        let temp_dir = tempfile::tempdir().unwrap();
        let executor = FixtureSourceExecutor::new([("test_source", Ok(vec![]))]);

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
        let executor = FixtureSourceExecutor::new([(
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

#[test]
fn active_valid_source_compiles_and_executes_posting_discovery_plan_through_runtime() {
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
        let executor = RuntimePostingDiscoveryExecutor::new(
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
        let executor = RuntimePostingDiscoveryExecutor::new("{");

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

#[test]
fn matching_uses_or_semantics_and_excludes_after_positive_matching() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("laser"), regex_rule("Optics\\s+Engineer")],
            vec![text_rule("praktikum"), regex_rule("Werkstudent")],
        )
        .await;
        let result_path = temp_dir.path().join("search-run-result.json");
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "LASER Physicist",
                    "SCHOTT",
                    "https://example.test/1",
                    &["Mainz"],
                ),
                candidate(
                    "Senior Optics Engineer",
                    "SCHOTT",
                    "https://example.test/2",
                    &["Mainz"],
                ),
                candidate(
                    "Laser Praktikum",
                    "SCHOTT",
                    "https://example.test/3",
                    &["Mainz"],
                ),
                candidate(
                    "Werkstudent Optics Engineer",
                    "SCHOTT",
                    "https://example.test/4",
                    &["Mainz"],
                ),
                candidate("Chemist", "SCHOTT", "https://example.test/5", &["Mainz"]),
            ]),
        )]);
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 5);
        assert_eq!(result.source_runs[0].matched_count, 2);
        assert_eq!(
            result
                .postings
                .iter()
                .map(|posting| posting.title.as_str())
                .collect::<Vec<_>>(),
            vec!["LASER Physicist", "Senior Optics Engineer"]
        );
        let result_json: Value = serde_json::from_str(
            &std::fs::read_to_string(result_path).expect("result JSON should be written"),
        )
        .unwrap();
        assert_eq!(result_json["status"], "completed");
        assert_eq!(result_json["sourceRuns"][0]["matchedCount"], 2);
    });
}

#[test]
fn exclude_regex_matching_is_case_insensitive_without_changing_include_regex() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![regex_rule("Laser")],
            vec![regex_rule("praktik(um|ant)")],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "Laser Engineer",
                    "SCHOTT",
                    "https://example.test/1",
                    &["Mainz"],
                ),
                candidate(
                    "Laser PraktikantIn",
                    "SCHOTT",
                    "https://example.test/2",
                    &["Mainz"],
                ),
                candidate(
                    "laser Engineer",
                    "SCHOTT",
                    "https://example.test/3",
                    &["Mainz"],
                ),
            ]),
        )]);
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 3);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(
            result
                .postings
                .iter()
                .map(|posting| posting.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Laser Engineer"]
        );
    });
}

#[test]
fn normalizes_source_candidates_before_matching_and_merging() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let source_keys = write_test_sources(temp_dir.path(), &[("test_source", "Test Source")]);
        let search_request = create_test_search_request(
            &pool,
            source_keys.clone(),
            vec![text_rule("Senior Laser Engineer")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![
                candidate(
                    "  Senior\n Laser   Engineer  ",
                    "\tACME   GmbH\n",
                    " https://example.test/laser ",
                    &[" Mainz ", "", "mainz", "  München\nNord  "],
                ),
                candidate(
                    "Senior Laser Engineer",
                    " ",
                    "https://example.test/empty-company",
                    &["Remote"],
                ),
            ]),
        )]);
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Senior Laser Engineer");
        assert_eq!(posting.company, "ACME GmbH");
        assert_eq!(posting.url, "https://example.test/laser");
        assert_eq!(posting.locations, vec!["Mainz", "München Nord"]);
        assert_eq!(posting.sources[0].url, "https://example.test/laser");
    });
}

#[test]
fn dedupes_with_overlapping_locations_or_missing_locations_and_preserves_sources() {
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
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![
                    candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://source-one.test/laser",
                        &["Mainz"],
                    ),
                    candidate(
                        "Remote Engineer",
                        "ACME",
                        "https://source-one.test/remote",
                        &[],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-one.test/optics-berlin",
                        &["Berlin"],
                    ),
                ]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![
                    candidate(
                        "Laser Engineer",
                        "ACME",
                        "https://source-two.test/laser",
                        &[" mainz ", "Wiesbaden"],
                    ),
                    candidate(
                        "Remote Engineer",
                        "ACME",
                        "https://source-two.test/remote",
                        &["Berlin"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-two.test/optics-hamburg",
                        &["Hamburg"],
                    ),
                ]),
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 4);

        let laser = result
            .postings
            .iter()
            .find(|posting| posting.title == "Laser Engineer")
            .unwrap();
        assert_eq!(laser.locations, vec!["Mainz", "Wiesbaden"]);
        assert_eq!(laser.sources.len(), 2);
        assert_eq!(
            laser
                .sources
                .iter()
                .map(|source| source.source_key.as_str())
                .collect::<Vec<_>>(),
            vec!["source_one", "source_two"]
        );

        let remote = result
            .postings
            .iter()
            .find(|posting| posting.title == "Remote Engineer")
            .unwrap();
        assert_eq!(remote.locations, vec!["Berlin"]);
        assert_eq!(remote.sources.len(), 2);

        let optics_postings = result
            .postings
            .iter()
            .filter(|posting| posting.title == "Optics Engineer")
            .collect::<Vec<_>>();
        assert_eq!(optics_postings.len(), 2);
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Berlin"]));
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Hamburg"]));
    });
}

#[test]
fn merging_and_import_preserve_per_source_posting_meta() {
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
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![candidate_with_meta(
                    "Laser Engineer",
                    "ACME",
                    "https://source-one.test/laser",
                    &["Mainz"],
                    [("jobId", "source-one-42")],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate_with_meta(
                    "Laser Engineer",
                    "ACME",
                    "https://source-two.test/laser",
                    &["Mainz"],
                    [("jobId", "source-two-99")],
                )]),
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].sources.len(), 2);
        assert_eq!(
            result.postings[0].sources[0].posting_meta,
            BTreeMap::from([("jobId".to_string(), "source-one-42".to_string())])
        );
        assert_eq!(
            result.postings[0].sources[1].posting_meta,
            BTreeMap::from([("jobId".to_string(), "source-two-99".to_string())])
        );

        let rows = sqlx::query(
            "SELECT source_key, posting_meta_json
             FROM job_posting_sources
             ORDER BY source_key",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<String, _>("source_key"), "source_one");
        assert_eq!(
            rows[0].get::<String, _>("posting_meta_json"),
            r#"{"jobId":"source-one-42"}"#
        );
        assert_eq!(rows[1].get::<String, _>("source_key"), "source_two");
        assert_eq!(
            rows[1].get::<String, _>("posting_meta_json"),
            r#"{"jobId":"source-two-99"}"#
        );
    });
}

#[test]
fn fuzzy_dedupes_equivalent_titles_and_preserves_representative_posting() {
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
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Head Of Laser & Post Processing Development (mwd) RP/1205240901",
                    "SCHOTT AG",
                    "https://source-one.test/schott-laser",
                    &["Mainz"],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate(
                    "Head of Laser & Post Processing Development (m/w/d)*",
                    "SCHOTT AG",
                    "https://source-two.test/schott-laser",
                    &["Mainz"],
                )]),
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(
            posting.title,
            "Head Of Laser & Post Processing Development (mwd) RP/1205240901"
        );
        assert_eq!(posting.company, "SCHOTT AG");
        assert_eq!(posting.url, "https://source-one.test/schott-laser");
        assert_eq!(posting.locations, vec!["Mainz"]);
        assert_eq!(posting.sources.len(), 2);
        assert_eq!(
            posting
                .sources
                .iter()
                .map(|source| source.source_key.as_str())
                .collect::<Vec<_>>(),
            vec!["source_one", "source_two"]
        );
    });
}

#[test]
fn fuzzy_dedupe_keeps_different_roles_at_same_company_and_location_separate() {
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
            vec![text_rule("laser")],
            vec![],
        )
        .await;
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![candidate(
                    "Laser Engineer",
                    "ACME",
                    "https://source-one.test/laser-engineer",
                    &["Mainz"],
                )]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![candidate(
                    "Senior Laser Engineer Frontend",
                    "ACME",
                    "https://source-two.test/senior-laser-engineer-frontend",
                    &["Mainz"],
                )]),
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 2);
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Laser Engineer"));
        assert!(result
            .postings
            .iter()
            .any(|posting| posting.title == "Senior Laser Engineer Frontend"));
    });
}

#[test]
fn location_compatibility_allows_whole_phrase_overlap_but_blocks_contradictions() {
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
        let executor = FixtureSourceExecutor::new([
            (
                source_keys[0].clone(),
                Ok(vec![
                    candidate(
                        "Platform Engineer",
                        "ACME",
                        "https://source-one.test/platform-berlin",
                        &["Berlin"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-one.test/optics-mainz",
                        &["Mainz"],
                    ),
                ]),
            ),
            (
                source_keys[1].clone(),
                Ok(vec![
                    candidate(
                        "Platform Engineer",
                        "ACME",
                        "https://source-two.test/platform-berlin-germany",
                        &["Berlin, Germany"],
                    ),
                    candidate(
                        "Optics Engineer",
                        "ACME",
                        "https://source-two.test/optics-frankfurt",
                        &["Frankfurt am Main"],
                    ),
                ]),
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

        assert_eq!(result.status, SearchRunStatus::Completed);
        assert_eq!(result.postings.len(), 3);

        let platform = result
            .postings
            .iter()
            .find(|posting| posting.title == "Platform Engineer")
            .unwrap();
        assert_eq!(platform.locations, vec!["Berlin", "Berlin, Germany"]);
        assert_eq!(platform.sources.len(), 2);

        let optics_postings = result
            .postings
            .iter()
            .filter(|posting| posting.title == "Optics Engineer")
            .collect::<Vec<_>>();
        assert_eq!(optics_postings.len(), 2);
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Mainz"]));
        assert!(optics_postings
            .iter()
            .any(|posting| posting.locations == vec!["Frankfurt am Main"]));
    });
}

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
        let executor = FixtureSourceExecutor::new([
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
                    "fixture source failed".to_string(),
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
            Some("fixture source failed")
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
        assert!(last_run_error.contains("fixture source failed"));

        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(result_path).unwrap()).unwrap();
        assert_eq!(result_json["status"], "completed_with_errors");
        assert_eq!(
            result_json["sourceRuns"][1]["error"],
            "fixture source failed"
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
        let executor = FixtureSourceExecutor::new([
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
        assert!(last_run_error.contains("first failed"));
        assert!(last_run_error.contains("source_two"));
        assert!(last_run_error.contains("second failed"));
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
        let executor = FixtureSourceExecutor::new([(
            source_keys[0].clone(),
            Ok(vec![candidate(
                "Laser Engineer",
                "ACME",
                "https://example.test/laser",
                &["Mainz"],
            )]),
        )]);

        let error = SearchRunService::new(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
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
    });
}

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

async fn create_test_search_request(
    pool: &SqlitePool,
    source_keys: Vec<String>,
    include_rules: Vec<SearchRuleInput>,
    exclude_rules: Vec<SearchRuleInput>,
) -> SearchRequest {
    let running_search_runs = RunningSearchRuns::default();
    SearchRequestService::new(pool, &running_search_runs)
        .create(CreateSearchRequestInput {
            status: SearchRequestStatus::Active,
            include_rules,
            exclude_rules,
            locations: vec!["Mainz".to_string()],
            radius_km: Some(30),
            source_keys,
        })
        .await
        .unwrap()
}

fn write_test_sources(app_data_dir: &Path, sources: &[(&str, &str)]) -> Vec<String> {
    sources
        .iter()
        .map(|(key, name)| {
            write_json(
                app_data_dir.join(format!("sources/{key}.json")),
                &source_json(key, name),
            );
            (*key).to_string()
        })
        .collect()
}

fn source_json(key: &str, name: &str) -> String {
    source_json_with_status(key, name, "active")
}

fn source_json_with_status(key: &str, name: &str, status: &str) -> String {
    json!({
        "schemaVersion": 2,
        "key": key,
        "name": name,
        "status": status,
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "fixture_discovery",
            "name": "Fixture Discovery",
            "postingDiscovery": minimal_posting_discovery("fixture")
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Deterministic Search Run fixture Source."
        }
    })
    .to_string()
}

fn text_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "text".to_string(),
        value: value.to_string(),
    }
}

fn regex_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "regex".to_string(),
        value: value.to_string(),
    }
}

fn mutable_profile_json(marker: &str) -> String {
    json!({
        "schemaVersion": 2,
        "key": "mutable_profile",
        "name": "Mutable Profile",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Mutable Search Run fixture profile."
        },
        "accessPaths": [
            {
                "key": "posting_discovery",
                "name": "Posting Discovery",
                "description": marker,
                "sourceConfigSchema": { "type": "object" },
                "postingDiscovery": minimal_posting_discovery(marker)
            }
        ]
    })
    .to_string()
}

fn mutable_profile_source_json(key: &str, name: &str) -> String {
    json!({
        "schemaVersion": 2,
        "key": key,
        "name": name,
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "mutable_profile",
            "pathKey": "posting_discovery"
        }
    })
    .to_string()
}

fn minimal_posting_discovery(marker: &str) -> Value {
    json!({
        "strategies": [
            {
                "key": "json_api",
                "description": marker,
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/jobs.json",
                    "timeoutMs": 1000
                },
                "parse": { "type": "json" },
                "select": {
                    "type": "json_path",
                    "jsonPath": "$.jobs"
                },
                "extract": {
                    "fields": {
                        "title": {
                            "type": "json_path",
                            "jsonPath": "$.title",
                            "cardinality": "one"
                        },
                        "company": {
                            "type": "json_path",
                            "jsonPath": "$.company",
                            "cardinality": "one"
                        },
                        "url": {
                            "type": "json_path",
                            "jsonPath": "$.url",
                            "cardinality": "one"
                        },
                        "locations": {
                            "type": "json_path",
                            "jsonPath": "$.locations",
                            "cardinality": "all"
                        },
                        "postingMeta": {
                            "jobId": {
                                "type": "json_path",
                                "jsonPath": "$.jobId",
                                "cardinality": "one"
                            }
                        }
                    }
                }
            }
        ]
    })
}

fn write_json(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

async fn wait_for_background_task_state(
    scheduler: &crate::background_tasks::BackgroundTaskScheduler,
    task_id: &str,
    state: crate::background_tasks::BackgroundTaskState,
) -> crate::background_tasks::BackgroundTaskSnapshot {
    for _ in 0..100 {
        let snapshot = scheduler.get(task_id).unwrap();
        if snapshot.state == state {
            return snapshot;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("task {task_id} did not reach state {state:?}");
}

fn candidate(title: &str, company: &str, url: &str, locations: &[&str]) -> SourceCandidate {
    candidate_with_meta(title, company, url, locations, [])
}

fn candidate_with_meta(
    title: &str,
    company: &str,
    url: &str,
    locations: &[&str],
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> SourceCandidate {
    SourceCandidate {
        title: title.to_string(),
        company: company.to_string(),
        url: url.to_string(),
        locations: locations
            .iter()
            .map(|location| (*location).to_string())
            .collect(),
        posting_meta: posting_meta
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}

async fn migrated_pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    pool
}
