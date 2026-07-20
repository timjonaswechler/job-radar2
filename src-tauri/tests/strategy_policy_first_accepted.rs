use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use job_radar_lib::{
    compile_source, execute_policy_detail_with_clients_and_context,
    execute_policy_discovery_with_clients_and_context, CompileSourceOutcome, DetailFetchError,
    DetailFetchRequest, DetailFetchResponse, DetailFetcher, DetailPostingOccurrence,
    DiscoveryFetchError, DiscoveryFetchRequest, DiscoveryFetchResponse, DiscoveryFetcher,
    PolicyDiscoveryStep, PolicySourceDocument, PolicySourceExecutionPlan,
    PolicySourceProfileDocument, PolicySourceProfileRegistrySnapshot, RuntimeCancellation,
    RuntimeExecutionContext, StrategyPolicy, UnavailableProfileBrowserClient,
};
use serde_json::{json, Value};

#[test]
fn final_strategy_set_requires_the_closed_first_accepted_policy() {
    let strategy_set = json!({ "strategies": [] });
    serde_json::from_value::<PolicyDiscoveryStep>(strategy_set)
        .expect_err("a final Strategy Set without Policy must be rejected");

    let strategy_set = json!({ "policy": "first_accepted", "strategies": [] });
    serde_json::from_value::<PolicyDiscoveryStep>(strategy_set)
        .expect_err("a raw string Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "unknown" },
        "strategies": []
    });
    serde_json::from_value::<PolicyDiscoveryStep>(strategy_set)
        .expect_err("an unknown Policy must be rejected");

    let strategy_set = json!({
        "policy": { "type": "first_accepted", "extra": true },
        "strategies": []
    });
    serde_json::from_value::<PolicyDiscoveryStep>(strategy_set)
        .expect_err("additional Policy properties must be rejected");

    let strategy_set: PolicyDiscoveryStep = serde_json::from_value(json!({
        "policy": { "type": "first_accepted" },
        "strategies": []
    }))
    .unwrap();
    assert_eq!(strategy_set.policy, StrategyPolicy::FirstAccepted);
    assert_eq!(
        serde_json::to_value(strategy_set.policy).unwrap(),
        json!({ "type": "first_accepted" })
    );
}

#[test]
fn final_compiler_preserves_policy_for_inherited_specialized_added_and_source_owned_sets() {
    let profile = profile_document();

    let inherited = compile(profile_source(None, "main"), profile.clone());
    assert_plan_policies(&inherited);

    let specialized = compile(
        profile_source(
            Some(json!([{
                "key": "main",
                "postingDiscovery": {
                    "policy": { "type": "first_accepted" },
                    "strategies": [discovery_strategy(
                        "source_added",
                        "https://example.test/discovery/source-added"
                    )]
                },
                "postingDetail": { "policy": { "type": "first_accepted" } }
            }])),
            "main",
        ),
        profile.clone(),
    );
    assert_plan_policies(&specialized);
    assert_eq!(
        specialized.discovery.execution.strategies.len(),
        4,
        "a complete Source-added Strategy must retain the inherited Policy"
    );

    let added_path = json!({
        "key": "added",
        "name": "Added path",
        "postingDiscovery": discovery_step(),
        "postingDetail": detail_step()
    });
    let added = compile(profile_source(Some(json!([added_path])), "added"), profile);
    assert_plan_policies(&added);

    let source_owned: PolicySourceDocument = serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "owned",
        "name": "Owned",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "owned_path",
            "name": "Owned path",
            "postingDiscovery": discovery_step(),
            "postingDetail": detail_step()
        },
        "sourceSupport": {
            "level": "experimental",
            "summary": "Deterministic test source."
        }
    }))
    .unwrap();
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = compile_source(
        &source_owned,
        &PolicySourceProfileRegistrySnapshot::default(),
    )
    else {
        panic!("valid Source-owned final path must compile");
    };
    assert!(diagnostics.is_empty());
    assert!(matches!(
        source.access,
        job_radar_lib::CompiledSourceAccess::SourceOwned { .. }
    ));
    assert_plan_policies(&source.execution_plan);
}

#[test]
fn first_accepted_execution_is_ordered_and_recovers_for_both_phases() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = ScriptedDiscoveryFetcher::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({
                "jobs": [{
                    "title": "Rejected partial output",
                    "company": "Example",
                    "url": "https://example.test/jobs/rejected"
                }]
            })
            .to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({
                "jobs": [{
                    "title": "Platform Engineer",
                    "company": "Example",
                    "url": "https://example.test/jobs/1"
                }]
            })
            .to_string()),
        ),
    ]);

    let result = block_on(execute_policy_discovery_with_clients_and_context(
        &plan,
        &discovery,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(result.candidates[0].title, "Platform Engineer");
    assert_eq!(
        discovery.requests(),
        vec![
            "https://example.test/discovery/empty",
            "https://example.test/discovery/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["acceptance_min_results_not_met",]
    );

    let detail = ScriptedDetailFetcher::new([
        (
            "https://example.test/detail/failed",
            Ok(json!({ "description": "Rejected partial detail." }).to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Ok(json!({ "description": "A complete accepted description." }).to_string()),
        ),
    ]);
    let result = block_on(execute_policy_detail_with_clients_and_context(
        &plan,
        &posting(),
        &detail,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert_eq!(
        result.description_text.as_deref(),
        Some("A complete accepted description.")
    );
    assert_eq!(
        detail.requests(),
        vec![
            "https://example.test/detail/failed",
            "https://example.test/detail/accepted",
        ]
    );
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["description_too_short",]
    );
}

#[test]
fn first_accepted_execution_stops_after_an_accepted_first_attempt() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = ScriptedDiscoveryFetcher::new([(
        "https://example.test/discovery/empty",
        Ok(json!({
            "jobs": [
                {
                    "title": "First accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/first"
                },
                {
                    "title": "Also accepted",
                    "company": "Example",
                    "url": "https://example.test/jobs/second"
                }
            ]
        })
        .to_string()),
    )]);
    let result = block_on(execute_policy_discovery_with_clients_and_context(
        &plan,
        &discovery,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(discovery.requests().len(), 1);
    assert!(result.diagnostics.is_empty());

    let accepted_description = "accepted ".repeat(20);
    let detail = ScriptedDetailFetcher::new([(
        "https://example.test/detail/failed",
        Ok(json!({ "description": accepted_description }).to_string()),
    )]);
    let result = block_on(execute_policy_detail_with_clients_and_context(
        &plan,
        &posting(),
        &detail,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));
    assert!(result.description_text.is_some());
    assert_eq!(detail.requests().len(), 1);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn first_accepted_exhaustion_adds_one_terminal_after_attempt_diagnostics() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let discovery = ScriptedDiscoveryFetcher::new([
        (
            "https://example.test/discovery/empty",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/accepted",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
        (
            "https://example.test/discovery/unused",
            Ok(json!({ "jobs": [] }).to_string()),
        ),
    ]);

    let result = block_on(execute_policy_discovery_with_clients_and_context(
        &plan,
        &discovery,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));

    assert!(result.candidates.is_empty());
    let codes = result
        .diagnostics
        .iter()
        .map(|d| d.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "acceptance_min_results_not_met",
            "fallback_exhausted",
        ]
    );
    assert_eq!(
        result.diagnostics.last().unwrap().path,
        "/postingDiscovery/strategies"
    );

    let detail = ScriptedDetailFetcher::new([
        (
            "https://example.test/detail/failed",
            Err("failed one".to_string()),
        ),
        (
            "https://example.test/detail/accepted",
            Err("failed two".to_string()),
        ),
        (
            "https://example.test/detail/unused",
            Err("failed three".to_string()),
        ),
    ]);
    let result = block_on(execute_policy_detail_with_clients_and_context(
        &plan,
        &posting(),
        &detail,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::uncancellable(),
    ));
    assert!(result.description_text.is_none());
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "fetch_failed",
            "fetch_failed",
            "fetch_failed",
            "fallback_exhausted"
        ]
    );
    assert_eq!(
        result.diagnostics.last().unwrap().path,
        "/postingDetail/strategies"
    );
}

#[test]
fn cancellation_discards_an_accepted_attempt_and_suppresses_later_work_and_exhaustion() {
    let plan = compile(profile_source(None, "main"), profile_document());
    let cancellation = Arc::new(AtomicBool::new(false));
    let fetcher = CancellingDiscoveryFetcher {
        cancellation: cancellation.clone(),
        requests: Mutex::new(Vec::new()),
    };
    let signal = AtomicCancellation(cancellation);

    let result = block_on(execute_policy_discovery_with_clients_and_context(
        &plan,
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::with_cancellation(&signal),
    ));

    assert!(result.candidates.is_empty());
    assert_eq!(fetcher.requests.lock().unwrap().len(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "fallback_exhausted"));

    let cancellation = Arc::new(AtomicBool::new(false));
    let fetcher = CancellingDetailFetcher {
        cancellation: cancellation.clone(),
        requests: Mutex::new(Vec::new()),
    };
    let signal = AtomicCancellation(cancellation);
    let result = block_on(execute_policy_detail_with_clients_and_context(
        &plan,
        &posting(),
        &fetcher,
        &UnavailableProfileBrowserClient,
        RuntimeExecutionContext::with_cancellation(&signal),
    ));
    assert!(result.description_text.is_none());
    assert_eq!(fetcher.requests.lock().unwrap().len(), 1);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "runtime_execution_cancelled")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != "fallback_exhausted"));
}

fn assert_plan_policies(plan: &PolicySourceExecutionPlan) {
    assert_eq!(plan.discovery.policy, StrategyPolicy::FirstAccepted);
    assert_eq!(
        plan.detail.as_ref().unwrap().policy,
        StrategyPolicy::FirstAccepted
    );
}

fn compile(
    source: PolicySourceDocument,
    profile: PolicySourceProfileDocument,
) -> PolicySourceExecutionPlan {
    let outcome = compile_source(
        &source,
        &PolicySourceProfileRegistrySnapshot {
            profiles: vec![profile],
            sources: Vec::new(),
        },
    );
    let CompileSourceOutcome::Compiled {
        source,
        diagnostics,
    } = outcome
    else {
        panic!("valid final source must compile: {outcome:?}");
    };
    assert!(diagnostics.is_empty());
    source.execution_plan
}

fn profile_document() -> PolicySourceProfileDocument {
    serde_json::from_value(json!({
        "schemaVersion": 3,
        "key": "policy_profile",
        "name": "Policy profile",
        "kind": "generic",
        "support": {
            "level": "experimental",
            "summary": "Deterministic policy test profile."
        },
        "accessPaths": [{
            "key": "main",
            "name": "Main",
            "postingDiscovery": discovery_step(),
            "postingDetail": detail_step()
        }]
    }))
    .unwrap()
}

fn profile_source(access_paths: Option<Value>, path_key: &str) -> PolicySourceDocument {
    let mut value = json!({
        "schemaVersion": 3,
        "key": "policy_source",
        "name": "Policy source",
        "status": "active",
        "sourceConfig": {},
        "selectedAccessPath": {
            "type": "profile_access_path",
            "profileKey": "policy_profile",
            "pathKey": path_key
        }
    });
    if let Some(access_paths) = access_paths {
        value["accessPaths"] = access_paths;
    }
    serde_json::from_value(value).unwrap()
}

fn discovery_step() -> Value {
    let mut rejected = discovery_strategy("empty", "https://example.test/discovery/empty");
    rejected["acceptWhen"] = json!({ "minResults": 2 });
    json!({
        "policy": { "type": "first_accepted" },
        "acceptWhen": { "minResults": 1 },
        "strategies": [
            rejected,
            discovery_strategy("accepted", "https://example.test/discovery/accepted"),
            discovery_strategy("unused", "https://example.test/discovery/unused")
        ]
    })
}

fn discovery_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
        "parse": { "type": "json" },
        "select": { "type": "json_path", "jsonPath": "$.jobs" },
        "extract": {
            "fields": {
                "title": { "type": "json_path", "jsonPath": "$.title" },
                "company": { "type": "json_path", "jsonPath": "$.company" },
                "url": { "type": "json_path", "jsonPath": "$.url" }
            }
        }
    })
}

fn detail_step() -> Value {
    let mut rejected = detail_strategy("failed", "https://example.test/detail/failed");
    rejected["acceptWhen"] = json!({ "minDescriptionLength": 100 });
    json!({
        "policy": { "type": "first_accepted" },
        "strategies": [
            rejected,
            detail_strategy("accepted", "https://example.test/detail/accepted"),
            detail_strategy("unused", "https://example.test/detail/unused")
        ]
    })
}

fn detail_strategy(key: &str, url: &str) -> Value {
    json!({
        "key": key,
        "fetch": { "mode": "http", "method": "GET", "url": url, "timeoutMs": 1000 },
        "parse": { "type": "json" },
        "select": { "type": "document" },
        "extract": {
            "fields": {
                "descriptionText": { "type": "json_path", "jsonPath": "$.description" }
            }
        }
    })
}

fn posting() -> DetailPostingOccurrence {
    DetailPostingOccurrence {
        url: "https://example.test/jobs/1".to_string(),
        title: None,
        company: None,
        locations: Vec::new(),
        description_text: None,
        posting_meta: BTreeMap::new(),
    }
}

type DiscoveryScript = BTreeMap<String, Result<String, String>>;

struct ScriptedDiscoveryFetcher {
    script: DiscoveryScript,
    requests: Mutex<Vec<String>>,
}

impl ScriptedDiscoveryFetcher {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            script: entries
                .into_iter()
                .map(|(url, result)| (url.to_string(), result))
                .collect(),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl DiscoveryFetcher for ScriptedDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.url.clone());
            match self
                .script
                .get(&request.url)
                .cloned()
                .unwrap_or_else(|| Err("unexpected request".to_string()))
            {
                Ok(body) => Ok(DiscoveryFetchResponse { body }),
                Err(message) => Err(DiscoveryFetchError::new(message)),
            }
        })
    }
}

struct ScriptedDetailFetcher {
    script: Mutex<VecDeque<(String, Result<String, String>)>>,
    requests: Mutex<Vec<String>>,
}

impl ScriptedDetailFetcher {
    fn new<const N: usize>(entries: [(&str, Result<String, String>); N]) -> Self {
        Self {
            script: Mutex::new(
                entries
                    .into_iter()
                    .map(|(url, result)| (url.to_string(), result))
                    .collect(),
            ),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl DetailFetcher for ScriptedDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.url.clone());
            let (expected, result) = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected detail request");
            assert_eq!(request.url, expected);
            match result {
                Ok(body) => Ok(DetailFetchResponse { body }),
                Err(message) => Err(DetailFetchError::new(message)),
            }
        })
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

struct AtomicCancellation(Arc<AtomicBool>);

impl RuntimeCancellation for AtomicCancellation {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

struct CancellingDiscoveryFetcher {
    cancellation: Arc<AtomicBool>,
    requests: Mutex<Vec<String>>,
}

struct CancellingDetailFetcher {
    cancellation: Arc<AtomicBool>,
    requests: Mutex<Vec<String>>,
}

impl DetailFetcher for CancellingDetailFetcher {
    fn fetch<'a>(
        &'a self,
        request: DetailFetchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DetailFetchResponse, DetailFetchError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.url);
            self.cancellation.store(true, Ordering::SeqCst);
            Ok(DetailFetchResponse {
                body: json!({ "description": "Discarded detail description." }).to_string(),
            })
        })
    }
}

impl DiscoveryFetcher for CancellingDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: DiscoveryFetchRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<DiscoveryFetchResponse, DiscoveryFetchError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request.url);
            self.cancellation.store(true, Ordering::SeqCst);
            Ok(DiscoveryFetchResponse {
                body: json!({
                    "jobs": [{
                        "title": "Discarded",
                        "company": "Example",
                        "url": "https://example.test/jobs/discarded"
                    }]
                })
                .to_string(),
            })
        })
    }
}
