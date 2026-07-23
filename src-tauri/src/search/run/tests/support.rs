pub(super) use crate::search::request::{
    CreateSearchRequestInput, RunningSearchRuns, SearchRequest, SearchRequestService,
    SearchRequestStatus, SearchRuleInput,
};
pub(super) use crate::search::run::{
    SearchRunResolutionRuntime, SearchRunResultArtifact, SearchRunService, SearchRunStatus,
    SourceExecutionError, SourceRunStatus,
};
pub(super) use serde_json::{json, Value};
pub(super) use sqlx::{Row, SqlitePool};
pub(super) use std::collections::BTreeMap;

use crate::{
    profile_dsl::runtime::{
        PhaseCompletion, PhaseExecutionReport, PhaseUsage, ScriptedSourceDetailExecution,
    },
    search::{
        candidate_resolution::{
            ScriptedDiscoveryBatch, ScriptedDiscoveryOutcome, ScriptedSourceDiscoveryExecution,
        },
        run::ScriptedResolutionSource,
    },
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FixtureCandidate {
    pub title: String,
    pub company: String,
    pub url: String,
    pub locations: Vec<String>,
    pub posting_meta: BTreeMap<String, String>,
}

pub(super) fn fixture_resolution_runtime<K: ToString>(
    responses: impl IntoIterator<Item = (K, Result<Vec<FixtureCandidate>, SourceExecutionError>)>,
) -> SearchRunResolutionRuntime {
    let limits = super::super::execution::production_resolution_ceilings();
    let sources = responses.into_iter().map(|(key, response)| {
        let key = key.to_string();
        let outcome = match response {
            Ok(candidates) => ScriptedDiscoveryOutcome::Batch(ScriptedDiscoveryBatch {
                expected_continuation: None,
                expected_maximum: limits.max_batch_size,
                expected_limits: limits.phase,
                occurrences: candidates
                    .into_iter()
                    .map(|candidate| occurrence(&key, candidate))
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
            Err(
                SourceExecutionError::Cancelled(_)
                | SourceExecutionError::CancelledWithDiagnostics { .. },
            ) => ScriptedDiscoveryOutcome::Cancelled {
                expected_continuation: None,
                expected_maximum: limits.max_batch_size,
                expected_limits: limits.phase,
                complete_budget_report: PhaseExecutionReport {
                    usage: PhaseUsage::default(),
                    completion: PhaseCompletion::Cancelled {
                        reason: crate::profile_dsl::runtime::PhaseCancellationReason::UserCancelled,
                    },
                },
                diagnostics: Vec::new(),
            },
            Err(error) => {
                let diagnostics = match error {
                    SourceExecutionError::FailedWithDiagnostics { diagnostics, .. } => diagnostics,
                    _ => Vec::new(),
                };
                ScriptedDiscoveryOutcome::ExecutionFailed {
                    expected_continuation: None,
                    expected_maximum: limits.max_batch_size,
                    expected_limits: limits.phase,
                    complete_budget_report: PhaseExecutionReport {
                        usage: PhaseUsage::default(),
                        completion: PhaseCompletion::ExecutionFailed,
                    },
                    diagnostics,
                }
            }
        };
        (
            key.clone(),
            ScriptedResolutionSource {
                discovery: ScriptedSourceDiscoveryExecution::new_outcomes(key, [outcome]),
                detail: ScriptedSourceDetailExecution::new([]),
            },
        )
    });
    SearchRunResolutionRuntime::scripted(sources)
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
            locations: vec![],
            radius_km: None,
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
        "schemaVersion": 3,
        "key": key,
        "name": name,
        "status": status,
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "fixture_discovery",
            "name": "Fixture Discovery",
            "discovery": minimal_discovery("fixture")
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Deterministic Search Run fixture Source."
        }
    })
    .to_string()
}

pub(super) fn source_json_with_detail(key: &str, name: &str) -> String {
    let mut source: Value = serde_json::from_str(&source_json(key, name)).unwrap();
    source["selectedAccessPath"]["detail"] = minimal_detail();
    source.to_string()
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
        "schemaVersion": 3,
        "key": "mutable_profile",
        "name": "Mutable Profile",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Mutable Search Run fixture profile."
        },
        "accessPaths": [
            {
                "key": "discovery",
                "name": "Posting Discovery",
                "description": marker,
                "sourceConfigSchema": { "type": "object" },
                "discovery": minimal_discovery(marker)
            }
        ]
    })
    .to_string()
}

pub(super) fn mutable_profile_source_json(key: &str, name: &str) -> String {
    json!({
        "schemaVersion": 3,
        "key": key,
        "name": name,
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "mutable_profile",
            "pathKey": "discovery"
        }
    })
    .to_string()
}

pub(super) fn minimal_detail() -> Value {
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [
            {
                "key": "detail",
                "fetch": {
                    "mode": "http",
                    "method": "GET",
                    "url": "https://example.test/detail.json",
                    "timeoutMs": 1000
                },
                "parse": { "type": "json" },
                "select": { "type": "document" },
                "extract": {
                    "fields": {
                        "title": { "type": "json_path", "jsonPath": "$.title" },
                        "company": { "type": "json_path", "jsonPath": "$.company" },
                        "locations": { "type": "json_path", "jsonPath": "$.locations" }
                    }
                }
            }
        ]
    })
}

pub(super) fn minimal_discovery(marker: &str) -> Value {
    json!({
        "policy": { "type": "first_accepted" },
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
                    "reference": {
                        "url": {
                            "type": "json_path",
                            "jsonPath": "$.url",
                            "cardinality": "one"
                        }
                    },
                    "providerValues": {
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
                        "locations": {
                            "type": "json_path",
                            "jsonPath": "$.locations",
                            "cardinality": "all"
                        }
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
) -> FixtureCandidate {
    candidate_with_meta(title, company, url, locations, [])
}

pub(super) fn candidate_with_meta(
    title: &str,
    company: &str,
    url: &str,
    locations: &[&str],
    posting_meta: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> FixtureCandidate {
    FixtureCandidate {
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

pub(super) fn occurrence(
    source_key: &str,
    candidate: FixtureCandidate,
) -> crate::profile_dsl::occurrence::PostingOccurrence {
    let (reference, identity) = crate::profile_dsl::occurrence::validate_posting_reference(
        source_key,
        &candidate.url,
        None,
    )
    .unwrap();
    crate::profile_dsl::occurrence::PostingOccurrence {
        identity,
        reference,
        provider_values: crate::profile_dsl::occurrence::ProviderValues {
            title: Some(candidate.title),
            company: Some(candidate.company),
            locations: candidate.locations,
            description_text: None,
        },
        hints: Default::default(),
        posting_meta: candidate.posting_meta,
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
