use super::*;
use crate::{
    search::request::{RunningSearchRuns, SearchRequestService},
    search::run::{
        BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError, SourceExecutionInput,
        SourceExecutor, SourceRunStatus,
    },
};
use serde_json::Value;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{collections::HashMap, sync::Mutex};

struct FixtureSourceExecutor {
    responses: Mutex<HashMap<String, Result<Vec<SourceCandidate>, SourceExecutionError>>>,
}

impl FixtureSourceExecutor {
    fn new(
        responses: impl IntoIterator<
            Item = (
                &'static str,
                Result<Vec<SourceCandidate>, SourceExecutionError>,
            ),
        >,
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
                        "missing fixture response for {}",
                        input.source.key
                    )))
                })
        })
    }
}

#[test]
fn smoke_path_creates_exact_request_filters_results_and_records_stepstone_failure() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_schott_smoke_source_file(temp_dir.path()).unwrap();
        let running_search_runs = RunningSearchRuns::default();
        let executor = FixtureSourceExecutor::new([
            (
                SCHOTT_SOURCE_KEY,
                Ok(vec![
                    candidate(
                        "Laser Entwicklungsingenieur",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-Laser-Entwicklungsingenieur-55122/",
                        &["Mainz"],
                    ),
                    candidate(
                        "Physik Praktikum",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-Physik-Praktikum-55122/",
                        &["Mainz"],
                    ),
                    candidate(
                        "ChemielaborantIn Analytik",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-ChemielaborantIn-Analytik-55122/",
                        &["Mainz"],
                    ),
                ]),
            ),
            (
                STEPSTONE_SOURCE_KEY,
                Err(SourceExecutionError::Failed(
                    "stepstone fixture unavailable".to_string(),
                )),
            ),
        ]);
        let result_path = temp_dir.path().join("search-run-result.json");
        std::fs::write(&result_path, "stale smoke result").unwrap();

        let summary = run_schott_stepstone_smoke(
            &pool,
            &running_search_runs,
            &executor,
            result_path.clone(),
            temp_dir.path(),
        )
        .await
        .unwrap();

        assert!(summary.search_request_created);
        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .get(summary.search_request_id)
            .await
            .unwrap();
        assert_eq!(
            search_request.include_rules,
            expected_rules(INCLUDE_RULE_VALUES)
        );
        assert_eq!(
            search_request.exclude_rules,
            expected_rules(EXCLUDE_RULE_VALUES)
        );
        assert_eq!(search_request.locations, vec![SMOKE_LOCATION]);
        assert_eq!(search_request.radius_km, Some(SMOKE_RADIUS_KM));
        assert_eq!(search_request.source_keys, smoke_source_keys());

        assert_eq!(
            serialized_label(&summary.result.status),
            "completed_with_errors"
        );
        assert_eq!(summary.result.source_runs[0].source_key, SCHOTT_SOURCE_KEY);
        assert_eq!(
            summary.result.source_runs[0].status,
            SourceRunStatus::Completed
        );
        assert_eq!(summary.result.source_runs[0].candidate_count, 3);
        assert_eq!(summary.result.source_runs[0].matched_count, 1);
        assert_eq!(
            summary.result.source_runs[1].source_key,
            STEPSTONE_SOURCE_KEY
        );
        assert_eq!(
            summary.result.source_runs[1].status,
            SourceRunStatus::Failed
        );
        assert_eq!(
            summary.result.source_runs[1].error.as_deref(),
            Some("stepstone fixture unavailable")
        );
        assert_eq!(summary.result.postings.len(), 1);
        assert_eq!(
            summary.result.postings[0].title,
            "Laser Entwicklungsingenieur"
        );
        assert_eq!(summary.result.postings[0].company, "SCHOTT");
        assert_eq!(summary.result.postings[0].locations, vec!["Mainz"]);
        assert_eq!(
            summary.result.postings[0].sources[0].source_key,
            SCHOTT_SOURCE_KEY
        );

        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&result_path).unwrap()).unwrap();
        assert_ne!(
            std::fs::read_to_string(&result_path).unwrap(),
            "stale smoke result"
        );
        assert_eq!(result_json["status"], "completed_with_errors");
        assert_eq!(
            result_json["postings"][0]["title"],
            "Laser Entwicklungsingenieur"
        );
        assert!(result_json["postings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|posting| !posting["title"].as_str().unwrap().contains("Praktikum")));
    });
}

#[test]
fn ensure_schott_source_creates_only_missing_local_smoke_source_json() {
    let temp_dir = tempfile::tempdir().unwrap();

    let created = ensure_schott_smoke_source(temp_dir.path()).unwrap();
    let reused = ensure_schott_smoke_source(temp_dir.path()).unwrap();
    let snapshot = crate::source::registry::load_snapshot(temp_dir.path());

    assert_eq!(created.document.key, SCHOTT_SOURCE_KEY);
    assert_eq!(reused.document.key, SCHOTT_SOURCE_KEY);
    assert_eq!(created.document, reused.document);
    validate_smoke_source(&created).unwrap();
    assert!(snapshot.source(STEPSTONE_SOURCE_KEY).is_some());
    assert!(temp_dir
        .path()
        .join(format!("sources/{SCHOTT_SOURCE_KEY}.json"))
        .is_file());
    assert_eq!(
        snapshot
            .valid_sources
            .iter()
            .filter(|source| source.document.key == SCHOTT_SOURCE_KEY)
            .count(),
        1
    );
}

#[test]
fn smoke_path_reuses_existing_smoke_request_on_later_runs() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_schott_smoke_source_file(temp_dir.path()).unwrap();
        let running_search_runs = RunningSearchRuns::default();
        let executor = FixtureSourceExecutor::new([
            (
                SCHOTT_SOURCE_KEY,
                Ok(vec![candidate(
                    "Laser Ingenieur",
                    "SCHOTT",
                    "https://join.schott.com/job/Mainz-Laser-Ingenieur-55122/",
                    &["Mainz"],
                )]),
            ),
            (STEPSTONE_SOURCE_KEY, Ok(vec![])),
        ]);

        let first = run_schott_stepstone_smoke(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .await
        .unwrap();
        let second = run_schott_stepstone_smoke(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .await
        .unwrap();

        assert!(first.search_request_created);
        assert!(!second.search_request_created);
        assert_eq!(first.search_request_id, second.search_request_id);
        assert_eq!(
            SearchRequestService::new(&pool, &running_search_runs)
                .list()
                .await
                .unwrap()
                .len(),
            1
        );
    });
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
