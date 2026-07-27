use source_profile_dsl::{
    compile_source, execute_detail, execute_discovery, CompileSourceOutcome, Diagnostics,
    PhaseBrowser, PhaseCancelled, PhaseExecutionFailure, PhaseExecutionReport, PhaseOutcome,
    PhasePreStartFailure, PhaseRunError, PhaseRunResult, PolicyOutcome, PolicyUnsatisfiedCause,
    PostingOccurrence, ProfileCompilerInput, ProfileHttpClient, RequestedDetailFields,
    RuntimeExecutionContext, SourceDocument, SourceExecutionPlan, SourceProfileDocument,
};

#[derive(Default)]
pub struct SourceProfileRegistrySnapshot {
    pub profiles: Vec<RegistrySourceProfile>,
    pub sources: Vec<()>,
    pub diagnostics: Diagnostics,
}

pub struct RegistrySourceProfile {
    pub origin: String,
    pub path: String,
    pub document: SourceProfileDocument,
}

impl source_profile_dsl::SourceProfileLookup for SourceProfileRegistrySnapshot {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles
            .iter()
            .map(|entry| &entry.document)
            .find(|profile| profile.key == key)
    }
}

pub fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(future)
}

pub struct AcceptedPhase<P> {
    pub payload: P,
    pub diagnostics: source_profile_dsl::Diagnostics,
    pub report: PhaseExecutionReport,
}

pub fn accepted_phase<P: std::fmt::Debug>(result: PhaseRunResult<P>) -> AcceptedPhase<P> {
    match result {
        Ok(PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::Accepted { reduced_payload },
            complete_budget_report,
            diagnostics,
        }) => AcceptedPhase {
            payload: reduced_payload,
            diagnostics,
            report: complete_budget_report,
        },
        other => panic!("expected accepted phase outcome, got {other:?}"),
    }
}

pub struct PhaseTerminal {
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}

pub fn policy_unsatisfied<P: std::fmt::Debug>(
    result: PhaseRunResult<P>,
    expected_cause: PolicyUnsatisfiedCause,
) -> PhaseTerminal {
    match result {
        Ok(PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::PolicyUnsatisfied { cause },
            complete_budget_report,
            diagnostics,
        }) => {
            assert_eq!(cause, expected_cause);
            PhaseTerminal {
                diagnostics,
                report: complete_budget_report,
            }
        }
        other => panic!("expected policy-unsatisfied phase outcome, got {other:?}"),
    }
}

pub fn budget_exhausted<P: std::fmt::Debug>(result: PhaseRunResult<P>) -> PhaseTerminal {
    match result {
        Ok(PhaseOutcome::BudgetExhausted {
            complete_budget_report,
            diagnostics,
        }) => PhaseTerminal {
            diagnostics,
            report: complete_budget_report,
        },
        other => panic!("expected budget-exhausted phase outcome, got {other:?}"),
    }
}

pub fn execution_failed<P: std::fmt::Debug>(
    result: PhaseRunResult<P>,
    expected_failure: PhaseExecutionFailure,
) -> PhaseTerminal {
    match result {
        Ok(PhaseOutcome::ExecutionFailed {
            typed_failure,
            complete_budget_report,
            diagnostics,
        }) => {
            assert_eq!(typed_failure, expected_failure);
            PhaseTerminal {
                diagnostics,
                report: complete_budget_report,
            }
        }
        other => panic!("expected execution-failed phase outcome, got {other:?}"),
    }
}

pub fn cancelled<P: std::fmt::Debug>(result: PhaseRunResult<P>) -> PhaseCancelled {
    match result {
        Err(PhaseRunError::Cancelled(cancelled)) => cancelled,
        other => panic!("expected cancelled phase result, got {other:?}"),
    }
}

pub fn not_started<P: std::fmt::Debug>(
    result: PhaseRunResult<P>,
    expected_failure: PhasePreStartFailure,
) -> Diagnostics {
    match result {
        Err(PhaseRunError::NotStarted {
            failure,
            diagnostics,
        }) => {
            assert_eq!(failure, expected_failure);
            diagnostics
        }
        other => panic!("expected not-started phase result, got {other:?}"),
    }
}

#[allow(dead_code)]
pub async fn execute_discovery_test<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> AcceptedPhase<source_profile_dsl::DiscoveryPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    execute_discovery_test_with_config(plan, &Default::default(), fetcher).await
}

#[allow(dead_code)]
pub async fn execute_discovery_test_with_config<F>(
    plan: &SourceExecutionPlan,
    source_config: &serde_json::Map<String, serde_json::Value>,
    fetcher: &F,
) -> AcceptedPhase<source_profile_dsl::DiscoveryPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    accepted_phase(
        execute_discovery(
            plan,
            source_config,
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
    )
}

#[allow(dead_code)]
pub async fn execute_discovery_rejected_test<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> PhaseTerminal
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    policy_unsatisfied(
        execute_discovery(
            plan,
            &Default::default(),
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
        PolicyUnsatisfiedCause::RejectedOnly,
    )
}

#[allow(dead_code)]
pub async fn execute_discovery_failed_test<F>(
    plan: &SourceExecutionPlan,
    fetcher: &F,
) -> PhaseTerminal
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    policy_unsatisfied(
        execute_discovery(
            plan,
            &Default::default(),
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
        PolicyUnsatisfiedCause::IncludesExecutionFailure,
    )
}

#[allow(dead_code)]
pub async fn execute_detail_test<F>(
    plan: &SourceExecutionPlan,
    posting: &PostingOccurrence,
    fetcher: &F,
) -> AcceptedPhase<source_profile_dsl::DetailPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    execute_detail_test_with_config(plan, &Default::default(), posting, fetcher).await
}

#[allow(dead_code)]
pub async fn execute_detail_test_with_config<F>(
    plan: &SourceExecutionPlan,
    source_config: &serde_json::Map<String, serde_json::Value>,
    posting: &PostingOccurrence,
    fetcher: &F,
) -> AcceptedPhase<source_profile_dsl::DetailPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    accepted_phase(
        execute_detail(
            plan,
            source_config,
            posting,
            RequestedDetailFields::description_text(),
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
    )
}

#[allow(dead_code)]
pub async fn execute_detail_rejected_test<F>(
    plan: &SourceExecutionPlan,
    posting: &PostingOccurrence,
    fetcher: &F,
) -> PhaseTerminal
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    policy_unsatisfied(
        execute_detail(
            plan,
            &Default::default(),
            posting,
            RequestedDetailFields::description_text(),
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
        PolicyUnsatisfiedCause::RejectedOnly,
    )
}

#[allow(dead_code)]
pub async fn execute_detail_failed_test<F>(
    plan: &SourceExecutionPlan,
    posting: &PostingOccurrence,
    fetcher: &F,
) -> PhaseTerminal
where
    F: ProfileHttpClient + Sync + ?Sized,
{
    policy_unsatisfied(
        execute_detail(
            plan,
            &Default::default(),
            posting,
            RequestedDetailFields::description_text(),
            fetcher,
            PhaseBrowser::BrowserFree,
            RuntimeExecutionContext::uncancellable(),
        )
        .await,
        PolicyUnsatisfiedCause::IncludesExecutionFailure,
    )
}

pub fn compile_test_source(
    source: &SourceDocument,
    profile: Option<SourceProfileDocument>,
) -> CompileSourceOutcome {
    let profiles = profile.into_iter().collect::<Vec<_>>();
    compile_source(source, &ProfileCompilerInput::new(&profiles))
}

pub fn unwrap_plan(outcome: CompileSourceOutcome) -> SourceExecutionPlan {
    match outcome {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } if diagnostics.iter().all(|diagnostic| {
            diagnostic.severity != source_profile_dsl::DiagnosticSeverity::Error
        }) =>
        {
            source.execution_plan
        }
        other => panic!("expected compiled Source, got {other:?}"),
    }
}
