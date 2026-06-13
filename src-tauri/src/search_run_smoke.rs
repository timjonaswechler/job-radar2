use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::{ffi::OsString, path::PathBuf};

use crate::{
    paths::AppPaths,
    search_request_model::{
        CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
        SearchRequestStatus, SearchRule, SearchRuleInput, SearchRuleKind, SearchRuleTarget,
    },
    search_run_model::{
        default_search_run_result_path, DefaultSourceExecutor, SearchRunResult, SearchRunService,
        SourceExecutor,
    },
    source_model::{
        create_source, list_sources, list_system_profiles, CreateSourceInput, Source, SourceStatus,
    },
};

const SMOKE_COMMAND: &str = "dev-search-run-smoke";
const SMOKE_APP_DATA_DIR_ENV: &str = "JOB_RADAR_SMOKE_APP_DATA_DIR";
const SCHOTT_SOURCE_KEY: &str = "schott_careers";
const SCHOTT_ADAPTER_KEY: &str = "declarative_sitemap_jobboard";
const SCHOTT_SOURCE_NAME: &str = "SCHOTT Karriere";
const SCHOTT_SITEMAP_URL: &str = "https://join.schott.com/sitemap.xml";
const STEPSTONE_SOURCE_KEY: &str = "stepstone_de";
const STEPSTONE_ADAPTER_KEY: &str = "stepstone_search";
const SUCCESSFACTORS_PROFILE_KEY: &str = "successfactors";
const SMOKE_LOCATION: &str = "Mainz";
const SMOKE_RADIUS_KM: i64 = 30;
const INCLUDE_RULE_VALUES: &[&str] = &["Physik", "Laser"];
const EXCLUDE_RULE_VALUES: &[&str] = &["Praktikum", "Werkstudent", "Schülerpraktikum"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchRunSmokeSummary {
    pub search_request_id: i64,
    pub search_request_created: bool,
    pub result_path: String,
    pub result: SearchRunResult,
}

struct SmokeCliOptions {
    app_data_dir: Option<PathBuf>,
    ensure_schott_source: bool,
    help: bool,
}

pub fn run_dev_search_run_smoke_cli<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
{
    let options = parse_smoke_cli_args(args)?;
    if options.help {
        println!("{}", smoke_cli_help());
        return Ok(());
    }

    let app_data_dir = options
        .app_data_dir
        .or_else(|| std::env::var_os(SMOKE_APP_DATA_DIR_ENV).map(PathBuf::from))
        .ok_or_else(|| {
            format!(
                "missing --app-data-dir <path> or {SMOKE_APP_DATA_DIR_ENV}; see docs/dev-search-run-smoke.md"
            )
        })?;

    tauri::async_runtime::block_on(async move {
        let paths = AppPaths::from_app_data_dir(app_data_dir).map_err(|error| error.to_string())?;
        let state = crate::app_state::AppState::new(paths)
            .await
            .map_err(|error| error.to_string())?;

        if options.ensure_schott_source {
            ensure_schott_smoke_source(&state.db).await?;
        }

        let result_path = default_search_run_result_path();
        let source_executor = DefaultSourceExecutor::new(state.paths.browser_runtime_dir.clone());
        let summary = run_schott_stepstone_smoke(
            &state.db,
            &state.running_search_runs,
            &source_executor,
            result_path,
        )
        .await?;

        print_smoke_summary(&summary);
        Ok(())
    })
}

pub(crate) async fn run_schott_stepstone_smoke(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_executor: &dyn SourceExecutor,
    result_path: impl Into<PathBuf>,
) -> Result<SearchRunSmokeSummary, String> {
    let result_path = result_path.into();
    let source_ids = smoke_source_ids(pool).await?;
    let (search_request, search_request_created) =
        get_or_create_smoke_search_request(pool, running_search_runs, source_ids).await?;

    let result = SearchRunService::new(
        pool,
        running_search_runs,
        source_executor,
        result_path.clone(),
    )
    .run(search_request.id)
    .await?;

    Ok(SearchRunSmokeSummary {
        search_request_id: search_request.id,
        search_request_created,
        result_path: result_path.to_string_lossy().to_string(),
        result,
    })
}

async fn get_or_create_smoke_search_request(
    pool: &SqlitePool,
    running_search_runs: &RunningSearchRuns,
    source_ids: Vec<i64>,
) -> Result<(SearchRequest, bool), String> {
    let service = SearchRequestService::new(pool, running_search_runs);
    for search_request in service.list().await? {
        if is_smoke_search_request(&search_request, &source_ids) {
            return Ok((search_request, false));
        }
    }

    let created = service
        .create(smoke_search_request_input(source_ids))
        .await?;
    Ok((created, true))
}

fn is_smoke_search_request(search_request: &SearchRequest, source_ids: &[i64]) -> bool {
    search_request.status == SearchRequestStatus::Active
        && search_request.include_rules == expected_rules(INCLUDE_RULE_VALUES)
        && search_request.exclude_rules == expected_rules(EXCLUDE_RULE_VALUES)
        && search_request.locations == vec![SMOKE_LOCATION.to_string()]
        && search_request.radius_km == Some(SMOKE_RADIUS_KM)
        && search_request.source_ids == source_ids
        && search_request.validation_error.is_none()
}

fn smoke_search_request_input(source_ids: Vec<i64>) -> CreateSearchRequestInput {
    CreateSearchRequestInput {
        status: SearchRequestStatus::Active,
        include_rules: INCLUDE_RULE_VALUES
            .iter()
            .map(|value| text_rule_input(value))
            .collect(),
        exclude_rules: EXCLUDE_RULE_VALUES
            .iter()
            .map(|value| text_rule_input(value))
            .collect(),
        locations: vec![SMOKE_LOCATION.to_string()],
        radius_km: Some(SMOKE_RADIUS_KM),
        source_ids,
    }
}

fn expected_rules(values: &[&str]) -> Vec<SearchRule> {
    values
        .iter()
        .map(|value| SearchRule {
            target: SearchRuleTarget::Title,
            kind: SearchRuleKind::Text,
            value: (*value).to_string(),
        })
        .collect()
}

fn text_rule_input(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "text".to_string(),
        value: value.to_string(),
    }
}

async fn smoke_source_ids(pool: &SqlitePool) -> Result<Vec<i64>, String> {
    let sources = list_sources(pool).await?;
    let schott = require_smoke_source(&sources, SCHOTT_SOURCE_KEY)?;
    let stepstone = require_smoke_source(&sources, STEPSTONE_SOURCE_KEY)?;

    validate_smoke_source(schott, SCHOTT_ADAPTER_KEY)?;
    validate_smoke_source(stepstone, STEPSTONE_ADAPTER_KEY)?;
    validate_schott_source_config(schott)?;

    Ok(vec![schott.id, stepstone.id])
}

fn require_smoke_source<'a>(sources: &'a [Source], key: &str) -> Result<&'a Source, String> {
    sources
        .iter()
        .find(|source| source.key == key)
        .ok_or_else(|| {
            format!(
                "smoke source `{key}` was not found in the local DB; see docs/dev-search-run-smoke.md"
            )
        })
}

fn validate_smoke_source(source: &Source, expected_adapter_key: &str) -> Result<(), String> {
    if source.adapter_key != expected_adapter_key {
        return Err(format!(
            "smoke source `{}` must use adapterKey `{expected_adapter_key}`, found `{}`",
            source.key, source.adapter_key
        ));
    }

    if source.status != SourceStatus::Active {
        return Err(format!(
            "smoke source `{}` must be active, found {:?}",
            source.key, source.status
        ));
    }

    Ok(())
}

fn validate_schott_source_config(source: &Source) -> Result<(), String> {
    let url = source
        .source_config
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let recursive = source
        .source_config
        .get("recursive")
        .and_then(|value| value.as_bool());

    if url != SCHOTT_SITEMAP_URL || recursive != Some(false) {
        return Err(format!(
            "smoke source `{}` must use sourceConfig {{\"url\":\"{}\",\"recursive\":false}}",
            source.key, SCHOTT_SITEMAP_URL
        ));
    }

    Ok(())
}

async fn ensure_schott_smoke_source(pool: &SqlitePool) -> Result<Source, String> {
    let sources = list_sources(pool).await?;
    if let Some(source) = sources
        .iter()
        .find(|source| source.key == SCHOTT_SOURCE_KEY)
    {
        validate_smoke_source(source, SCHOTT_ADAPTER_KEY)?;
        validate_schott_source_config(source)?;
        return Ok(source.clone());
    }

    let system_profile = list_system_profiles(pool)
        .await?
        .into_iter()
        .find(|profile| profile.key == SUCCESSFACTORS_PROFILE_KEY)
        .ok_or_else(|| {
            format!(
                "built-in system profile `{SUCCESSFACTORS_PROFILE_KEY}` is missing; cannot create `{SCHOTT_SOURCE_KEY}`"
            )
        })?;

    create_source(
        pool,
        CreateSourceInput {
            key: SCHOTT_SOURCE_KEY.to_string(),
            adapter_key: SCHOTT_ADAPTER_KEY.to_string(),
            system_profile_id: Some(system_profile.id),
            browser_profile_id: None,
            name: SCHOTT_SOURCE_NAME.to_string(),
            description: Some(
                "Development smoke source for SCHOTT sitemap validation.".to_string(),
            ),
            source_config: json!({
                "url": SCHOTT_SITEMAP_URL,
                "recursive": false
            }),
            status: SourceStatus::Active,
            validation_error: None,
        },
    )
    .await
}

fn parse_smoke_cli_args<I>(args: I) -> Result<SmokeCliOptions, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut app_data_dir = None;
    let mut ensure_schott_source = false;
    let mut help = false;
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            help = true;
            continue;
        }
        if arg == "--ensure-schott-source" {
            ensure_schott_source = true;
            continue;
        }
        if arg == "--app-data-dir" {
            let value = args
                .next()
                .ok_or_else(|| "--app-data-dir requires a path".to_string())?;
            app_data_dir = Some(PathBuf::from(value));
            continue;
        }

        let arg_string = arg.to_string_lossy();
        if let Some(value) = arg_string.strip_prefix("--app-data-dir=") {
            if value.is_empty() {
                return Err("--app-data-dir requires a path".to_string());
            }
            app_data_dir = Some(PathBuf::from(value));
            continue;
        }

        return Err(format!(
            "unknown {SMOKE_COMMAND} argument `{}`; use --help",
            arg_string
        ));
    }

    Ok(SmokeCliOptions {
        app_data_dir,
        ensure_schott_source,
        help,
    })
}

fn smoke_cli_help() -> &'static str {
    "Usage: cargo run --manifest-path src-tauri/Cargo.toml -- dev-search-run-smoke --app-data-dir <path> [--ensure-schott-source]\n\nRuns the network-dependent SCHOTT + StepStone development smoke Suchlauf and overwrites search-run-result.json in the repository root. Use JOB_RADAR_SMOKE_APP_DATA_DIR instead of --app-data-dir if preferred."
}

fn print_smoke_summary(summary: &SearchRunSmokeSummary) {
    println!("Search-run smoke completed");
    println!("Search request ID: {}", summary.search_request_id);
    println!(
        "Search request: {}",
        if summary.search_request_created {
            "created"
        } else {
            "reused"
        }
    );
    println!("Result path: {}", summary.result_path);
    println!(
        "Overall status: {}",
        serialized_label(&summary.result.status)
    );
    println!("Postings: {}", summary.result.postings.len());
    println!("Source runs:");
    for source_run in &summary.result.source_runs {
        let error = source_run.error.as_deref().unwrap_or("-");
        println!(
            "- {}: status={}, candidates={}, matched={}, error={}",
            source_run.source_key,
            serialized_label(&source_run.status),
            source_run.candidate_count,
            source_run.matched_count,
            error
        );
    }
}

fn serialized_label<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .map(|value| value.trim_matches('"').to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        search_run_model::{
            BoxedSourceExecutionFuture, SourceCandidate, SourceExecutionError,
            SourceExecutionInput, SourceRunStatus,
        },
        source_model::{
            create_browser_profile, create_system_profile, CreateBrowserProfileInput,
            CreateSystemProfileInput,
        },
    };
    use serde_json::Value;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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
        fn execute<'a>(
            &'a self,
            input: SourceExecutionInput<'a>,
        ) -> BoxedSourceExecutionFuture<'a> {
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
            let (schott_id, stepstone_id) = create_smoke_sources(&pool).await;
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
            let temp_dir = tempfile::tempdir().unwrap();
            let result_path = temp_dir.path().join("search-run-result.json");
            std::fs::write(&result_path, "stale smoke result").unwrap();

            let summary = run_schott_stepstone_smoke(
                &pool,
                &running_search_runs,
                &executor,
                result_path.clone(),
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
            assert_eq!(search_request.source_ids, vec![schott_id, stepstone_id]);

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
    fn ensure_schott_source_creates_only_missing_local_smoke_source() {
        tauri::async_runtime::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let pool = crate::db::connect_and_migrate(
                &temp_dir.path().join("job_radar.db"),
                &temp_dir.path().join("system-profiles"),
            )
            .await
            .unwrap();

            let created = ensure_schott_smoke_source(&pool).await.unwrap();
            let reused = ensure_schott_smoke_source(&pool).await.unwrap();
            let sources = list_sources(&pool).await.unwrap();

            assert_eq!(created.id, reused.id);
            assert_eq!(created.key, SCHOTT_SOURCE_KEY);
            assert_eq!(created.adapter_key, SCHOTT_ADAPTER_KEY);
            assert_eq!(created.name, SCHOTT_SOURCE_NAME);
            assert_eq!(
                created.source_config,
                json!({
                    "url": SCHOTT_SITEMAP_URL,
                    "recursive": false
                })
            );
            assert!(sources
                .iter()
                .any(|source| source.key == STEPSTONE_SOURCE_KEY));
            assert_eq!(
                sources
                    .iter()
                    .filter(|source| source.key == SCHOTT_SOURCE_KEY)
                    .count(),
                1
            );
        });
    }

    #[test]
    fn smoke_path_reuses_existing_smoke_request_on_later_runs() {
        tauri::async_runtime::block_on(async {
            let pool = migrated_pool().await;
            create_smoke_sources(&pool).await;
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
            let temp_dir = tempfile::tempdir().unwrap();

            let first = run_schott_stepstone_smoke(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
            )
            .await
            .unwrap();
            let second = run_schott_stepstone_smoke(
                &pool,
                &running_search_runs,
                &executor,
                temp_dir.path().join("search-run-result.json"),
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

    async fn create_smoke_sources(pool: &SqlitePool) -> (i64, i64) {
        let system_profile = create_system_profile(
            pool,
            CreateSystemProfileInput {
                key: SUCCESSFACTORS_PROFILE_KEY.to_string(),
                name: "SAP SuccessFactors".to_string(),
                description: None,
                adapter_key: SCHOTT_ADAPTER_KEY.to_string(),
                definition_schema_version: 1,
                definition: json!({
                    "detect": {
                        "required": [
                            { "htmlContains": "SAP SuccessFactors" }
                        ]
                    }
                }),
                source_config_schema: json!({
                    "type": "object",
                    "required": ["url"],
                    "properties": {
                        "url": { "type": "string", "format": "uri" },
                        "recursive": { "type": "boolean" }
                    }
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();
        let schott = create_source(
            pool,
            CreateSourceInput {
                key: SCHOTT_SOURCE_KEY.to_string(),
                adapter_key: SCHOTT_ADAPTER_KEY.to_string(),
                system_profile_id: Some(system_profile.id),
                browser_profile_id: None,
                name: SCHOTT_SOURCE_NAME.to_string(),
                description: None,
                source_config: json!({
                    "url": SCHOTT_SITEMAP_URL,
                    "recursive": false
                }),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        let browser_profile = create_browser_profile(
            pool,
            CreateBrowserProfileInput {
                key: "job_portal_manual_release".to_string(),
                name: "Job-Portal Browserfreigabe".to_string(),
                description: None,
                name_i18n_key: None,
                description_i18n_key: None,
                definition_path: None,
                definition_hash: None,
                definition_schema_version: 1,
                definition: json!({}),
                source_config_schema: json!({}),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();
        let stepstone = create_source(
            pool,
            CreateSourceInput {
                key: STEPSTONE_SOURCE_KEY.to_string(),
                adapter_key: STEPSTONE_ADAPTER_KEY.to_string(),
                system_profile_id: None,
                browser_profile_id: Some(browser_profile.id),
                name: "StepStone Deutschland".to_string(),
                description: None,
                source_config: json!({}),
                status: SourceStatus::Active,
                validation_error: None,
            },
        )
        .await
        .unwrap();

        (schott.id, stepstone.id)
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
}
