use std::collections::BTreeMap;

use dom_query::NodeRef;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    profile_dsl::{
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        execution_plan::{
            capabilities::ExecutionPlanFetch, detail::ExecutionPlanDetailStrategy,
            SourceExecutionPlan,
        },
        occurrence::{
            ContributionOrigin, DetailContributionEvidence, DetailField, DetailPatch,
            DetailRejection, PostingOccurrence, RequestedDetailFields,
        },
        primitives::{
            acceptance::{
                evaluate_detail_acceptance, validate_detail_acceptance_request, CompiledAcceptance,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailExecutionResult {
    pub patch: DetailPatch,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<DetailContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<DetailContributionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rejections: Vec<DetailRejection>,
    pub diagnostics: Diagnostics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<PhaseExecutionReport>,
}

pub async fn execute_detail<F, B>(
    plan: &SourceExecutionPlan,
    source_config: &SourceConfig,
    posting: &PostingOccurrence,
    requested_fields: RequestedDetailFields,
    fetcher: &F,
    browser: &B,
    context: RuntimeExecutionContext<'_>,
) -> DetailExecutionResult
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let Some(detail) = &plan.detail else {
        return DetailExecutionResult {
            patch: DetailPatch::default(),
            provenance: Vec::new(),
            conflicts: Vec::new(),
            rejections: Vec::new(),
            diagnostics: vec![runtime_error(
                "detail_missing",
                "Execution Plan does not contain compiled detail",
                "/detail",
                None,
                json!({}),
            )],
            report: None,
        };
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
        return DetailExecutionResult {
            patch: DetailPatch::default(),
            provenance: Vec::new(),
            conflicts: Vec::new(),
            rejections: Vec::new(),
            diagnostics: vec![diagnostic],
            report: Some(InvocationAllowance::prestart_failure_report()),
        };
    }

    if detail.strategies.is_empty() {
        return DetailExecutionResult {
            patch: DetailPatch::default(),
            provenance: Vec::new(),
            conflicts: Vec::new(),
            rejections: Vec::new(),
            diagnostics: vec![runtime_error(
                "detail_strategy_missing",
                "detail does not contain an executable strategy",
                "/detail/strategies",
                None,
                json!({}),
            )],
            report: None,
        };
    }
    let browser_requires_reserve = detail
        .strategies
        .iter()
        .any(|strategy| uses_browser(&strategy.fetch));
    if browser_requires_reserve && detail.limits.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS {
        return DetailExecutionResult {
            patch: DetailPatch::default(),
            provenance: Vec::new(),
            conflicts: Vec::new(),
            rejections: Vec::new(),
            diagnostics: vec![runtime_error(
                "invalid_compiled_browser_phase_duration",
                "Compiled Detail Browser duration does not preserve the teardown reserve",
                "/detail/limits/maxDurationMs",
                None,
                json!({}),
            )],
            report: None,
        };
    }
    if let Some(caller) = context.caller_limits() {
        if !caller.all_positive()
            || !caller.within(detail.limits)
            || (browser_requires_reserve && caller.max_duration_ms < BROWSER_TEARDOWN_RESERVE_MS)
        {
            return DetailExecutionResult {
                patch: DetailPatch::default(),
            provenance: Vec::new(),
            conflicts: Vec::new(),
            rejections: Vec::new(),
                diagnostics: vec![runtime_error(
                    "invalid_caller_phase_limits",
                    "Caller phase limits must be positive, may only tighten compiled limits, and must preserve the Browser teardown reserve",
                    "/detail/limits",
                    None,
                    json!({}),
                )],
                report: Some(InvocationAllowance::prestart_failure_report()),
            };
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
        &allowance,
    )
}

fn project_detail_execution(
    execution: super::strategy_set::StrategySetExecution<DetailPatch>,
    posting: &PostingOccurrence,
    requested_fields: &RequestedDetailFields,
    phase_acceptance: Option<&CompiledAcceptance>,
    allowance: &InvocationAllowance,
) -> DetailExecutionResult {
    let accepted_attempt = match execution.terminal {
        StrategySetTerminal::Accepted { attempt_index } => Some(attempt_index),
        StrategySetTerminal::Cancelled(cancellation) => {
            let mut diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .collect::<Diagnostics>();
            diagnostics.push(runtime_execution_cancelled_diagnostic(&cancellation));
            return DetailExecutionResult {
                patch: DetailPatch::default(),
                provenance: Vec::new(),
                conflicts: Vec::new(),
                rejections: Vec::new(),
                diagnostics,
                report: Some(allowance.report(PhaseCompletion::Cancelled {
                    reason: PhaseCancellationReason::UserCancelled,
                })),
            };
        }
        StrategySetTerminal::Stopped(stop) => {
            let diagnostics = execution
                .attempts
                .into_iter()
                .flat_map(|attempt| attempt.diagnostics)
                .chain(std::iter::once(diagnostic_for_stop(&stop, "/detail")))
                .collect();
            let completion = completion_for_stop(stop);
            return DetailExecutionResult {
                patch: DetailPatch::default(),
                provenance: Vec::new(),
                conflicts: Vec::new(),
                rejections: Vec::new(),
                diagnostics,
                report: Some(allowance.report(completion)),
            };
        }
        StrategySetTerminal::Exhausted => None,
    };

    let mut diagnostics = Vec::new();
    let mut contributions = Vec::new();
    for (attempt_index, attempt) in execution.attempts.into_iter().enumerate() {
        debug_assert_eq!(attempt.strategy_index, attempt_index);
        debug_assert!(!attempt.strategy_key.is_empty());
        diagnostics.extend(attempt.diagnostics);
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
        diagnostics.push(runtime_error(
            "fallback_exhausted",
            "detail fallback strategies were exhausted without an accepted result",
            "/detail/strategies",
            None,
            json!({}),
        ));
    }
    let reduced = reduce_detail(&posting.identity, requested_fields, contributions);
    diagnostics.extend(reduced.diagnostics);
    let final_accepted = accepted_attempt.is_some()
        && evaluate_detail_acceptance(
            &reduced.patch,
            phase_acceptance,
            None,
            "/detail",
            None,
            &mut diagnostics,
        );
    let completion = if final_accepted {
        PhaseCompletion::Accepted
    } else {
        PhaseCompletion::PolicyUnsatisfied
    };
    DetailExecutionResult {
        patch: if final_accepted {
            reduced.patch
        } else {
            DetailPatch::default()
        },
        provenance: if final_accepted {
            reduced.provenance
        } else {
            Vec::new()
        },
        conflicts: if final_accepted {
            reduced.conflicts
        } else {
            Vec::new()
        },
        rejections: if final_accepted {
            reduced.rejections
        } else {
            Vec::new()
        },
        diagnostics,
        report: Some(allowance.report(completion)),
    }
}

fn cancelled_detail_result(
    cancellation: TypedCancellation,
    report: PhaseExecutionReport,
) -> DetailExecutionResult {
    DetailExecutionResult {
        patch: DetailPatch::default(),
        provenance: Vec::new(),
        conflicts: Vec::new(),
        rejections: Vec::new(),
        diagnostics: vec![runtime_execution_cancelled_diagnostic(&cancellation)],
        report: Some(report),
    }
}
