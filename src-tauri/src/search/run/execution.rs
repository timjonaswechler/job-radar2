use std::{future::Future, path::PathBuf, pin::Pin};

use crate::{
    background_tasks::CancellationToken,
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        execution_plan::SourceExecutionPlan,
        runtime::{
            execute_discovery, ManagedProfileBrowserClient, PostingOccurrence,
            ProfileBrowserClient, ProfileHttpClient, ReqwestProfileHttpClient,
            RuntimeExecutionContext,
        },
    },
    source::documents::SourceConfig,
};

use super::{SourceCandidate, SourceExecutionError};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceExecutionOutput {
    pub occurrences: Vec<PostingOccurrence>,
    pub diagnostics: crate::profile_dsl::diagnostics::Diagnostics,
}

pub type BoxedSourceExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SourceExecutionOutput, SourceExecutionError>> + Send + 'a>>;

/// Public source-execution seam used by Search Runs.
///
/// `SearchRunService` resolves selected Source keys through one Source Profile
/// registry snapshot at run start, compiles active valid Sources into immutable
/// typed Execution Plans, and passes those plans here. Active Search Runs must
/// execute compiled Discovery; they must not use adapter routing.
#[derive(Clone, PartialEq)]
pub struct SourceExecutionSource {
    pub key: String,
    pub name: String,
    source_config: SourceConfig,
    pub execution_plan: SourceExecutionPlan,
}

impl std::fmt::Debug for SourceExecutionSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceExecutionSource")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("execution_plan", &self.execution_plan)
            .finish_non_exhaustive()
    }
}

impl SourceExecutionSource {
    pub fn new(execution_plan: SourceExecutionPlan, source_config: SourceConfig) -> Self {
        Self {
            key: execution_plan.source.key.clone(),
            name: execution_plan.source.name.clone(),
            source_config,
            execution_plan,
        }
    }

    pub(crate) fn source_config(&self) -> &SourceConfig {
        &self.source_config
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
            let fetcher = ReqwestProfileHttpClient::new();
            let browser = ManagedProfileBrowserClient::new(self.browser_runtime_dir.clone());
            execute_discovery_for_source(input, &fetcher, &browser).await
        })
    }
}

pub(super) async fn execute_discovery_for_source<F, B>(
    input: SourceExecutionInput<'_>,
    fetcher: &F,
    browser: &B,
) -> Result<SourceExecutionOutput, SourceExecutionError>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if input
        .cancellation_token
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(source_execution_cancelled_error(Vec::new()));
    }

    let context = input
        .cancellation_token
        .map(|token| RuntimeExecutionContext::with_cancellation(token))
        .unwrap_or_else(RuntimeExecutionContext::uncancellable);
    let result = execute_discovery(
        &input.source.execution_plan,
        input.source.source_config(),
        fetcher,
        browser,
        context,
    )
    .await;

    use crate::profile_dsl::runtime::{PhaseOutcome, PhaseRunError, PolicyOutcome};

    let outcome = match result {
        Ok(outcome) => outcome,
        Err(PhaseRunError::Cancelled(cancelled)) => {
            return Err(source_execution_cancelled_error(cancelled.diagnostics));
        }
        Err(PhaseRunError::NotStarted { diagnostics, .. }) => {
            return Err(discovery_failed_error(
                "Discovery did not start",
                diagnostics,
            ));
        }
    };

    match outcome {
        PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::Accepted { reduced_payload },
            diagnostics,
            ..
        } => Ok(SourceExecutionOutput {
            occurrences: reduced_payload.candidates,
            diagnostics,
        }),
        PhaseOutcome::Completed { diagnostics, .. } => Err(discovery_failed_error(
            "Discovery Policy was unsatisfied",
            diagnostics,
        )),
        PhaseOutcome::BudgetExhausted { diagnostics, .. } => Err(discovery_failed_error(
            "Discovery budget was exhausted",
            diagnostics,
        )),
        PhaseOutcome::ExecutionFailed { diagnostics, .. } => Err(discovery_failed_error(
            "Discovery execution failed",
            diagnostics,
        )),
    }
}

fn discovery_failed_error(
    message: impl Into<String>,
    diagnostics: Diagnostics,
) -> SourceExecutionError {
    SourceExecutionError::FailedWithDiagnostics {
        message: message.into(),
        diagnostics,
    }
}

fn source_execution_cancelled_error(mut diagnostics: Diagnostics) -> SourceExecutionError {
    diagnostics.push(source_execution_cancelled_diagnostic());
    SourceExecutionError::CancelledWithDiagnostics {
        message: "Discovery cancelled".to_string(),
        diagnostics,
    }
}

fn source_execution_cancelled_diagnostic() -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_execution_cancelled".to_string(),
        message: "Discovery cancelled".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "/discovery".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({ "reason": "search_run_cancelled" })),
    }
}

pub(crate) fn source_candidate(occurrence: PostingOccurrence) -> Option<SourceCandidate> {
    Some(SourceCandidate {
        title: occurrence.provider_values.title?,
        company: occurrence.provider_values.company?,
        url: occurrence.reference.provider_url,
        locations: occurrence.provider_values.locations,
        posting_meta: occurrence.posting_meta,
    })
}
