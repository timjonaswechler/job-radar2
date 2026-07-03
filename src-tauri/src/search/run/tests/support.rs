pub(super) use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRuleInput,
};
pub(super) use crate::search::run::{
    BoxedSourceExecutionFuture, SearchRunResultArtifact, SearchRunService, SearchRunStatus,
    SourceCandidate, SourceExecutionError, SourceExecutionInput, SourceExecutor, SourceRunStatus,
};
pub(super) use serde_json::{json, Value};
pub(super) use sqlx::{Row, SqlitePool};
pub(super) use std::collections::BTreeMap;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(super) struct FixtureSourceExecutor {
    responses: Mutex<HashMap<String, Result<Vec<SourceCandidate>, SourceExecutionError>>>,
}

impl FixtureSourceExecutor {
    pub(super) fn new<K: ToString>(
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

pub(super) struct RuntimePostingDiscoveryExecutor {
    response_body: String,
}

impl RuntimePostingDiscoveryExecutor {
    pub(super) fn new(response_body: impl Into<String>) -> Self {
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

pub(super) struct FixturePostingDiscoveryFetcher {
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

pub(super) struct RegistryMutatingPlanCaptureExecutor {
    profile_path: PathBuf,
    seen_discovery_markers: Mutex<Vec<(String, String)>>,
}

impl RegistryMutatingPlanCaptureExecutor {
    pub(super) fn new(profile_path: PathBuf) -> Self {
        Self {
            profile_path,
            seen_discovery_markers: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn seen_discovery_markers(&self) -> Vec<(String, String)> {
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

pub(super) async fn create_test_search_request(
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

pub(super) fn write_test_sources(app_data_dir: &Path, sources: &[(&str, &str)]) -> Vec<String> {
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

pub(super) fn source_json(key: &str, name: &str) -> String {
    source_json_with_status(key, name, "active")
}

pub(super) fn source_json_with_status(key: &str, name: &str, status: &str) -> String {
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

pub(super) fn text_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "text".to_string(),
        value: value.to_string(),
    }
}

pub(super) fn regex_rule(value: &str) -> SearchRuleInput {
    SearchRuleInput {
        target: "title".to_string(),
        kind: "regex".to_string(),
        value: value.to_string(),
    }
}

pub(super) fn mutable_profile_json(marker: &str) -> String {
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

pub(super) fn mutable_profile_source_json(key: &str, name: &str) -> String {
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

pub(super) fn minimal_posting_discovery(marker: &str) -> Value {
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

pub(super) fn write_json(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

pub(super) async fn wait_for_background_task_state(
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

pub(super) fn candidate(
    title: &str,
    company: &str,
    url: &str,
    locations: &[&str],
) -> SourceCandidate {
    candidate_with_meta(title, company, url, locations, [])
}

pub(super) fn candidate_with_meta(
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

pub(super) async fn migrated_pool() -> SqlitePool {
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
