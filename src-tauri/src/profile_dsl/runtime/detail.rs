use std::collections::BTreeMap;

use dom_query::NodeRef;
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        execution_plan::{
            capabilities::ExecutionPlanFetch, detail::ExecutionPlanDetailStrategy,
            SourceExecutionPlan,
        },
        occurrence::{
            ContributionOrigin, DetailField, DetailPatch, PostingOccurrence, RequestedDetailFields,
        },
        primitives::{
            acceptance::{
                evaluate_detail_final_acceptance, evaluate_detail_strategy_acceptance,
                validate_detail_acceptance_request, CompiledAcceptance,
            },
            parse::{CompleteParseText, ParseDiagnosticContext},
            predicate::CompiledPredicate,
            transform::normalize_whitespace_text,
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
        DetailPhasePayload, PhaseCancelled, PhaseExecutionFailure, PhaseOutcome,
        PhasePreStartFailure, PhaseRunError, PhaseRunResult, PolicyOutcome, PolicyUnsatisfiedCause,
    },
    reducers::{reduce_detail, DetailContribution},
    strategy_set::{
        execute_first_accepted, StrategyAttemptCompletion, StrategyExecution, StrategySetTerminal,
    },
};

mod diagnostics;
mod document;
mod extract;
mod fetch;
mod strategy;
mod support;

use diagnostics::runtime_error;
use document::{select_detail_document, RuntimeItem};
use extract::{
    evaluate_predicate, evaluate_strategy_captures, evaluate_value_list, evaluate_value_scalar,
};
use fetch::fetch_strategy_document;
use strategy::execute_strategy;

pub async fn execute_detail<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &PostingOccurrence,
    requested_fields: RequestedDetailFields,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> PhaseRunResult<DetailPhasePayload>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let Some(detail) = &plan.detail else {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: vec![runtime_error(
                "detail_missing",
                "Execution Plan does not contain compiled detail",
                "/detail",
                None,
                json!({}),
            )],
        });
    };

    if let Some(diagnostic) = validate_detail_acceptance_request(
        detail.accept_when.as_ref(),
        detail
            .strategies
            .iter()
            .enumerate()
            .map(|(index, strategy)| {
                (
                    format!("/detail/strategies/{index}"),
                    strategy.key.clone(),
                    strategy.accept_when.as_ref(),
                )
            }),
        &requested_fields,
    ) {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::RequestMismatch,
            diagnostics: vec![diagnostic],
        });
    }

    if detail.strategies.is_empty() {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: vec![runtime_error(
                "detail_strategy_missing",
                "detail does not contain an executable strategy",
                "/detail/strategies",
                None,
                json!({}),
            )],
        });
    }
    let browser_requires_reserve = detail
        .strategies
        .iter()
        .any(|strategy| uses_browser(&strategy.fetch));
    if browser_requires_reserve && detail.limits.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS {
        return Err(PhaseRunError::NotStarted {
            failure: PhasePreStartFailure::PlanMismatch,
            diagnostics: vec![runtime_error(
                "invalid_compiled_browser_phase_duration",
                "Compiled Detail Browser duration does not preserve the teardown reserve",
                "/detail/limits/maxDurationMs",
                None,
                json!({}),
            )],
        });
    }
    if let Some(caller) = context.caller_limits() {
        if !caller.all_positive()
            || !caller.within(detail.limits)
            || (browser_requires_reserve && caller.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS)
        {
            let diagnostics = vec![runtime_error(
                    "invalid_caller_phase_limits",
                    "Caller phase limits must be positive, may only tighten compiled limits, and must preserve the Browser teardown reserve",
                    "/detail/limits",
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
        detail.limits,
        detail.limits_authored,
        context.caller_limits(),
    );
    let context = context.for_invocation(&allowance);
    if context.is_cancelled() {
        return cancelled_detail_result(
            TypedCancellation::phase(RuntimePhase::Detail),
            allowance.report(PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }),
        );
    }

    let execution = execute_first_accepted(
        &detail.strategies,
        |strategy| strategy.key.as_str(),
        |strategy_index, strategy| {
            context.is_cancelled().then(|| {
                TypedCancellation::strategy(
                    RuntimePhase::Detail,
                    strategy_index,
                    &strategy.key,
                    CancellationOperation::Phase,
                )
            })
        },
        |strategy_index, strategy| {
            let requested_fields = requested_fields.clone();
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
                    posting,
                    &requested_fields,
                    fetcher,
                    browser,
                    strategy_index,
                    strategy,
                    detail.accept_when.as_ref(),
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
                            RuntimePhase::Detail,
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
    project_detail_execution(
        execution,
        posting,
        &requested_fields,
        detail.accept_when.as_ref(),
        context,
        &allowance,
    )
}

fn project_detail_execution(
    execution: super::strategy_set::StrategySetExecution<DetailPatch>,
    posting: &PostingOccurrence,
    requested_fields: &RequestedDetailFields,
    phase_acceptance: Option<&CompiledAcceptance>,
    context: RuntimeExecutionContext<'_>,
    allowance: &InvocationAllowance,
) -> PhaseRunResult<DetailPhasePayload> {
    let accepted_attempt = match execution.terminal {
        StrategySetTerminal::Accepted { attempt_index } => Some(attempt_index),
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
                .chain(std::iter::once(diagnostic_for_stop(&stop, "/detail")))
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
        StrategySetTerminal::Exhausted => None,
    };

    let mut diagnostics = Vec::new();
    let mut contributions = Vec::new();
    let mut includes_execution_failure = false;
    for (attempt_index, attempt) in execution.attempts.into_iter().enumerate() {
        debug_assert_eq!(attempt.strategy_index, attempt_index);
        debug_assert!(!attempt.strategy_key.is_empty());
        diagnostics.extend(attempt.diagnostics);
        includes_execution_failure |=
            matches!(attempt.completion, StrategyAttemptCompletion::Failed);
        if Some(attempt_index) == accepted_attempt {
            let StrategyAttemptCompletion::Accepted(patch) = attempt.completion else {
                unreachable!("accepted terminal must reference accepted typed output");
            };
            contributions.push(DetailContribution {
                identity: posting.identity.clone(),
                patch,
                origin: ContributionOrigin {
                    strategy_key: attempt.strategy_key,
                    attempt_index,
                    provider_item_index: None,
                },
            });
        }
    }

    if accepted_attempt.is_none() {
        if context.is_cancelled() {
            diagnostics.push(runtime_execution_cancelled_diagnostic(
                &TypedCancellation::phase(RuntimePhase::Detail),
            ));
            return Err(PhaseRunError::Cancelled(PhaseCancelled {
                complete_budget_report: allowance.report(PhaseCompletion::Cancelled {
                    reason: PhaseCancellationReason::UserCancelled,
                }),
                diagnostics,
            }));
        }
        diagnostics.push(runtime_error(
            "fallback_exhausted",
            "detail fallback strategies were exhausted without an accepted result",
            "/detail/strategies",
            None,
            json!({}),
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
    let reduced = reduce_detail(&posting.identity, requested_fields, contributions);
    diagnostics.extend(reduced.diagnostics);
    let final_accepted =
        evaluate_detail_final_acceptance(&reduced.patch, phase_acceptance, &mut diagnostics)
            .is_satisfied();
    if context.is_cancelled() {
        diagnostics.push(runtime_execution_cancelled_diagnostic(
            &TypedCancellation::phase(RuntimePhase::Detail),
        ));
        return Err(PhaseRunError::Cancelled(PhaseCancelled {
            complete_budget_report: allowance.report(PhaseCompletion::Cancelled {
                reason: PhaseCancellationReason::UserCancelled,
            }),
            diagnostics,
        }));
    }
    let completion = if final_accepted {
        PhaseCompletion::Accepted
    } else {
        PhaseCompletion::PolicyUnsatisfied
    };
    let policy_outcome = if final_accepted {
        PolicyOutcome::Accepted {
            reduced_payload: DetailPhasePayload {
                patch: reduced.patch,
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

fn cancelled_detail_result(
    cancellation: TypedCancellation,
    report: PhaseExecutionReport,
) -> PhaseRunResult<DetailPhasePayload> {
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
        primitives::acceptance::{AcceptanceField, CompiledAcceptance},
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
        let execution = StrategySetExecution::<DetailPatch> {
            attempts: Vec::new(),
            terminal: StrategySetTerminal::Exhausted,
        };
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        let cancellation = Cancelled;
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);
        let posting = PostingOccurrence {
            identity:
                crate::profile_dsl::occurrence::PostingOccurrenceIdentity::ProviderPostingId {
                    source_key: "source".into(),
                    provider_posting_id: "id".into(),
                },
            reference: crate::profile_dsl::occurrence::PostingReference {
                provider_url: "https://example.com/jobs/1".into(),
                provider_posting_id: Some("id".into()),
            },
            provider_values: Default::default(),
            hints: Default::default(),
            posting_meta: Default::default(),
        };
        let requested_fields = RequestedDetailFields::new([DetailField::Title]).unwrap();

        let result = project_detail_execution(
            execution,
            &posting,
            &requested_fields,
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
    fn final_acceptance_is_evaluated_before_cancellation_discards_detail_payload() {
        let execution = StrategySetExecution {
            attempts: vec![StrategyAttempt {
                strategy_index: 0,
                strategy_key: "accepted".into(),
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(DetailPatch::default()),
            }],
            terminal: StrategySetTerminal::Accepted { attempt_index: 0 },
        };
        let acceptance = CompiledAcceptance {
            required_fields: vec![AcceptanceField::Title],
            min_description_length: None,
            min_results: None,
        };
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        let cancellation = Cancelled;
        let context = RuntimeExecutionContext::with_cancellation(&cancellation);
        let posting = PostingOccurrence {
            identity:
                crate::profile_dsl::occurrence::PostingOccurrenceIdentity::ProviderPostingId {
                    source_key: "source".into(),
                    provider_posting_id: "id".into(),
                },
            reference: crate::profile_dsl::occurrence::PostingReference {
                provider_url: "https://example.com/jobs/1".into(),
                provider_posting_id: Some("id".into()),
            },
            provider_values: Default::default(),
            hints: Default::default(),
            posting_meta: Default::default(),
        };
        let requested_fields = RequestedDetailFields::new([DetailField::Title]).unwrap();

        let result = project_detail_execution(
            execution,
            &posting,
            &requested_fields,
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
                "acceptance_required_field_missing",
                "runtime_execution_cancelled"
            ]
        );
    }
}
