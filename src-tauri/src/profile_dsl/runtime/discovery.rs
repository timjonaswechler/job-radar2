use std::collections::{BTreeMap, HashSet, VecDeque};

use serde_json::{json, Value};

use crate::{
    profile_dsl::primitives::select::resolve_authored_json_path as resolve_simple_json_path,
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::PaginationParameterLocation,
        execution_plan::{
            capabilities::{ExecutionPlanFetch, ExecutionPlanPagination},
            discovery::{ExecutionPlanDiscoveryOutput, ExecutionPlanDiscoveryStrategy},
            SourceExecutionPlan,
        },
        occurrence::{ContributionOrigin, PostingOccurrence},
        policy::StrategyPolicy,
        primitives::{
            acceptance::{
                evaluate_discovery_final_acceptance, evaluate_discovery_strategy_acceptance,
                CompiledAcceptance,
            },
            parse::{CompleteParseText, ParseDiagnosticContext},
            value::{CompiledListValue, CompiledValue},
        },
    },
    source::documents::SourceConfig,
};

use super::{
    allowance::{
        completion_for_stop, diagnostic_for_stop, uses_browser, AllowanceCharge,
        InvocationAllowance, PhaseCancellationReason, PhaseCompletion, PhaseExecutionReport,
        BROWSER_TEARDOWN_RESERVE_MS,
    },
    browser::{
        ProfileBrowserClient, ProfileBrowserFetchError, ProfileBrowserFetchErrorKind,
        ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    },
    cancellation::{
        runtime_execution_cancelled_diagnostic, CancellationOperation, RuntimeExecutionContext,
        RuntimePhase, TypedCancellation,
    },
    http::{ProfileHttpClient, ProfileHttpFailureKind},
    outcome::{
        DiscoveryPhasePayload, PhaseCancelled, PhaseExecutionFailure, PhaseOutcome,
        PhasePreStartFailure, PhaseRunError, PhaseRunResult, PolicyOutcome, PolicyUnsatisfiedCause,
    },
    reducers::{reduce_discovery, DiscoveryContribution},
    strategy_set::{
        execute_strategy_set, policy_unsatisfied_diagnostic, StrategyAttemptCompletion,
        StrategyExecution, StrategySetTerminal,
    },
};

mod diagnostics;
mod document;
mod extract;
mod fetch;
mod pagination;
mod strategy;
mod support;

use diagnostics::{runtime_error, runtime_warning};
use document::select_items;
use extract::extract_candidate;
use fetch::{
    fetch_strategy_document_at_url, fetch_strategy_document_with_query_params,
    DiscoveryFetchOutcome,
};
use pagination::execute_paginated_strategy;
use strategy::{execute_single_strategy_fetch, execute_strategy, extract_candidates_from_items};

pub async fn execute_discovery<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> PhaseRunResult<DiscoveryPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    if plan.discovery.strategies.is_empty() {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: vec![runtime_error(
                "discovery_strategy_missing",
                "discovery does not contain an executable strategy",
                "/discovery/strategies",
                None,
                json!({}),
            )],
        });
    }
    let browser_requires_reserve = plan
        .discovery
        .strategies
        .iter()
        .any(|strategy| uses_browser(&strategy.fetch));
    if browser_requires_reserve
        && plan.discovery.limits.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS
    {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: vec![runtime_error(
                "invalid_compiled_browser_phase_duration",
                "Compiled Discovery Browser duration does not preserve the teardown reserve",
                "/discovery/limits/maxDurationMs",
                None,
                json!({}),
            )],
        });
    }
    if let Some(caller) = context.caller_limits() {
        if !caller.all_positive()
            || !caller.within(plan.discovery.limits)
            || (browser_requires_reserve && caller.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS)
        {
            let diagnostics = vec![runtime_error(
                    "invalid_caller_phase_limits",
                    "Caller phase limits must be positive, may only tighten compiled limits, and must preserve the Browser teardown reserve",
                    "/discovery/limits",
                    None,
                    json!({}),
                )];
            return Ok(PhaseOutcome::ExecutionFailed {
                typed_failure: PhaseExecutionFailure::InvalidCallerLimits,
                complete_budget_report: InvocationAllowance::prestart_failure_report(),
                diagnostics,
            });
        }
    }
    let allowance = InvocationAllowance::new(
        plan.discovery.limits,
        plan.discovery.limits_authored,
        context.caller_limits(),
    );
    let context = context.for_invocation(&allowance);
    if context.is_cancelled() {
        return cancelled_discovery_result(
            TypedCancellation::phase(RuntimePhase::Discovery),
            allowance.report(PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }),
        );
    }

    let execution = execute_strategy_set(
        plan.discovery.policy,
        &plan.discovery.strategies,
        |strategy| strategy.key.as_str(),
        |strategy_index, strategy| {
            context.is_cancelled().then(|| {
                TypedCancellation::strategy(
                    RuntimePhase::Discovery,
                    strategy_index,
                    &strategy.key,
                    CancellationOperation::Phase,
                )
            })
        },
        |strategy_index, strategy| {
            Box::pin(async move {
                if let Err(stop) = context.debit(AllowanceCharge {
                    strategy_attempts: 1,
                    ..AllowanceCharge::default()
                }) {
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(stop),
                    };
                }
                let mut execution = execute_strategy(
                    plan,
                    source_config,
                    fetcher,
                    browser,
                    strategy_index,
                    strategy,
                    plan.discovery.accept_when.as_ref(),
                    context,
                )
                .await;
                if context.stop().is_none() && !context.is_cancelled() {
                    context.mark_deadline_if_expired();
                }
                if let Some(stop) = context.stop() {
                    execution.completion = StrategyAttemptCompletion::Stopped(stop);
                } else if context.is_cancelled()
                    && !matches!(
                        execution.completion,
                        StrategyAttemptCompletion::Cancelled(_)
                    )
                {
                    execution.completion =
                        StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
                            RuntimePhase::Discovery,
                            strategy_index,
                            &strategy.key,
                            CancellationOperation::Phase,
                        ));
                }
                execution
            })
        },
    )
    .await;
    project_discovery_execution(
        execution,
        plan.discovery.policy,
        plan.discovery.accept_when.as_ref(),
        context,
        &allowance,
    )
}

fn project_discovery_execution(
    execution: super::strategy_set::StrategySetExecution<Vec<PostingOccurrence>>,
    policy: StrategyPolicy,
    phase_acceptance: Option<&CompiledAcceptance>,
    context: RuntimeExecutionContext<'_>,
    allowance: &InvocationAllowance,
) -> PhaseRunResult<DiscoveryPhasePayload> {
    let policy_satisfied = match execution.terminal {
        StrategySetTerminal::Satisfied => true,
        StrategySetTerminal::Cancelled(cancellation) => {
            let mut diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .collect::<Diagnostics>();
            diagnostics.push(runtime_execution_cancelled_diagnostic(&cancellation));
            return Err(PhaseRunError::Cancelled(PhaseCancelled {
                complete_budget_report: allowance.report(PhaseCompletion::Cancelled {
                    reason: PhaseCancellationReason::UserCancelled,
                }),
                diagnostics,
            }));
        }
        StrategySetTerminal::Stopped(stop) => {
            let diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .chain(std::iter::once(diagnostic_for_stop(&stop, "/discovery")))
                .collect();
            let completion = completion_for_stop(stop);
            let report = allowance.report(completion.clone());
            return Ok(match completion {
                PhaseCompletion::BudgetExhausted { .. } => PhaseOutcome::BudgetExhausted {
                    complete_budget_report: report,
                    diagnostics,
                },
                PhaseCompletion::ExecutionFailed => PhaseOutcome::ExecutionFailed {
                    typed_failure: PhaseExecutionFailure::Internal,
                    complete_budget_report: report,
                    diagnostics,
                },
                _ => unreachable!("stopped phase has budget or execution-failure completion"),
            });
        }
        StrategySetTerminal::PolicyUnsatisfied => false,
    };

    let mut diagnostics = Vec::new();
    let mut contributions = Vec::new();
    let mut includes_execution_failure = false;
    for (attempt_index, attempt) in execution.attempts.into_iter().enumerate() {
        debug_assert_eq!(attempt.strategy_index, attempt_index);
        debug_assert!(!attempt.strategy_key.is_empty());
        diagnostics.extend(attempt.diagnostics);
        match attempt.completion {
            StrategyAttemptCompletion::Accepted(output) if policy_satisfied => {
                contributions.extend(output.into_iter().enumerate().map(
                    |(item_index, occurrence)| DiscoveryContribution {
                        occurrence,
                        origin: ContributionOrigin {
                            strategy_key: attempt.strategy_key.clone(),
                            attempt_index,
                            provider_item_index: Some(item_index),
                        },
                    },
                ));
            }
            StrategyAttemptCompletion::Failed => includes_execution_failure = true,
            _ => {}
        }
    }

    if !policy_satisfied {
        if context.is_cancelled() {
            diagnostics.push(runtime_execution_cancelled_diagnostic(
                &TypedCancellation::phase(RuntimePhase::Discovery),
            ));
            return Err(PhaseRunError::Cancelled(PhaseCancelled {
                complete_budget_report: allowance.report(PhaseCompletion::Cancelled {
                    reason: PhaseCancellationReason::UserCancelled,
                }),
                diagnostics,
            }));
        }
        diagnostics.push(policy_unsatisfied_diagnostic(
            policy,
            RuntimePhase::Discovery,
        ));
        return Ok(PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::PolicyUnsatisfied {
                cause: if includes_execution_failure {
                    PolicyUnsatisfiedCause::IncludesExecutionFailure
                } else {
                    PolicyUnsatisfiedCause::RejectedOnly
                },
            },
            complete_budget_report: allowance.report(PhaseCompletion::PolicyUnsatisfied),
            diagnostics,
        });
    }
    let reduced = reduce_discovery(contributions);
    diagnostics.extend(reduced.diagnostics);
    let final_accepted = evaluate_discovery_final_acceptance(
        &reduced.candidates,
        phase_acceptance,
        &mut diagnostics,
    )
    .is_satisfied();
    if context.is_cancelled() {
        diagnostics.push(runtime_execution_cancelled_diagnostic(
            &TypedCancellation::phase(RuntimePhase::Discovery),
        ));
        return Err(PhaseRunError::Cancelled(PhaseCancelled {
            complete_budget_report: allowance.report(PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }),
            diagnostics,
        }));
    }
    if !final_accepted && policy.reports_final_rejection() {
        diagnostics.push(policy_unsatisfied_diagnostic(
            policy,
            RuntimePhase::Discovery,
        ));
    }
    let completion = if final_accepted {
        PhaseCompletion::Accepted
    } else {
        PhaseCompletion::PolicyUnsatisfied
    };
    let policy_outcome = if final_accepted {
        PolicyOutcome::Accepted {
            reduced_payload: DiscoveryPhasePayload {
                candidates: reduced.candidates,
                provenance: reduced.provenance,
                conflicts: reduced.conflicts,
                rejections: reduced.rejections,
            },
        }
    } else {
        PolicyOutcome::PolicyUnsatisfied {
            cause: PolicyUnsatisfiedCause::RejectedOnly,
        }
    };
    Ok(PhaseOutcome::Completed {
        policy_outcome,
        complete_budget_report: allowance.report(completion),
        diagnostics,
    })
}

fn cancelled_discovery_result(
    cancellation: TypedCancellation,
    report: PhaseExecutionReport,
) -> PhaseRunResult<DiscoveryPhasePayload> {
    Err(PhaseRunError::Cancelled(PhaseCancelled {
        diagnostics: vec![runtime_execution_cancelled_diagnostic(&cancellation)],
        complete_budget_report: report,
    }))
}

#[cfg(test)]
mod acceptance_projection_tests {
    use super::*;
    use crate::profile_dsl::{
        documents::PhaseLimits,
        primitives::acceptance::CompiledAcceptance,
        runtime::{
            cancellation::RuntimeCancellation,
            strategy_set::{StrategyAttempt, StrategySetExecution},
        },
    };

    struct Cancelled;

    impl RuntimeCancellation for Cancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    #[test]
    fn cancellation_after_policy_exhaustion_wins_without_fallback_summary() {
        let execution = StrategySetExecution::<Vec<PostingOccurrence>> {
            attempts: Vec::new(),
            terminal: StrategySetTerminal::PolicyUnsatisfied,
        };
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        let cancellation = Cancelled;
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);

        let result = project_discovery_execution(
            execution,
            StrategyPolicy::FirstAccepted,
            None,
            context,
            &allowance,
        );

        let PhaseRunError::Cancelled(cancelled) = result.unwrap_err() else {
            panic!("expected typed cancellation")
        };
        assert_eq!(
            cancelled.complete_budget_report.completion,
            PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }
        );
        assert_eq!(
            cancelled
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["runtime_execution_cancelled"]
        );
    }

    #[test]
    fn final_acceptance_is_evaluated_before_cancellation_discards_discovery_payload() {
        let execution = StrategySetExecution {
            attempts: vec![StrategyAttempt {
                strategy_index: 0,
                strategy_key: "accepted".into(),
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(Vec::new()),
            }],
            terminal: StrategySetTerminal::Satisfied,
        };
        let acceptance = CompiledAcceptance {
            required_fields: Vec::new(),
            min_description_length: None,
            min_results: Some(1),
        };
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        let cancellation = Cancelled;
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);

        let result = project_discovery_execution(
            execution,
            StrategyPolicy::FirstAccepted,
            Some(&acceptance),
            context,
            &allowance,
        );

        let PhaseRunError::Cancelled(result) = result.unwrap_err() else {
            panic!("expected typed cancellation")
        };
        assert_eq!(
            result.complete_budget_report.completion,
            PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "acceptance_min_results_not_met",
                "runtime_execution_cancelled"
            ]
        );
    }
}
