use std::{future::Future, path::PathBuf, pin::Pin};

use crate::profile_dsl::{
    diagnostics::DiagnosticSeverity,
    execution_plan::SourceExecutionPlan,
    runtime::{
        execute_posting_discovery_with_clients, ManagedProfileBrowserClient,
        PostingDiscoveryCandidate, ReqwestPostingDiscoveryFetcher,
    },
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
/// execute compiled `postingDiscovery`; they must not use legacy adapter routing.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceExecutionSource {
    pub key: String,
    pub name: String,
    pub execution_plan: SourceExecutionPlan,
}

impl From<SourceExecutionPlan> for SourceExecutionSource {
    fn from(execution_plan: SourceExecutionPlan) -> Self {
        Self {
            key: execution_plan.source.key.clone(),
            name: execution_plan.source.name.clone(),
            execution_plan,
        }
    }
}

pub struct SourceExecutionInput<'a> {
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

fn source_candidate(candidate: PostingDiscoveryCandidate) -> SourceCandidate {
    SourceCandidate {
        title: candidate.title,
        company: candidate.company,
        url: candidate.url,
        locations: candidate.locations,
        posting_meta: candidate.posting_meta,
    }
}
