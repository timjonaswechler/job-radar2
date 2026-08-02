use super::*;
use search_resolution::{
    self as resolution, ScriptedDiscoveryBatch, ScriptedSourceDiscoveryExecution,
};

use crate::search::run::{
    ScriptedResolutionSource, SearchRunResolutionRuntime, SourceExecutionError, SourceRunStatus,
};
use search_requests::Catalog;
use serde_json::Value;
use source_engine::{
    execution::{PhaseCompletion, PhaseExecutionReport, PhaseUsage},
    test_support::ScriptedSourceDetailExecution,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::collections::BTreeMap;

const FIRST_SOURCE_KEY: &str = "fixture_source_one";
const SECOND_SOURCE_KEY: &str = "fixture_source_two";
const FIXTURE_COMPANY: &str = "Fixture Company";

#[derive(Clone)]
struct FixtureCandidate {
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
    posting_meta: BTreeMap<String, String>,
}

fn fixture_resolution_runtime(
    responses: impl IntoIterator<
        Item = (
            &'static str,
            Result<Vec<FixtureCandidate>, SourceExecutionError>,
        ),
    >,
) -> SearchRunResolutionRuntime {
    let limits = crate::search::run::production_resolution_ceilings();
    SearchRunResolutionRuntime::scripted(responses.into_iter().map(|(key, response)| {
        let outcome = match response {
            Ok(candidates) => resolution::ScriptedDiscoveryOutcome::Batch(ScriptedDiscoveryBatch {
                expected_continuation: None,
                expected_maximum: limits.max_batch_size,
                expected_limits: limits.phase,
                occurrences: candidates
                    .into_iter()
                    .map(|candidate| occurrence(key, candidate))
                    .collect(),
                exhausted: true,
                remaining: Some(0),
                continuation: None,
                continuation_source_key: None,
                complete_budget_report: PhaseExecutionReport {
                    usage: PhaseUsage::default(),
                    completion: PhaseCompletion::Accepted,
                },
                diagnostics: Vec::new(),
            }),
            Err(_) => resolution::ScriptedDiscoveryOutcome::ExecutionFailed {
                expected_continuation: None,
                expected_maximum: limits.max_batch_size,
                expected_limits: limits.phase,
                complete_budget_report: PhaseExecutionReport {
                    usage: PhaseUsage::default(),
                    completion: PhaseCompletion::ExecutionFailed,
                },
                diagnostics: Vec::new(),
            },
        };
        (
            key.to_string(),
            ScriptedResolutionSource {
                discovery: ScriptedSourceDiscoveryExecution::new_outcomes(key, [outcome]),
                detail: ScriptedSourceDetailExecution::new([]),
            },
        )
    }))
}

#[test]
fn smoke_cli_requires_at_least_one_explicit_source_key() {
    let error = super::cli::parse_smoke_cli_args([
        std::ffi::OsString::from("--app-data-dir"),
        std::ffi::OsString::from("/tmp/job-radar-smoke"),
    ])
    .err()
    .expect("missing explicit Source selection must be rejected");

    assert_eq!(error, "at least one --source-key is required");
}

#[test]
fn smoke_cli_does_not_consume_another_option_as_a_source_key() {
    let error = super::cli::parse_smoke_cli_args([
        std::ffi::OsString::from("--source-key"),
        std::ffi::OsString::from("--allow-draft"),
    ])
    .err()
    .expect("an option must not be accepted as a Source key");

    assert_eq!(error, "--source-key requires a non-empty source key value");
}

#[test]
fn smoke_path_creates_generic_request_and_writes_bounded_result() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_fixture_source_file(temp_dir.path(), FIRST_SOURCE_KEY, "Fixture Source One");
        let catalog = Catalog::new(pool.clone());
        let executor = fixture_resolution_runtime([(
            FIRST_SOURCE_KEY,
            Ok(vec![
                candidate(
                    "Fixture Engineer",
                    FIXTURE_COMPANY,
                    "https://source-one.example.test/job/fixture-engineer-1001",
                    &["Example City"],
                ),
                candidate(
                    "Fixture Analyst",
                    FIXTURE_COMPANY,
                    "https://source-one.example.test/job/fixture-analyst-1002",
                    &["Example City"],
                ),
            ]),
        )]);
        let result_path = temp_dir.path().join("search-run-result.json");
        std::fs::write(&result_path, "stale smoke result").unwrap();

        let summary = run_search_run_smoke(
            &pool,
            &catalog,
            &executor,
            result_path.clone(),
            sources::installed::Store::new(temp_dir.path()),
            vec![FIRST_SOURCE_KEY.to_string()],
        )
        .await
        .unwrap();

        assert!(summary.search_request_created);
        let search_request = catalog
            .clone()
            .get(search_requests::Id::new(summary.search_request_id).unwrap())
            .await
            .unwrap();
        assert_eq!(search_request.include_rules, expected_smoke_rules());
        assert!(search_request.exclude_rules.is_empty());
        assert!(search_request.locations.is_empty());
        assert_eq!(search_request.radius_km, None);
        assert_eq!(search_request.source_keys, vec![FIRST_SOURCE_KEY]);

        assert_eq!(serialized_label(&summary.result.status), "completed");
        assert_eq!(summary.result.source_runs[0].source_key, FIRST_SOURCE_KEY);
        assert_eq!(
            summary.result.source_runs[0].status,
            SourceRunStatus::Completed
        );
        assert_eq!(
            summary.result.source_runs[0]
                .resolution
                .as_ref()
                .unwrap()
                .counts
                .discovered,
            2
        );
        assert_eq!(summary.result.matched_posting_count, 2);
        let result_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&result_path).unwrap()).unwrap();
        assert_ne!(
            std::fs::read_to_string(&result_path).unwrap(),
            "stale smoke result"
        );
        assert_eq!(result_json["status"], "completed");
        assert_eq!(result_json["postingCount"], 2);
    });
}

#[test]
fn smoke_path_can_target_multiple_existing_sources() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_fixture_source_file(temp_dir.path(), FIRST_SOURCE_KEY, "Fixture Source One");
        write_fixture_source_file(temp_dir.path(), SECOND_SOURCE_KEY, "Fixture Source Two");
        let catalog = Catalog::new(pool.clone());
        let executor = fixture_resolution_runtime([
            (
                FIRST_SOURCE_KEY,
                Ok(vec![candidate(
                    "Fixture Engineer",
                    FIXTURE_COMPANY,
                    "https://source-one.example.test/job/fixture-engineer-1001",
                    &["Example City"],
                )]),
            ),
            (
                SECOND_SOURCE_KEY,
                Ok(vec![candidate(
                    "Fixture Researcher",
                    FIXTURE_COMPANY,
                    "https://source-two.example.test/job/fixture-researcher-2001",
                    &["Example City"],
                )]),
            ),
        ]);

        let source_keys = vec![FIRST_SOURCE_KEY.to_string(), SECOND_SOURCE_KEY.to_string()];
        let summary = run_search_run_smoke(
            &pool,
            &catalog,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
            source_keys.clone(),
        )
        .await
        .unwrap();

        let search_request = catalog
            .clone()
            .get(search_requests::Id::new(summary.search_request_id).unwrap())
            .await
            .unwrap();
        assert_eq!(search_request.source_keys, source_keys);
        assert_eq!(serialized_label(&summary.result.status), "completed");
        assert_eq!(summary.result.source_runs.len(), 2);
        assert_eq!(summary.result.source_runs[0].source_key, FIRST_SOURCE_KEY);
        assert_eq!(summary.result.source_runs[1].source_key, SECOND_SOURCE_KEY);
        assert_eq!(summary.result.matched_posting_count, 2);

        assert!(!temp_dir.path().join("search-run-candidates.json").exists());
        let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM search_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        let match_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM matches")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(run_count, 1);
        assert_eq!(match_count, 2);
    });
}

#[test]
fn smoke_path_can_execute_draft_sources_when_allowed_without_persisting_status_change() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_fixture_source_file_with_status(
            temp_dir.path(),
            FIRST_SOURCE_KEY,
            "Fixture Draft Source",
            "draft",
        );
        let catalog = Catalog::new(pool.clone());
        let executor = fixture_resolution_runtime([(
            FIRST_SOURCE_KEY,
            Ok(vec![candidate(
                "Fixture Engineer",
                FIXTURE_COMPANY,
                "https://source-one.example.test/job/fixture-engineer-1001",
                &["Example City"],
            )]),
        )]);
        let source_keys = vec![FIRST_SOURCE_KEY.to_string()];

        let skipped = run_search_run_smoke(
            &pool,
            &catalog,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
            source_keys.clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            skipped.result.source_runs[0].status,
            SourceRunStatus::Skipped
        );

        let allowed = run_search_run_smoke_with_options(
            &pool,
            &catalog,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
            source_keys,
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            allowed.result.source_runs[0].status,
            SourceRunStatus::Completed
        );

        let persisted_source: Value = serde_json::from_str(
            &std::fs::read_to_string(
                temp_dir
                    .path()
                    .join(format!("sources/{FIRST_SOURCE_KEY}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(persisted_source["status"], "draft");
    });
}

#[test]
fn smoke_path_reuses_existing_smoke_request_on_later_runs() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_fixture_source_file(temp_dir.path(), FIRST_SOURCE_KEY, "Fixture Source One");
        let catalog = Catalog::new(pool.clone());
        let executor = fixture_resolution_runtime([(
            FIRST_SOURCE_KEY,
            Ok(vec![candidate(
                "Fixture Engineer",
                FIXTURE_COMPANY,
                "https://source-one.example.test/job/fixture-engineer-1001",
                &["Example City"],
            )]),
        )]);
        let source_keys = vec![FIRST_SOURCE_KEY.to_string()];

        let first = run_search_run_smoke(
            &pool,
            &catalog,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
            source_keys.clone(),
        )
        .await
        .unwrap();
        let second = run_search_run_smoke(
            &pool,
            &catalog,
            &executor,
            temp_dir.path().join("search-run-result.json"),
            sources::installed::Store::new(temp_dir.path()),
            source_keys,
        )
        .await
        .unwrap();

        assert!(first.search_request_created);
        assert!(!second.search_request_created);
        assert_eq!(first.search_request_id, second.search_request_id);
        assert_eq!(catalog.clone().list().await.unwrap().len(), 1);
    });
}

fn write_fixture_source_file(app_data_dir: &std::path::Path, key: &str, name: &str) {
    write_fixture_source_file_with_status(app_data_dir, key, name, "active");
}

fn write_fixture_source_file_with_status(
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
            "baseUrl": "https://source.example.test",
            "sitemapUrl": "https://source.example.test/sitemap.xml"
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

fn occurrence(
    source_key: &str,
    candidate: FixtureCandidate,
) -> source_engine::execution::PostingOccurrence {
    let (reference, identity) =
        source_engine::execution::validate_posting_reference(source_key, &candidate.url, None)
            .unwrap();
    source_engine::execution::PostingOccurrence {
        identity,
        reference,
        provider_values: source_engine::execution::ProviderValues {
            title: Some(candidate.title),
            company: Some(candidate.company),
            locations: candidate.locations,
            description_text: None,
        },
        hints: Default::default(),
        posting_meta: candidate.posting_meta,
    }
}

fn candidate(title: &str, company: &str, url: &str, locations: &[&str]) -> FixtureCandidate {
    FixtureCandidate {
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
