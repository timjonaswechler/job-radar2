use std::{future::Future, path::PathBuf, pin::Pin};

use serde_json::Value;

use crate::{
    profile_dsl::{
        diagnostics::DiagnosticSeverity,
        execution_plan::SourceExecutionPlan,
        runtime::{
            execute_posting_discovery_with_clients, ManagedProfileBrowserClient,
            PostingDiscoveryCandidate, ReqwestPostingDiscoveryFetcher,
        },
    },
    search::request::SearchRequest,
    source::registry::{BrowserInteraction, ResolvedSelectedAccessPath},
};

use super::{SourceCandidate, SourceExecutionError};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceExecutionOutput {
    pub candidates: Vec<SourceCandidate>,
    pub diagnostics: crate::profile_dsl::diagnostics::Diagnostics,
}

impl From<Vec<SourceCandidate>> for SourceExecutionOutput {
    fn from(candidates: Vec<SourceCandidate>) -> Self {
        Self {
            candidates,
            diagnostics: Vec::new(),
        }
    }
}

impl std::ops::Deref for SourceExecutionOutput {
    type Target = Vec<SourceCandidate>;

    fn deref(&self) -> &Self::Target {
        &self.candidates
    }
}

impl PartialEq<Vec<SourceCandidate>> for SourceExecutionOutput {
    fn eq(&self, other: &Vec<SourceCandidate>) -> bool {
        &self.candidates == other
    }
}

pub type BoxedSourceExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SourceExecutionOutput, SourceExecutionError>> + Send + 'a>>;

/// Public source-execution seam used by Search Runs.
///
/// `SearchRunService` resolves selected Source keys through one Source Profile
/// registry snapshot at run start, compiles active valid Sources into immutable
/// typed Execution Plans, and passes those plans here. Active Search Runs must
/// execute compiled `postingDiscovery`; they must not dispatch on `adapterKey`.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceExecutionSource {
    pub key: String,
    pub name: String,
    pub adapter_key: String,
    pub source_config: Value,
    pub effective_source_config_schema: Value,
    pub selected_access_path: ResolvedSelectedAccessPath,
    pub execution_plan: SourceExecutionPlan,
}

impl SourceExecutionSource {
    #[allow(dead_code)]
    pub(crate) fn query(&self) -> Option<&Value> {
        match &self.selected_access_path {
            ResolvedSelectedAccessPath::Profile { query, .. }
            | ResolvedSelectedAccessPath::SourceSpecific { query, .. } => query.as_ref(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn inventory(&self) -> Option<&Value> {
        match &self.selected_access_path {
            ResolvedSelectedAccessPath::Profile { inventory, .. }
            | ResolvedSelectedAccessPath::SourceSpecific { inventory, .. } => inventory.as_ref(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn interactions(&self) -> Option<&[BrowserInteraction]> {
        match &self.selected_access_path {
            ResolvedSelectedAccessPath::Profile { interactions, .. }
            | ResolvedSelectedAccessPath::SourceSpecific { interactions, .. } => {
                interactions.as_deref()
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn manual_release(&self) -> Option<&Value> {
        match &self.selected_access_path {
            ResolvedSelectedAccessPath::Profile { manual_release, .. }
            | ResolvedSelectedAccessPath::SourceSpecific { manual_release, .. } => {
                manual_release.as_ref()
            }
        }
    }
}

impl From<SourceExecutionPlan> for SourceExecutionSource {
    fn from(execution_plan: SourceExecutionPlan) -> Self {
        Self {
            key: execution_plan.source.key.clone(),
            name: execution_plan.source.name.clone(),
            adapter_key: "compiled_posting_discovery".to_string(),
            source_config: Value::Object(execution_plan.source_config.clone()),
            effective_source_config_schema: serde_json::json!({ "type": "object" }),
            selected_access_path: ResolvedSelectedAccessPath::SourceSpecific {
                query: None,
                inventory: None,
                interactions: None,
                manual_release: None,
            },
            execution_plan,
        }
    }
}

pub struct SourceExecutionInput<'a> {
    pub search_request: &'a SearchRequest,
    pub source: &'a SourceExecutionSource,
}

pub trait SourceExecutor: Send + Sync {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a>;
}

pub struct DefaultSourceExecutor {
    browser_runtime_dir: PathBuf,
}

impl DefaultSourceExecutor {
    pub fn new(browser_runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            browser_runtime_dir: browser_runtime_dir.into(),
        }
    }
}

impl SourceExecutor for DefaultSourceExecutor {
    fn execute<'a>(&'a self, input: SourceExecutionInput<'a>) -> BoxedSourceExecutionFuture<'a> {
        Box::pin(async move {
            let fetcher = ReqwestPostingDiscoveryFetcher::new();
            let browser = ManagedProfileBrowserClient::new(self.browser_runtime_dir.clone());
            let result = execute_posting_discovery_with_clients(
                &input.source.execution_plan,
                &fetcher,
                &browser,
            )
            .await;
            let execution_failed = result.candidates.is_empty()
                && result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);

            if execution_failed {
                let message = result
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "postingDiscovery failed".to_string());
                return Err(SourceExecutionError::FailedWithDiagnostics {
                    message,
                    diagnostics: result.diagnostics,
                });
            }

            Ok(SourceExecutionOutput {
                candidates: result
                    .candidates
                    .into_iter()
                    .map(source_candidate)
                    .collect(),
                diagnostics: result.diagnostics,
            })
        })
    }
}

#[cfg(test)]
pub(crate) fn fixture_source_execution_plan(
    key: &str,
    name: &str,
    source_config: Value,
) -> SourceExecutionPlan {
    serde_json::from_value(serde_json::json!({
        "source": { "key": key, "name": name },
        "selectedAccessPath": {
            "type": "source_owned_access_path",
            "key": "fixture",
            "name": "Fixture"
        },
        "sourceConfig": source_config,
        "postingDiscovery": {
            "strategies": [
                {
                    "key": "fixture",
                    "fetch": {
                        "mode": "http",
                        "method": "GET",
                        "url": "https://example.test/jobs.json",
                        "timeoutMs": 1000
                    },
                    "parse": { "type": "json" },
                    "select": { "type": "json_path", "jsonPath": "$.jobs" },
                    "extract": {
                        "fields": {
                            "title": { "type": "json_path", "jsonPath": "$.title", "cardinality": "one" },
                            "company": { "type": "json_path", "jsonPath": "$.company", "cardinality": "one" },
                            "url": { "type": "json_path", "jsonPath": "$.url", "cardinality": "one" }
                        }
                    }
                }
            ]
        }
    }))
    .expect("fixture execution plan should deserialize")
}

fn source_candidate(candidate: PostingDiscoveryCandidate) -> SourceCandidate {
    SourceCandidate {
        title: candidate.title,
        company: candidate.company,
        url: candidate.url,
        locations: candidate.locations,
        posting_meta: candidate.posting_meta,
    }
}
