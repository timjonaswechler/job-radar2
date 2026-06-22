use super::*;
use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRuleInput,
};
use serde_json::{json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    collections::HashMap,
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
        })
    }
}

struct RegistryMutatingPlanCaptureExecutor {
    profile_path: PathBuf,
    seen_inventory_markers: Mutex<Vec<(String, String)>>,
}

impl RegistryMutatingPlanCaptureExecutor {
    fn new(profile_path: PathBuf) -> Self {
        Self {
            profile_path,
            seen_inventory_markers: Mutex::new(Vec::new()),
        }
    }

    fn seen_inventory_markers(&self) -> Vec<(String, String)> {
        self.seen_inventory_markers.lock().unwrap().clone()
    }
}

impl SourceExecutor for RegistryMutatingPlanCaptureExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            let marker = input
                .source
                .inventory()
                .and_then(|inventory| inventory.get("marker"))
                .and_then(Value::as_str)
                .unwrap_or("missing")
                .to_string();
            self.seen_inventory_markers
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
            )])
        })
    }
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
            .contains("sourceKey `missing_source` was not found"));
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
            executor.seen_inventory_markers(),
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
    json!({
        "schemaVersion": 1,
        "key": key,
        "name": name,
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_specific",
            "adapterKey": "fixture_inventory"
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
        "schemaVersion": 1,
        "key": "mutable_profile",
        "name": "Mutable Profile",
        "kind": "generic",
        "accessPaths": [
            {
                "key": "inventory",
                "adapterKey": "test_inventory",
                "sourceConfigSchema": { "type": "object" },
                "inventory": { "marker": marker }
            }
        ]
    })
    .to_string()
}

fn mutable_profile_source_json(key: &str, name: &str) -> String {
    json!({
        "schemaVersion": 1,
        "key": key,
        "name": name,
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile",
            "profileKey": "mutable_profile",
            "pathKey": "inventory"
        }
    })
    .to_string()
}

fn write_json(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn candidate(title: &str, company: &str, url: &str, locations: &[&str]) -> SourceCandidate {
    SourceCandidate {
        title: title.to_string(),
        company: company.to_string(),
        url: url.to_string(),
        locations: locations
            .iter()
            .map(|location| (*location).to_string())
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
