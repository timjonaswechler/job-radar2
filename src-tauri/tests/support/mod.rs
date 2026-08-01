use crate::job_radar_lib::{
    __test_execute_detail_phase as execute_detail, compile_source, execute_discovery,
    CompileSourceOutcome, Diagnostics, PhaseBrowser, PhaseCancelled, PhaseExecutionFailure,
    PhaseExecutionReport, PhaseOutcome, PhasePreStartFailure, PhaseRunError, PhaseRunResult,
    PolicyOutcome, PolicyUnsatisfiedCause, PostingOccurrence, ProfileHttpClient,
    RequestedDetailFields, RuntimeExecutionContext, SourceDocument, SourceExecutionPlan,
    SourceProfileDocument, SourceProfileLookup,
};

pub struct AcceptedPhase<P> {
    pub payload: P,
    pub diagnostics: crate::job_radar_lib::Diagnostics,
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
) -> AcceptedPhase<crate::job_radar_lib::DiscoveryPhasePayload>
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
) -> AcceptedPhase<crate::job_radar_lib::DiscoveryPhasePayload>
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
) -> AcceptedPhase<crate::job_radar_lib::DetailPhasePayload>
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
) -> AcceptedPhase<crate::job_radar_lib::DetailPhasePayload>
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
    struct TestProfiles(Vec<SourceProfileDocument>);
    impl SourceProfileLookup for TestProfiles {
        fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
            self.0.iter().find(|profile| profile.key == key)
        }
    }

    let behavior = source_engine::definition::SourceBehavior {
        key: source.key.clone(),
        name: source.name.clone(),
        source_config: source.source_config.clone(),
        selected_access_path: source.selected_access_path.clone(),
        access_paths: source.access_paths.clone(),
        source_support: source.source_support.clone(),
    };
    compile_source(&behavior, &TestProfiles(profile.into_iter().collect()))
}

pub fn unwrap_plan(outcome: CompileSourceOutcome) -> SourceExecutionPlan {
    match outcome {
        CompileSourceOutcome::Compiled {
            source,
            diagnostics,
        } if diagnostics.iter().all(|diagnostic| {
            diagnostic.severity != crate::job_radar_lib::DiagnosticSeverity::Error
        }) =>
        {
            source_engine::test_support::test_execution_plan(&source)
        }
        other => panic!("expected compiled Source, got {other:?}"),
    }
}
