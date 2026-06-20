use super::*;
use crate::{
    search_request_model::{
        CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
        SearchRequestStatus, SearchRuleInput,
    },
    search_run_model::{
        DefaultSourceExecutor, SearchRunService, SearchRunStatus, SourceCandidate,
        SourceExecutionError, SourceExecutionInput, SourceExecutionSource, SourceExecutor,
        SourceRunStatus,
    },
    source_registry::{BrowserInteraction, ResolvedSelectedAccessPath},
};
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::{collections::HashMap, path::Path, sync::Mutex};

struct FixtureBrowserInventoryClient {
    responses: HashMap<String, Result<String, String>>,
    rendered_requests: Mutex<Vec<(String, Option<BrowserInventoryWait>)>>,
}

impl FixtureBrowserInventoryClient {
    fn new(
        responses: impl IntoIterator<Item = (&'static str, Result<&'static str, &'static str>)>,
    ) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, response)| {
                    (
                        url.to_string(),
                        response.map(str::to_string).map_err(str::to_string),
                    )
                })
                .collect(),
            rendered_requests: Mutex::new(Vec::new()),
        }
    }

    fn rendered_requests(&self) -> Vec<(String, Option<BrowserInventoryWait>)> {
        self.rendered_requests.lock().unwrap().clone()
    }
}

impl BrowserInventoryClient for FixtureBrowserInventoryClient {
    fn render_html(
        &self,
        url: Url,
        wait_for: Option<BrowserInventoryWait>,
    ) -> BoxedBrowserInventoryFuture<'_> {
        Box::pin(async move {
            self.rendered_requests
                .lock()
                .unwrap()
                .push((url.as_str().to_string(), wait_for));
            self.responses
                .get(url.as_str())
                .cloned()
                .unwrap_or_else(|| Err(format!("{} not found", url.as_str())))
        })
    }
}

#[test]
fn browser_inventory_source_runs_through_search_run_with_source_profile() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_browser_access_profile_source(
            temp_dir.path(),
            "browser_inventory_fixture",
            "Browser Inventory Fixture",
            json!({ "startUrl": "https://example.test/jobs" }),
            Some(vec![json!({
                "type": "waitFor",
                "selector": "[data-job-card]",
                "timeoutMs": 15000
            })]),
            Some(browser_inventory_without_navigate_definition()),
        );
        let search_request = create_search_request(
            &pool,
            vec!["browser_inventory_fixture".to_string()],
            "Senior Laser Engineer",
        )
        .await;
        let fixture_browser = FixtureBrowserInventoryClient::new([(
            "https://example.test/jobs",
            Ok(r#"
                <html><body>
                  <article data-job-card>
                    <a href="/jobs/laser">
                      <span data-job-title>  Senior
 Laser   Engineer  </span>
                    </a>
                    <span data-company>	ACME   GmbH
</span>
                    <span data-location> Mainz </span>
                    <span data-location>mainz</span>
                  </article>
                  <article data-job-card>
                    <a href="https://example.test/jobs/chemist">
                      <span data-job-title>Chemist</span>
                    </a>
                    <span data-company>ACME GmbH</span>
                    <span data-location>Berlin</span>
                  </article>
                </body></html>
                "#),
        )]);
        let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
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
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Senior Laser Engineer");
        assert_eq!(posting.company, "ACME GmbH");
        assert_eq!(posting.url, "https://example.test/jobs/laser");
        assert_eq!(posting.locations, vec!["Mainz"]);
        assert_eq!(posting.sources[0].source_key, "browser_inventory_fixture");
        assert_eq!(
            executor.browser.rendered_requests(),
            vec![(
                "https://example.test/jobs".to_string(),
                Some(BrowserInventoryWait {
                    selector: "[data-job-card]".to_string(),
                    timeout_ms: 15_000,
                })
            )]
        );
    });
}

#[test]
fn stepstone_source_profile_builds_query_url_and_extracts_cards_through_search_run() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_builtin_profile_source(
            temp_dir.path(),
            "stepstone_fixture",
            "StepStone Fixture",
            "stepstone_de",
            "browser_inventory",
            json!({ "baseUrl": "https://stepstone.example" }),
        );
        let search_request = SearchRequestService::new(&pool, &RunningSearchRuns::default())
            .create(CreateSearchRequestInput {
                status: SearchRequestStatus::Active,
                include_rules: vec![
                    text_rule("Rust Engineer"),
                    regex_rule("Senior\\s+Developer"),
                    text_rule(" Data "),
                ],
                exclude_rules: vec![],
                locations: vec![" Berlin ".to_string(), "München".to_string()],
                radius_km: Some(50),
                source_keys: vec!["stepstone_fixture".to_string()],
            })
            .await
            .unwrap();
        let fixture_browser = FixtureBrowserInventoryClient::new([(
            "https://stepstone.example/jobs?what=Rust+Engineer+Data&where=Berlin&radius=50",
            Ok(r#"
                <html><body>
                  <article data-at="job-item">
                    <a data-at="job-item-title" href="/stellenangebote--Rust-Engineer-Berlin-ACME--123.html">
                        Rust
                        Engineer
                    </a>
                    <span data-at="job-item-company-name"> ACME   GmbH </span>
                    <span data-at="job-item-location"> Berlin </span>
                    <span data-at="job-item-location">berlin</span>
                  </article>
                  <article data-at="job-item">
                    <a data-at="job-item-title" href="/stellenangebote--Chemist-Berlin-ACME--456.html">
                        Chemist
                    </a>
                    <span data-at="job-item-company-name">ACME GmbH</span>
                    <span data-at="job-item-location">Berlin</span>
                  </article>
                </body></html>
                "#),
        )]);
        let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
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
        assert_eq!(result.source_runs[0].source_key, "stepstone_fixture");
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Completed);
        assert_eq!(result.source_runs[0].candidate_count, 2);
        assert_eq!(result.source_runs[0].matched_count, 1);
        assert_eq!(result.postings.len(), 1);
        let posting = &result.postings[0];
        assert_eq!(posting.title, "Rust Engineer");
        assert_eq!(posting.company, "ACME GmbH");
        assert_eq!(
            posting.url,
            "https://stepstone.example/stellenangebote--Rust-Engineer-Berlin-ACME--123.html"
        );
        assert_eq!(posting.locations, vec!["Berlin"]);
        assert_eq!(posting.sources[0].source_key, "stepstone_fixture");
        assert_eq!(
            executor.browser.rendered_requests(),
            vec![(
                "https://stepstone.example/jobs?what=Rust+Engineer+Data&where=Berlin&radius=50"
                    .to_string(),
                Some(BrowserInventoryWait {
                    selector: "article[data-at=\"job-item\"]".to_string(),
                    timeout_ms: 15_000,
                })
            )]
        );
    });
}

#[test]
fn browser_inventory_executes_from_resolved_execution_plan() {
    tauri::async_runtime::block_on(async {
        let fixture_browser = FixtureBrowserInventoryClient::new([(
            "https://example.test/jobs?what=Engineer",
            Ok(r#"
                <html><body>
                  <article data-job-card>
                    <a href="/jobs/laser">
                      <span data-job-title>Laser Engineer</span>
                    </a>
                    <span data-company>ACME GmbH</span>
                    <span data-location>Mainz</span>
                  </article>
                </body></html>
                "#),
        )]);
        let executor = DeclarativeBrowserInventoryExecutor::new(fixture_browser);
        let search_request = search_request();
        let source = source_with_browser_plan(
            json!({ "baseUrl": "https://example.test" }),
            Some(json!({
                "baseUrl": {
                    "sourceConfigKey": "baseUrl",
                    "default": "https://www.example.test"
                },
                "path": "/jobs",
                "params": [
                    { "name": "what", "value": "{{searchRequest:titleText}}" }
                ]
            })),
            browser_inventory_without_navigate_definition(),
            Some(vec![BrowserInteraction::WaitFor {
                selector: "[data-job-card]".to_string(),
                timeout_ms: Some(15_000),
            }]),
        );

        let candidates = executor
            .execute(SourceExecutionInput {
                search_request: &search_request,
                source: &source,
            })
            .await
            .unwrap();

        assert_eq!(
            candidates,
            vec![SourceCandidate {
                title: "Laser Engineer".to_string(),
                company: "ACME GmbH".to_string(),
                url: "https://example.test/jobs/laser".to_string(),
                locations: vec!["Mainz".to_string()],
            }]
        );
        assert_eq!(
            executor.browser.rendered_requests(),
            vec![(
                "https://example.test/jobs?what=Engineer".to_string(),
                Some(BrowserInventoryWait {
                    selector: "[data-job-card]".to_string(),
                    timeout_ms: 15_000,
                })
            )]
        );
    });
}

#[test]
fn adapter_requires_inventory_plan_before_rendering() {
    tauri::async_runtime::block_on(async {
        let executor =
            DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
        let search_request = search_request();
        let source = source_without_inventory(json!({ "startUrl": "https://example.test/jobs" }));

        let error = executor
            .execute(SourceExecutionInput {
                search_request: &search_request,
                source: &source,
            })
            .await
            .expect_err("browser inventory must require a resolved inventory plan");

        assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "executionPlan.inventory must be a JSON object for source browser_inventory_fixture"
                        .to_string()
                )
            );
        assert!(executor.browser.rendered_requests().is_empty());
    });
}

#[test]
fn source_without_query_requires_start_url_before_rendering() {
    tauri::async_runtime::block_on(async {
        let executor =
            DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
        let search_request = search_request();
        let source = source_with_browser_plan(
            json!({}),
            None,
            browser_inventory_without_navigate_definition(),
            None,
        );

        let error = executor
            .execute(SourceExecutionInput {
                search_request: &search_request,
                source: &source,
            })
            .await
            .expect_err("source-specific browser inventory without query must need startUrl");

        assert_eq!(
                error,
                SourceExecutionError::Failed(
                    "source browser_inventory_fixture requires sourceConfig.startUrl when executionPlan.query is absent"
                        .to_string()
                )
            );
        assert!(executor.browser.rendered_requests().is_empty());
    });
}

#[test]
fn missing_inventory_definition_becomes_failed_source_run() {
    tauri::async_runtime::block_on(async {
        let pool = migrated_pool().await;
        let temp_dir = tempfile::tempdir().unwrap();
        write_browser_access_profile_source(
            temp_dir.path(),
            "browser_inventory_fixture",
            "Browser Inventory Fixture",
            json!({ "startUrl": "https://example.test/jobs" }),
            None,
            None,
        );
        let search_request = create_search_request(
            &pool,
            vec!["browser_inventory_fixture".to_string()],
            "Engineer",
        )
        .await;
        let executor =
            DeclarativeBrowserInventoryExecutor::new(FixtureBrowserInventoryClient::new([]));
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
        assert_eq!(result.source_runs[0].status, SourceRunStatus::Failed);
        assert_eq!(
                result.source_runs[0].error.as_deref(),
                Some("executionPlan.inventory must be a JSON object for source browser_inventory_fixture")
            );
        assert!(result.postings.is_empty());
        assert!(executor.browser.rendered_requests().is_empty());
    });
}

#[test]
fn default_source_executor_routes_browser_inventory_adapter() {
    tauri::async_runtime::block_on(async {
        let executor =
            DefaultSourceExecutor::new(tempfile::tempdir().unwrap().path().join("browser-runtime"));
        let search_request = search_request();
        let source = source_without_inventory(json!({ "startUrl": "https://example.test/jobs" }));

        let error = executor
                .execute(SourceExecutionInput {
                    search_request: &search_request,
                    source: &source,
                })
                .await
                .expect_err(
                    "missing execution-plan inventory should fail before managed browser runtime access",
                );

        match error {
            SourceExecutionError::Failed(message) => {
                assert!(message.contains("executionPlan.inventory"));
                assert!(!message.contains("has no search-run executor yet"));
            }
            SourceExecutionError::Cancelled(message) => {
                panic!("expected failed source execution, got cancellation: {message}")
            }
        }
    });
}

fn write_browser_access_profile_source(
    app_data_dir: &Path,
    source_key: &str,
    source_name: &str,
    source_config: Value,
    interactions: Option<Vec<Value>>,
    inventory: Option<Value>,
) {
    let profile_key = format!("{source_key}_profile");
    let mut access_path = json!({
        "key": "browser_inventory",
        "adapterKey": ADAPTER_KEY,
        "sourceConfigSchema": {
            "type": "object",
            "required": ["startUrl"],
            "properties": {
                "startUrl": { "type": "string", "format": "uri" }
            }
        }
    });
    if let Some(interactions) = interactions {
        access_path["interactions"] = Value::Array(interactions);
    }
    if let Some(inventory) = inventory {
        access_path["inventory"] = inventory;
    }
    write_json(
        app_data_dir.join(format!("source-profiles/{profile_key}.json")),
        &json!({
            "schemaVersion": 1,
            "key": profile_key,
            "name": format!("{source_name} Profile"),
            "kind": "generic",
            "accessPaths": [access_path]
        })
        .to_string(),
    );
    write_builtin_profile_source(
        app_data_dir,
        source_key,
        source_name,
        &profile_key,
        "browser_inventory",
        source_config,
    );
}

fn write_builtin_profile_source(
    app_data_dir: &Path,
    source_key: &str,
    source_name: &str,
    profile_key: &str,
    path_key: &str,
    source_config: Value,
) {
    write_json(
        app_data_dir.join(format!("sources/{source_key}.json")),
        &json!({
            "schemaVersion": 1,
            "key": source_key,
            "name": source_name,
            "status": "active",
            "sourceConfig": source_config,
            "selectedAccessPath": {
                "type": "profile",
                "profileKey": profile_key,
                "pathKey": path_key
            }
        })
        .to_string(),
    );
}

async fn create_search_request(
    pool: &SqlitePool,
    source_keys: Vec<String>,
    include_text: &str,
) -> SearchRequest {
    let running_search_runs = RunningSearchRuns::default();
    SearchRequestService::new(pool, &running_search_runs)
        .create(CreateSearchRequestInput {
            status: SearchRequestStatus::Active,
            include_rules: vec![text_rule(include_text)],
            exclude_rules: vec![],
            locations: vec![],
            radius_km: None,
            source_keys,
        })
        .await
        .unwrap()
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

fn browser_inventory_without_navigate_definition() -> Value {
    json!({
        "items": { "select": "[data-job-card]" },
        "fields": {
            "title": { "selectorText": "[data-job-title]" },
            "company": { "selectorText": "[data-company]" },
            "url": {
                "selectorAttribute": { "selector": "a", "attribute": "href" }
            },
            "locations": [
                { "selectorText": "[data-location]" }
            ]
        }
    })
}

fn search_request() -> SearchRequest {
    SearchRequest {
        id: 1,
        status: SearchRequestStatus::Active,
        include_rules: vec![text_rule("Engineer")]
            .into_iter()
            .map(|rule| crate::search_request_model::SearchRule {
                target: crate::search_request_model::SearchRuleTarget::try_from(
                    rule.target.as_str(),
                )
                .unwrap(),
                kind: crate::search_request_model::SearchRuleKind::try_from(rule.kind.as_str())
                    .unwrap(),
                value: rule.value,
            })
            .collect(),
        exclude_rules: vec![],
        locations: vec![],
        radius_km: None,
        source_keys: vec!["browser_inventory_fixture".to_string()],
        validation_error: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn source_with_browser_plan(
    source_config: Value,
    query: Option<Value>,
    inventory: Value,
    interactions: Option<Vec<BrowserInteraction>>,
) -> SourceExecutionSource {
    source_execution_source(source_config, query, Some(inventory), interactions)
}

fn source_without_inventory(source_config: Value) -> SourceExecutionSource {
    source_execution_source(source_config, None, None, None)
}

fn source_execution_source(
    source_config: Value,
    query: Option<Value>,
    inventory: Option<Value>,
    interactions: Option<Vec<BrowserInteraction>>,
) -> SourceExecutionSource {
    SourceExecutionSource {
        key: "browser_inventory_fixture".to_string(),
        adapter_key: ADAPTER_KEY.to_string(),
        name: "Browser Inventory Fixture".to_string(),
        source_config,
        effective_source_config_schema: json!({ "type": "object" }),
        selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
            query,
            inventory,
            interactions,
            manual_release: None,
        },
    }
}

fn write_json(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
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
