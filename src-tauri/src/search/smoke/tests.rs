use super::*;
use crate::{
    search::request::{
        RunningSearchRuns, SearchRequestService, SearchRule, SearchRuleKind, SearchRuleTarget,
    },
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
                .map(Into::into)
        })
    }
}

#[test]
#[ignore = "network-dependent development smoke path"]
fn smoke_path_creates_exact_request_filters_results() {
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
                        "PraktikantIn Lasermaterialbearbeitung (mwd)",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-PraktikantIn-Lasermaterialbearbeitung-55122/",
                        &["Mainz"],
                    ),
                    candidate(
                        "Ausbildung PhysiklaborantIn (mwd)",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-Ausbildung-PhysiklaborantIn-55122/",
                        &["Mainz"],
                    ),
                    candidate(
                        "StudentIn Physik Technik Für Masterthesis Laser Materialbearbeitung",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-StudentIn-Physik-Technik-Masterthesis-Laser-Materialbearbeitung-55122/",
                        &["Mainz"],
                    ),
                    candidate(
                        "Masterthesis Laser-/ Materialbearbeitung (m/w/d)*",
                        "SCHOTT",
                        "https://join.schott.com/job/Mainz-Masterthesis-Laser-Materialbearbeitung-55122/",
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
        ]);
        let result_path = temp_dir.path().join("search-run-result.json");
        std::fs::write(&result_path, "stale smoke result").unwrap();

        let summary = run_schott_smoke(
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
            expected_regex_rules(&[
                "Praktik(um|ant)",
                "Werkstudent",
                "Masterthesis",
                "Ausbildung",
            ])
        );
        assert_eq!(search_request.locations, vec![SMOKE_LOCATION]);
        assert_eq!(search_request.radius_km, Some(SMOKE_RADIUS_KM));
        assert_eq!(search_request.source_keys, smoke_source_keys());

        assert_eq!(serialized_label(&summary.result.status), "completed");
        assert_eq!(summary.result.source_runs[0].source_key, SCHOTT_SOURCE_KEY);
        assert_eq!(
            summary.result.source_runs[0].status,
            SourceRunStatus::Completed
        );
        assert_eq!(summary.result.source_runs[0].candidate_count, 7);
        assert_eq!(summary.result.source_runs[0].matched_count, 1);
        assert_eq!(summary.result.source_runs.len(), 1);
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
        assert_eq!(result_json["status"], "completed");
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
    let snapshot = crate::source_profile::registry::load_snapshot(temp_dir.path());

    assert_eq!(created.document.key, SCHOTT_SOURCE_KEY);
    assert_eq!(reused.document.key, SCHOTT_SOURCE_KEY);
    assert_eq!(created.document, reused.document);
    validate_smoke_source(&created).unwrap();
    assert_eq!(smoke_source_keys(), vec![SCHOTT_SOURCE_KEY.to_string()]);
    assert!(temp_dir
        .path()
        .join(format!("sources/{SCHOTT_SOURCE_KEY}.json"))
        .is_file());
    assert_eq!(
        snapshot
            .sources
            .iter()
            .filter(|source| source.document.key == SCHOTT_SOURCE_KEY)
            .count(),
        1
    );
}

#[test]
fn smoke_path_can_target_multiple_existing_sources() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_successfactors_source_file(temp_dir.path(), "schott", "SCHOTT");
        write_successfactors_source_file(temp_dir.path(), "second_sap", "Second SAP");
        let running_search_runs = RunningSearchRuns::default();
        let executor = FixtureSourceExecutor::new([
            (
                "schott",
                Ok(vec![candidate(
                    "Laser Entwicklungsingenieur",
                    "SCHOTT",
                    "https://join.schott.com/job/Mainz-Laser-Entwicklungsingenieur-55122/",
                    &["Mainz"],
                )]),
            ),
            (
                "second_sap",
                Ok(vec![candidate(
                    "Physik Ingenieur",
                    "Second SAP",
                    "https://second.example.test/job/Mainz-Physik-Ingenieur-1001",
                    &["Mainz"],
                )]),
            ),
        ]);

        let summary = run_search_run_smoke(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
            vec!["schott".to_string(), "second_sap".to_string()],
        )
        .await
        .unwrap();

        let search_request = SearchRequestService::new(&pool, &running_search_runs)
            .get(summary.search_request_id)
            .await
            .unwrap();
        assert_eq!(
            search_request.source_keys,
            vec!["schott".to_string(), "second_sap".to_string()]
        );
        assert_eq!(serialized_label(&summary.result.status), "completed");
        assert_eq!(summary.result.source_runs.len(), 2);
        assert_eq!(summary.result.source_runs[0].source_key, "schott");
        assert_eq!(summary.result.source_runs[1].source_key, "second_sap");
        assert_eq!(summary.result.postings.len(), 2);

        let candidates_json: Value = serde_json::from_str(
            &std::fs::read_to_string(temp_dir.path().join("search-run-candidates.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(candidates_json["sources"].as_array().unwrap().len(), 2);
        assert_eq!(candidates_json["sources"][0]["sourceKey"], "schott");
        assert_eq!(
            candidates_json["sources"][0]["candidates"][0]["title"],
            "Laser Entwicklungsingenieur"
        );
        assert_eq!(candidates_json["sources"][1]["sourceKey"], "second_sap");
        assert_eq!(
            candidates_json["sources"][1]["candidates"][0]["title"],
            "Physik Ingenieur"
        );
    });
}

#[test]
fn smoke_path_can_execute_draft_sources_when_allowed_without_persisting_status_change() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_successfactors_source_file_with_status(
            temp_dir.path(),
            "draft_sap",
            "Draft SAP",
            "draft",
        );
        let running_search_runs = RunningSearchRuns::default();
        let executor = FixtureSourceExecutor::new([(
            "draft_sap",
            Ok(vec![candidate(
                "Laser Ingenieur",
                "Draft SAP",
                "https://draft.example.test/job/Mainz-Laser-Ingenieur-1001",
                &["Mainz"],
            )]),
        )]);

        let skipped = run_search_run_smoke(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
            vec!["draft_sap".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(
            skipped.result.source_runs[0].status,
            SourceRunStatus::Skipped
        );

        let allowed = run_search_run_smoke_with_options(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
            vec!["draft_sap".to_string()],
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            allowed.result.source_runs[0].status,
            SourceRunStatus::Completed
        );
        assert_eq!(allowed.result.source_runs[0].candidate_count, 1);

        let persisted_source: Value = serde_json::from_str(
            &std::fs::read_to_string(temp_dir.path().join("sources/draft_sap.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted_source["status"], "draft");
    });
}

#[test]
#[ignore = "network-dependent development smoke path"]
fn smoke_path_reuses_existing_smoke_request_on_later_runs() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_schott_smoke_source_file(temp_dir.path()).unwrap();
        let running_search_runs = RunningSearchRuns::default();
        let executor = FixtureSourceExecutor::new([(
            SCHOTT_SOURCE_KEY,
            Ok(vec![candidate(
                "Laser Ingenieur",
                "SCHOTT",
                "https://join.schott.com/job/Mainz-Laser-Ingenieur-55122/",
                &["Mainz"],
            )]),
        )]);

        let first = run_schott_smoke(
            &pool,
            &running_search_runs,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            temp_dir.path(),
        )
        .await
        .unwrap();
        let second = run_schott_smoke(
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

fn write_successfactors_source_file(app_data_dir: &std::path::Path, key: &str, name: &str) {
    write_successfactors_source_file_with_status(app_data_dir, key, name, "active");
}

fn write_successfactors_source_file_with_status(
    app_data_dir: &std::path::Path,
    key: &str,
    name: &str,
    status: &str,
) {
    std::fs::create_dir_all(app_data_dir.join("sources")).unwrap();
    let document = serde_json::json!({
        "schemaVersion": 3,
        "key": key,
        "name": name,
        "status": status,
        "sourceConfig": {
            "baseUrl": "https://example.test",
            "sitemapUrl": "https://example.test/sitemap.xml"
        },
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "successfactors",
            "pathKey": "rmk_sitemap_html"
        }
    });
    std::fs::write(
        app_data_dir.join(format!("sources/{key}.json")),
        serde_json::to_string_pretty(&document).unwrap(),
    )
    .unwrap();
}

fn expected_regex_rules(values: &[&str]) -> Vec<SearchRule> {
    values
        .iter()
        .map(|value| SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Regex,
            value: (*value).to_string(),
        })
        .collect()
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
        posting_meta: Default::default(),
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
