use std::{future::Future, path::PathBuf, pin::Pin};

use crate::{
    background_tasks::CancellationToken,
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        execution_plan::SourceExecutionPlan,
        runtime::{
            execute_posting_discovery_with_clients, ManagedProfileBrowserClient,
            PostingDiscoveryCandidate, PostingDiscoveryFetchError, PostingDiscoveryFetchRequest,
            PostingDiscoveryFetchResponse, PostingDiscoveryFetcher, ProfileBrowserClient,
            ProfileBrowserFetchError, ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest,
            ProfileBrowserFetchResponse, ReqwestPostingDiscoveryFetcher,
        },
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
    /// Cooperative Search Run cancellation token propagated from the background task.
    /// Executors should stop active runtime work promptly where their local seams allow it.
    pub cancellation_token: Option<&'a CancellationToken>,
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
            execute_posting_discovery_for_source(input, &fetcher, &browser).await
        })
    }
}

async fn execute_posting_discovery_for_source<F, B>(
    input: SourceExecutionInput<'_>,
    fetcher: &F,
    browser: &B,
) -> Result<SourceExecutionOutput, SourceExecutionError>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if input
        .cancellation_token
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(source_execution_cancelled_error(Vec::new()));
    }

    let fetcher = CancellablePostingDiscoveryFetcher {
        inner: fetcher,
        cancellation_token: input.cancellation_token,
    };
    let browser = CancellableProfileBrowserClient {
        inner: browser,
        cancellation_token: input.cancellation_token,
    };
    let result =
        execute_posting_discovery_with_clients(&input.source.execution_plan, &fetcher, &browser)
            .await;

    if input
        .cancellation_token
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(source_execution_cancelled_error(result.diagnostics));
    }

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
}

struct CancellablePostingDiscoveryFetcher<'a, F: ?Sized> {
    inner: &'a F,
    cancellation_token: Option<&'a CancellationToken>,
}

impl<F> PostingDiscoveryFetcher for CancellablePostingDiscoveryFetcher<'_, F>
where
    F: PostingDiscoveryFetcher + Sync + ?Sized,
{
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let Some(cancellation_token) = self.cancellation_token else {
                return self.inner.fetch(request).await;
            };
            if cancellation_token.is_cancelled() {
                return Err(posting_discovery_cancelled_fetch_error());
            }

            tokio::select! {
                result = self.inner.fetch(request) => result,
                _ = cancellation_token.cancelled() => Err(posting_discovery_cancelled_fetch_error()),
            }
        })
    }
}

struct CancellableProfileBrowserClient<'a, B: ?Sized> {
    inner: &'a B,
    cancellation_token: Option<&'a CancellationToken>,
}

impl<B> ProfileBrowserClient for CancellableProfileBrowserClient<'_, B>
where
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fn render<'a>(
        &'a self,
        request: ProfileBrowserFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProfileBrowserFetchResponse, ProfileBrowserFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let Some(cancellation_token) = self.cancellation_token else {
                return self.inner.render(request).await;
            };
            if cancellation_token.is_cancelled() {
                return Err(profile_browser_cancelled_fetch_error());
            }

            let result = self.inner.render(request).await;
            if cancellation_token.is_cancelled() {
                return Err(profile_browser_cancelled_fetch_error());
            }
            result
        })
    }
}

fn posting_discovery_cancelled_fetch_error() -> PostingDiscoveryFetchError {
    PostingDiscoveryFetchError::new("postingDiscovery cancelled")
}

fn profile_browser_cancelled_fetch_error() -> ProfileBrowserFetchError {
    ProfileBrowserFetchError::new(
        ProfileBrowserFetchErrorKind::RuntimeUnavailable,
        "postingDiscovery cancelled",
    )
}

fn source_execution_cancelled_error(mut diagnostics: Diagnostics) -> SourceExecutionError {
    diagnostics.push(source_execution_cancelled_diagnostic());
    SourceExecutionError::CancelledWithDiagnostics {
        message: "postingDiscovery cancelled".to_string(),
        diagnostics,
    }
}

fn source_execution_cancelled_diagnostic() -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_execution_cancelled".to_string(),
        message: "postingDiscovery cancelled".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "/postingDiscovery".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({ "reason": "search_run_cancelled" })),
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
