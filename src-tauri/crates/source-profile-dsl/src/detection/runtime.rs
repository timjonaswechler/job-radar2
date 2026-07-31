use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use url::Url;

use super::plan::{
    CompiledDetectionJsonValue, CompiledDetectionPlan, CompiledDetectionStrategy, CompiledUrlInput,
};
use super::reconciliation::{
    aggregate_detection_attempts, DetectionAttempt, DetectionContribution,
    DetectionEvidenceContribution, DetectionOrigin, DetectionProfileContext,
    PreparedDetectionOutput, ReconciledDetectionRunResult, ReconciledDetectionState,
};
use crate::definition::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::definition::documents::{DetectionEvidenceKind, PhaseLimits};
use crate::definition::policy::StrategyPolicy;
use crate::definition::primitives::capture::evaluate_named_pattern;
use crate::definition::primitives::fetch::http_execution::{
    execute_http_fetch, HttpFetchExecutionError, HttpFetchOverlay, HttpStatusPolicy,
};
use crate::definition::primitives::predicate::{literal_contains, values_equal};
use crate::definition::template::{
    render_template, CompiledTemplate, TemplateReference, TemplateValueView,
};
use crate::execution::allowance::{completion_for_stop, AllowanceStop, InvocationAllowance};
use crate::execution::browser_phase::{
    execute_canonical_browser_fetch, BrowserPhaseFetchInput, BrowserPhaseFetchProjection,
};
use crate::execution::cancellation::{CancellationOperation, RuntimePhase, TypedCancellation};
use crate::execution::strategy_set::{
    execute_strategy_set, policy_unsatisfied_diagnostic, StrategyAttemptCompletion,
    StrategyExecution, StrategySetTerminal,
};
use crate::execution::{
    BrowserAcquisition, BrowserAcquisitionFailureKind, PhaseBrowser, PhaseCompletion,
    PhaseExecutionReport, ProfileHttpClient, ProfileHttpFailureKind, RuntimeCancellation,
    RuntimeExecutionContext,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionProfileRejectionKind {
    Url,
    Status,
    Contains,
    Regex,
    Capture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "kind", rename_all = "snake_case")]
pub enum DetectionProfileExecutionFailureKind {
    Acquisition(ProfileHttpFailureKind),
    BrowserAcquisition(DetectionBrowserFailureKind),
    BrowserInfrastructure,
    Render,
    Reconciliation,
    Proposal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionBrowserFailureKind {
    RuntimeLaunch,
    Navigation,
    Wait,
    Interaction,
    ContentRead,
    Deadline,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectionProfileCompletion {
    Matched,
    Unsupported,
    Rejected {
        strategy_key: String,
        kind: DetectionProfileRejectionKind,
    },
    ExecutionFailed {
        strategy_key: Option<String>,
        kind: DetectionProfileExecutionFailureKind,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionProfileOutcome {
    pub profile_key: String,
    pub completion: DetectionProfileCompletion,
    pub diagnostics: Diagnostics,
}

#[cfg(not(feature = "test-support"))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionOperationResult {
    attempts: Vec<DetectionAttempt>,
    profile_outcomes: Vec<DetectionProfileOutcome>,
    pub run_result: ReconciledDetectionRunResult,
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}
#[cfg(feature = "test-support")]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionOperationResult {
    pub attempts: Vec<DetectionAttempt>,
    pub profile_outcomes: Vec<DetectionProfileOutcome>,
    pub run_result: ReconciledDetectionRunResult,
    pub diagnostics: Diagnostics,
    pub report: PhaseExecutionReport,
}

fn render_detection_json_value(
    value: &CompiledDetectionJsonValue,
    values: &dyn TemplateValueView,
) -> Result<serde_json::Value, ()> {
    match value {
        CompiledDetectionJsonValue::Template(template) => render_template(template, values)
            .map(serde_json::Value::String)
            .map_err(|_| ()),
        CompiledDetectionJsonValue::Array(items) => items
            .iter()
            .map(|item| render_detection_json_value(item, values))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        CompiledDetectionJsonValue::Object(items) => items
            .iter()
            .map(|(key, value)| {
                render_detection_json_value(value, values).map(|value| (key.clone(), value))
            })
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
        CompiledDetectionJsonValue::Literal(value) => Ok(value.clone()),
    }
}

pub async fn execute_detection_operation<C>(
    input_url: &str,
    plans: &[CompiledDetectionPlan],
    client: &C,
    browser: PhaseBrowser<&dyn BrowserAcquisition>,
    cancellation: &dyn RuntimeCancellation,
) -> DetectionOperationResult
where
    C: ProfileHttpClient + Sync + ?Sized,
{
    let canonical_url = match canonical_url(input_url) {
        Ok(url) => url,
        Err(diagnostic) => {
            let diagnostics = vec![diagnostic];
            return DetectionOperationResult {
                attempts: Vec::new(),
                profile_outcomes: Vec::new(),
                run_result: aggregate_detection_attempts(vec![DetectionAttempt::Failed(
                    diagnostics.clone(),
                )]),
                diagnostics,
                report: InvocationAllowance::prestart_failure_report(),
            };
        }
    };
    let allowance = InvocationAllowance::new_inactive_with_logical_wait_limit(
        detection_operation_limits(),
        false,
        None,
        Some(32),
    );
    let base_context = RuntimeExecutionContext::with_cancellation(cancellation);
    let execution_context = base_context.for_detection_http(&allowance);
    let mut attempts = Vec::new();
    let mut profile_outcomes = Vec::new();
    let mut diagnostics = Vec::new();

    for plan in plans {
        if cancellation.is_cancelled() {
            return terminal_result(
                &allowance,
                PhaseCompletion::Cancelled {
                    reason: crate::execution::PhaseCancellationReason::UserCancelled,
                },
                Vec::new(),
                Vec::new(),
                diagnostics,
            );
        }
        let profile_scope = match allowance.inactive_child_scope(
            InvocationAllowance::ROOT_SCOPE,
            detection_profile_limits(),
            Some(8),
        ) {
            Ok(scope) => scope,
            Err(stop) => {
                return terminal_result(
                    &allowance,
                    completion_for_stop(stop),
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                )
            }
        };
        let state = Mutex::new(plan.context.initial_state());
        let profile_completion = Mutex::new(None);
        let allowance_ref = &allowance;
        let browser_ref = &browser;
        let execution = execute_strategy_set(
            StrategyPolicy::AllRequired,
            &plan.strategies,
            CompiledDetectionStrategy::key,
            |index, strategy| {
                cancellation.is_cancelled().then(|| {
                    TypedCancellation::strategy(
                        RuntimePhase::Detection,
                        index,
                        strategy.key(),
                        CancellationOperation::Phase,
                    )
                })
            },
            |index, strategy| {
                let state = &state;
                let canonical_url = &canonical_url;
                let context = &plan.context;
                let profile_completion = &profile_completion;
                Box::pin(async move {
                    execute_strategy(
                        index,
                        strategy,
                        canonical_url,
                        context,
                        state,
                        client,
                        browser_ref,
                        allowance_ref,
                        profile_scope,
                        execution_context,
                        profile_completion,
                    )
                    .await
                })
            },
        )
        .await;
        let mut attempt_diagnostics = execution
            .attempts
            .iter()
            .flat_map(|attempt| attempt.diagnostics.clone())
            .collect::<Vec<_>>();
        match execution.terminal {
            StrategySetTerminal::Satisfied => {
                if cancellation.is_cancelled() {
                    diagnostics.extend(attempt_diagnostics);
                    return terminal_result(
                        &allowance,
                        PhaseCompletion::Cancelled {
                            reason: crate::execution::PhaseCancellationReason::UserCancelled,
                        },
                        Vec::new(),
                        Vec::new(),
                        diagnostics,
                    );
                }
                let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
                let values = DetectionTemplateValues::from_state(&canonical_url, &snapshot);
                let rendered_source_config = plan
                    .proposal_source_config
                    .as_ref()
                    .map(|source_config| {
                        source_config
                            .iter()
                            .map(|(key, value)| {
                                render_detection_json_value(value, &values)
                                    .map(|value| (key.clone(), value))
                            })
                            .collect::<Result<serde_json::Map<_, _>, _>>()
                    })
                    .transpose();
                let render_candidates = |templates: &[CompiledTemplate]| {
                    templates
                        .iter()
                        .map(|template| render_template(template, &values).map_err(|_| ()))
                        .collect::<Result<Vec<_>, _>>()
                };
                let prepared = rendered_source_config
                    .and_then(|source_config| {
                        render_candidates(&plan.key_candidates).map(|keys| (source_config, keys))
                    })
                    .and_then(|(source_config, keys)| {
                        render_candidates(&plan.name_candidates)
                            .map(|names| (source_config, keys, names))
                    });
                let Ok((source_config, key_candidates, name_candidates)) = prepared else {
                    let profile_diagnostics = vec![runtime_error(
                        "detection_proposal_template_failed",
                        "Detection proposal Template dependency was unavailable",
                        "/detection",
                        "proposal",
                    )];
                    attempts.push(DetectionAttempt::Failed(profile_diagnostics.clone()));
                    profile_outcomes.push(DetectionProfileOutcome {
                        profile_key: plan.profile_key.clone(),
                        completion: DetectionProfileCompletion::ExecutionFailed {
                            strategy_key: None,
                            kind: DetectionProfileExecutionFailureKind::Proposal,
                        },
                        diagnostics: profile_diagnostics,
                    });
                    continue;
                };
                let prepared_proposal = plan.context.prepare_proposal_with_canonical_url(
                    &snapshot,
                    &canonical_url,
                    source_config,
                    key_candidates,
                    name_candidates,
                );
                if cancellation.is_cancelled() {
                    diagnostics.extend(attempt_diagnostics);
                    return terminal_result(
                        &allowance,
                        PhaseCompletion::Cancelled {
                            reason: crate::execution::PhaseCancellationReason::UserCancelled,
                        },
                        Vec::new(),
                        Vec::new(),
                        diagnostics,
                    );
                }
                match prepared_proposal {
                    Ok(PreparedDetectionOutput::Proposal(proposal)) => {
                        attempts.push(DetectionAttempt::Matched(proposal));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::Matched,
                            diagnostics: attempt_diagnostics,
                        });
                    }
                    Ok(PreparedDetectionOutput::Unsupported(profile)) => {
                        attempts.push(DetectionAttempt::Unsupported(profile));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::Unsupported,
                            diagnostics: attempt_diagnostics,
                        });
                    }
                    Err(error) => {
                        let profile_diagnostics = error.diagnostics();
                        attempts.push(DetectionAttempt::Failed(profile_diagnostics.clone()));
                        profile_outcomes.push(DetectionProfileOutcome {
                            profile_key: plan.profile_key.clone(),
                            completion: DetectionProfileCompletion::ExecutionFailed {
                                strategy_key: None,
                                kind: DetectionProfileExecutionFailureKind::Proposal,
                            },
                            diagnostics: profile_diagnostics,
                        });
                    }
                }
            }
            StrategySetTerminal::PolicyUnsatisfied => {
                attempt_diagnostics.push(policy_unsatisfied_diagnostic(
                    StrategyPolicy::AllRequired,
                    RuntimePhase::Detection,
                ));
                attempts.push(DetectionAttempt::Failed(attempt_diagnostics.clone()));
                profile_outcomes.push(DetectionProfileOutcome {
                    profile_key: plan.profile_key.clone(),
                    completion: profile_completion
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .take()
                        .unwrap_or(DetectionProfileCompletion::ExecutionFailed {
                            strategy_key: None,
                            kind: DetectionProfileExecutionFailureKind::Reconciliation,
                        }),
                    diagnostics: attempt_diagnostics,
                });
            }
            StrategySetTerminal::Cancelled(_) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    PhaseCompletion::Cancelled {
                        reason: crate::execution::PhaseCancellationReason::UserCancelled,
                    },
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
            StrategySetTerminal::Stopped(AllowanceStop::Exhausted(_)) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    completion_for_stop(allowance.stop().unwrap_or(AllowanceStop::Internal)),
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
            StrategySetTerminal::Stopped(AllowanceStop::Internal) => {
                diagnostics.extend(attempt_diagnostics);
                return terminal_result(
                    &allowance,
                    PhaseCompletion::ExecutionFailed,
                    Vec::new(),
                    Vec::new(),
                    diagnostics,
                );
            }
        }
    }
    let completion = allowance
        .stop()
        .map(completion_for_stop)
        .unwrap_or(PhaseCompletion::Accepted);
    terminal_result(
        &allowance,
        completion,
        attempts,
        profile_outcomes,
        diagnostics,
    )
}

async fn execute_strategy<C>(
    index: usize,
    strategy: &CompiledDetectionStrategy,
    input_url: &str,
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    client: &C,
    browser: &PhaseBrowser<&dyn BrowserAcquisition>,
    allowance: &InvocationAllowance,
    profile_scope: usize,
    execution_context: RuntimeExecutionContext<'_>,
    profile_completion: &Mutex<Option<DetectionProfileCompletion>>,
) -> StrategyExecution<()>
where
    C: ProfileHttpClient + Sync + ?Sized,
{
    let base = format!("/detection/strategies/{index}");
    match strategy {
        CompiledDetectionStrategy::Url {
            key: _,
            input: CompiledUrlInput::AbsoluteUrl,
        } => StrategyExecution {
            diagnostics: Vec::new(),
            completion: StrategyAttemptCompletion::Accepted(()),
        },
        CompiledDetectionStrategy::Url {
            key,
            input: CompiledUrlInput::PatternAlternatives(alternatives),
        } => {
            for (alternative_index, alternative) in alternatives.iter().enumerate() {
                match evaluate_named_pattern(&alternative.pattern, input_url) {
                    Ok(Some(captures)) => {
                        if let Err(diagnostics) = apply_captures(
                            profile,
                            state,
                            key,
                            &format!("{base}/input/alternatives/{alternative_index}"),
                            captures,
                        ) {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                        let evidence = DetectionEvidenceContribution::new(
                            DetectionEvidenceKind::Url,
                            format!("{base}/input/alternatives/{alternative_index}/pattern"),
                            "Detection input URL matched an authored alternative",
                        )
                        .expect("compiled descriptor path is valid");
                        if let Err(diagnostics) = apply_one(
                            profile,
                            state,
                            DetectionContribution::new(
                                DetectionOrigin::new(
                                    key,
                                    format!(
                                        "{base}/input/alternatives/{alternative_index}/pattern"
                                    ),
                                )
                                .unwrap(),
                            )
                            .with_evidence(evidence),
                        ) {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                        return StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Accepted(()),
                        };
                    }
                    Ok(None) => continue,
                    Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection named capture did not resolve",
                        );
                    }
                }
            }
            set_rejection(profile_completion, key, DetectionProfileRejectionKind::Url);
            rejected(
                key,
                &base,
                "detection_url_not_matched",
                "Detection URL alternatives did not match",
            )
        }
        CompiledDetectionStrategy::Http {
            key,
            fetch,
            expect_status,
            contains,
            acceptance_regex,
            captures,
            evidence,
        } => {
            let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let values = DetectionTemplateValues::from_state(input_url, &snapshot);
            let response = match execute_http_fetch(
                client,
                fetch,
                &values,
                HttpFetchOverlay::default(),
                None,
                HttpStatusPolicy::PreserveResponse,
                execution_context,
            )
            .await
            {
                Ok(response) => response,
                Err(HttpFetchExecutionError::Cancelled) => return cancelled(index, key),
                Err(HttpFetchExecutionError::Acquisition(error))
                    if error.kind == ProfileHttpFailureKind::Cancelled =>
                {
                    return cancelled(index, key)
                }
                Err(HttpFetchExecutionError::Acquisition(error))
                    if error.kind == ProfileHttpFailureKind::ResponseBytesExceeded =>
                {
                    let stop = execution_context.stop().unwrap_or(AllowanceStop::Internal);
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(stop),
                    };
                }
                Err(HttpFetchExecutionError::BudgetExhausted) => {
                    let stop = execution_context.stop().unwrap_or(AllowanceStop::Internal);
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(stop),
                    };
                }
                Err(HttpFetchExecutionError::Render(error)) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Render,
                    );
                    return failed(vec![runtime_error(
                        error.code,
                        &error.message,
                        &format!("{base}/fetch{}", error.path),
                        key,
                    )]);
                }
                Err(HttpFetchExecutionError::Acquisition(error)) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Acquisition(error.kind),
                    );
                    return failed(vec![runtime_error(
                        "detection_http_acquisition_failed",
                        "Detection HTTP acquisition failed",
                        &format!("{base}/fetch"),
                        key,
                    )]);
                }
                Err(HttpFetchExecutionError::NonSuccessStatus { .. }) => {
                    unreachable!("PreserveResponse never projects status failure")
                }
            };
            if expect_status.is_some_and(|expected| !values_equal(&response.status(), &expected)) {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Status,
                );
                return rejected(
                    key,
                    &base,
                    "detection_status_rejected",
                    "Detection HTTP status did not match expectStatus",
                );
            }
            if contains
                .as_ref()
                .is_some_and(|expected| !literal_contains(&response.body, expected))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Contains,
                );
                return rejected(
                    key,
                    &base,
                    "detection_contains_rejected",
                    "Detection HTTP body did not contain the required literal",
                );
            }
            if acceptance_regex
                .as_ref()
                .is_some_and(|regex| !regex.is_match(&response.body))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Regex,
                );
                return rejected(
                    key,
                    &base,
                    "detection_regex_rejected",
                    "Detection HTTP body did not match the required regex",
                );
            }
            if let Some(pattern) = captures {
                match evaluate_named_pattern(pattern, &response.body) {
                    Ok(Some(outputs)) => {
                        if let Err(diagnostics) =
                            apply_captures(profile, state, key, &format!("{base}/regex"), outputs)
                        {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                    }
                    Ok(None) | Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection HTTP named captures did not resolve",
                        );
                    }
                }
            }
            let (evidence_path, evidence_message) = evidence.as_ref().map_or_else(
                || (format!("{base}/fetch"), "Detection HTTP Strategy accepted"),
                |message| (format!("{base}/evidence"), message.as_str()),
            );
            let contribution =
                DetectionContribution::new(DetectionOrigin::new(key, &evidence_path).unwrap())
                    .with_evidence(
                        DetectionEvidenceContribution::new(
                            DetectionEvidenceKind::Http,
                            &evidence_path,
                            evidence_message,
                        )
                        .unwrap(),
                    );
            if let Err(diagnostics) = apply_one(profile, state, contribution) {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::Reconciliation,
                );
                return failed(diagnostics);
            }
            StrategyExecution {
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(()),
            }
        }
        CompiledDetectionStrategy::Browser {
            key,
            url,
            timeout_ms,
            waits,
            interactions,
            contains,
            acceptance_regex,
            captures,
            evidence,
        } => {
            let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
            let values = DetectionTemplateValues::from_state(input_url, &snapshot);
            let target = match render_template(url, &values) {
                Ok(target) => target,
                Err(_) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::Render,
                    );
                    return failed(vec![runtime_error(
                        "detection_browser_template_failed",
                        "Detection Browser target Template dependency was unavailable",
                        &format!("{base}/fetch/url"),
                        key,
                    )]);
                }
            };
            let PhaseBrowser::Browser(acquisition) = browser else {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::BrowserAcquisition(
                        DetectionBrowserFailureKind::RuntimeLaunch,
                    ),
                );
                return failed(vec![runtime_error(
                    "browser_runtime_unavailable",
                    "Detection Browser acquisition is unavailable",
                    &format!("{base}/fetch"),
                    key,
                )]);
            };
            if let Err(stop) = allowance.activate_scope_chain(profile_scope) {
                return StrategyExecution {
                    diagnostics: Vec::new(),
                    completion: StrategyAttemptCompletion::Stopped(stop),
                };
            }
            let strategy_scope =
                match allowance.child_scope(profile_scope, detection_strategy_limits(), Some(4)) {
                    Ok(scope) => scope,
                    Err(stop) => {
                        return StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Stopped(stop),
                        }
                    }
                };
            let control = execution_context.for_allowance_scope(allowance, strategy_scope);
            let rendered = match execute_canonical_browser_fetch(
                *acquisition,
                RuntimePhase::Detection,
                BrowserPhaseFetchInput {
                    target,
                    timeout_ms: *timeout_ms,
                    waits: waits.clone(),
                    interactions: interactions.clone(),
                    base_path: base.clone(),
                    strategy_key: key.clone(),
                    strategy_index: index,
                    control,
                },
            )
            .await
            {
                BrowserPhaseFetchProjection::Rendered(rendered) => rendered,
                BrowserPhaseFetchProjection::AttemptFailed { diagnostic, kind } => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::BrowserAcquisition(
                            browser_failure_kind(&kind),
                        ),
                    );
                    return failed(vec![diagnostic]);
                }
                BrowserPhaseFetchProjection::PhaseFatal(diagnostic) => {
                    set_execution_failure(
                        profile_completion,
                        key,
                        DetectionProfileExecutionFailureKind::BrowserInfrastructure,
                    );
                    return failed(vec![diagnostic]);
                }
                BrowserPhaseFetchProjection::AllowanceStopped => {
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Stopped(
                            control.stop().unwrap_or(AllowanceStop::Internal),
                        ),
                    }
                }
                BrowserPhaseFetchProjection::Cancelled(cancellation) => {
                    return StrategyExecution {
                        diagnostics: Vec::new(),
                        completion: StrategyAttemptCompletion::Cancelled(cancellation),
                    }
                }
            };
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            if contains
                .as_ref()
                .is_some_and(|expected| !literal_contains(&rendered, expected))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Contains,
                );
                return rejected(
                    key,
                    &base,
                    "detection_contains_rejected",
                    "Detection Browser content did not contain the required literal",
                );
            }
            if acceptance_regex
                .as_ref()
                .is_some_and(|regex| !regex.is_match(&rendered))
            {
                set_rejection(
                    profile_completion,
                    key,
                    DetectionProfileRejectionKind::Regex,
                );
                return rejected(
                    key,
                    &base,
                    "detection_regex_rejected",
                    "Detection Browser content did not match the required regex",
                );
            }
            if let Some(pattern) = captures {
                match evaluate_named_pattern(pattern, &rendered) {
                    Ok(Some(outputs)) => {
                        if control.is_cancelled() {
                            return cancelled(index, key);
                        }
                        if let Err(diagnostics) =
                            apply_captures(profile, state, key, &format!("{base}/regex"), outputs)
                        {
                            set_execution_failure(
                                profile_completion,
                                key,
                                DetectionProfileExecutionFailureKind::Reconciliation,
                            );
                            return failed(diagnostics);
                        }
                    }
                    Ok(None) | Err(_) => {
                        set_rejection(
                            profile_completion,
                            key,
                            DetectionProfileRejectionKind::Capture,
                        );
                        return rejected(
                            key,
                            &base,
                            "detection_capture_rejected",
                            "Detection Browser named captures did not resolve",
                        );
                    }
                }
            }
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            let (evidence_path, evidence_message) = evidence.as_ref().map_or_else(
                || {
                    (
                        format!("{base}/fetch"),
                        "Detection Browser Strategy accepted",
                    )
                },
                |message| (format!("{base}/evidence"), message.as_str()),
            );
            let contribution = DetectionContribution::new(
                DetectionOrigin::new(key, &evidence_path).expect("compiled origin"),
            )
            .with_evidence(
                DetectionEvidenceContribution::new(
                    DetectionEvidenceKind::Browser,
                    &evidence_path,
                    evidence_message,
                )
                .expect("compiled evidence"),
            );
            if let Err(diagnostics) = apply_one(profile, state, contribution) {
                set_execution_failure(
                    profile_completion,
                    key,
                    DetectionProfileExecutionFailureKind::Reconciliation,
                );
                return failed(diagnostics);
            }
            if control.is_cancelled() {
                return cancelled(index, key);
            }
            StrategyExecution {
                diagnostics: Vec::new(),
                completion: StrategyAttemptCompletion::Accepted(()),
            }
        }
    }
}

fn apply_captures(
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    key: &str,
    path: &str,
    captures: Vec<crate::definition::primitives::capture::CaptureOutput>,
) -> Result<(), Diagnostics> {
    for capture in captures {
        apply_one(
            profile,
            state,
            DetectionContribution::new(DetectionOrigin::new(key, path).unwrap())
                .with_capture(capture.key, capture.value),
        )?;
    }
    Ok(())
}

fn apply_one(
    profile: &DetectionProfileContext,
    state: &Mutex<ReconciledDetectionState>,
    contribution: DetectionContribution,
) -> Result<(), Diagnostics> {
    let snapshot = state.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let next = profile
        .apply(&snapshot, contribution)
        .map_err(|error| error.diagnostics())?;
    *state.lock().unwrap_or_else(|p| p.into_inner()) = next;
    Ok(())
}

struct DetectionTemplateValues<'a> {
    input_url: &'a str,
    captures: HashMap<String, String>,
}
impl<'a> DetectionTemplateValues<'a> {
    fn from_state(input_url: &'a str, state: &ReconciledDetectionState) -> Self {
        Self {
            input_url,
            captures: state
                .captures()
                .iter()
                .map(|capture| (capture.key().to_string(), capture.value().to_string()))
                .collect(),
        }
    }
}
impl TemplateValueView for DetectionTemplateValues<'_> {
    fn resolve(&self, reference: &TemplateReference) -> Option<String> {
        match reference.namespace.as_deref() {
            None if reference.key == "inputUrl" => Some(self.input_url.to_string()),
            Some("capture") => self.captures.get(&reference.key).cloned(),
            _ => None,
        }
    }
}

fn detection_operation_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 8,
        max_duration_ms: 60_000,
        max_pages: 32,
        max_browser_actions: 32,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 16_777_216,
        ..PhaseLimits::BACKEND
    }
}

fn detection_profile_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 2,
        max_duration_ms: 30_000,
        max_pages: 8,
        max_browser_actions: 10,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 4_194_304,
        ..PhaseLimits::BACKEND
    }
}

fn detection_strategy_limits() -> PhaseLimits {
    PhaseLimits {
        max_requests: 1,
        max_duration_ms: 20_000,
        max_pages: 4,
        max_browser_actions: 32,
        max_response_bytes: 67_108_864,
        max_browser_rendered_bytes: 2_097_152,
        ..PhaseLimits::BACKEND
    }
}

fn browser_failure_kind(kind: &BrowserAcquisitionFailureKind) -> DetectionBrowserFailureKind {
    match kind {
        BrowserAcquisitionFailureKind::RuntimeLaunch => DetectionBrowserFailureKind::RuntimeLaunch,
        BrowserAcquisitionFailureKind::Navigation => DetectionBrowserFailureKind::Navigation,
        BrowserAcquisitionFailureKind::Wait { .. } => DetectionBrowserFailureKind::Wait,
        BrowserAcquisitionFailureKind::Interaction { .. } => {
            DetectionBrowserFailureKind::Interaction
        }
        BrowserAcquisitionFailureKind::ContentRead => DetectionBrowserFailureKind::ContentRead,
        BrowserAcquisitionFailureKind::Deadline => DetectionBrowserFailureKind::Deadline,
    }
}

fn canonical_url(input: &str) -> Result<String, Diagnostic> {
    let url = Url::parse(input.trim()).map_err(|_| {
        runtime_error(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL",
            "/inputUrl",
            "input",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.has_host()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(runtime_error(
            "invalid_detection_input_url",
            "Detection input URL must be an absolute HTTP(S) URL without userinfo",
            "/inputUrl",
            "input",
        ));
    }
    Ok(url.to_string())
}

fn runtime_error(code: &str, message: &str, path: &str, key: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Detection,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.to_string(),
        strategy_key: Some(key.to_string()),
        details: None,
    }
}
fn set_rejection(
    completion: &Mutex<Option<DetectionProfileCompletion>>,
    strategy_key: &str,
    kind: DetectionProfileRejectionKind,
) {
    *completion.lock().unwrap_or_else(|p| p.into_inner()) =
        Some(DetectionProfileCompletion::Rejected {
            strategy_key: strategy_key.to_string(),
            kind,
        });
}

fn set_execution_failure(
    completion: &Mutex<Option<DetectionProfileCompletion>>,
    strategy_key: &str,
    kind: DetectionProfileExecutionFailureKind,
) {
    *completion.lock().unwrap_or_else(|p| p.into_inner()) =
        Some(DetectionProfileCompletion::ExecutionFailed {
            strategy_key: Some(strategy_key.to_string()),
            kind,
        });
}

fn rejected(key: &str, path: &str, code: &str, message: &str) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics: vec![runtime_error(code, message, path, key)],
        completion: StrategyAttemptCompletion::Rejected,
    }
}
fn failed(diagnostics: Diagnostics) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics,
        completion: StrategyAttemptCompletion::Failed,
    }
}
fn cancelled(index: usize, key: &str) -> StrategyExecution<()> {
    StrategyExecution {
        diagnostics: Vec::new(),
        completion: StrategyAttemptCompletion::Cancelled(TypedCancellation::strategy(
            RuntimePhase::Detection,
            index,
            key,
            CancellationOperation::Fetch,
        )),
    }
}
fn terminal_result(
    allowance: &InvocationAllowance,
    completion: PhaseCompletion,
    attempts: Vec<DetectionAttempt>,
    profile_outcomes: Vec<DetectionProfileOutcome>,
    diagnostics: Diagnostics,
) -> DetectionOperationResult {
    let aggregate_attempts = match &completion {
        PhaseCompletion::BudgetExhausted { .. } => {
            vec![DetectionAttempt::BudgetExhausted(diagnostics.clone())]
        }
        PhaseCompletion::Cancelled { .. } => {
            vec![DetectionAttempt::Cancelled(diagnostics.clone())]
        }
        PhaseCompletion::ExecutionFailed if attempts.is_empty() => {
            vec![DetectionAttempt::Failed(diagnostics.clone())]
        }
        PhaseCompletion::Accepted
        | PhaseCompletion::PolicyUnsatisfied
        | PhaseCompletion::ExecutionFailed => attempts.clone(),
    };
    let report = allowance.report(completion);
    DetectionOperationResult {
        attempts,
        profile_outcomes,
        run_result: aggregate_detection_attempts(aggregate_attempts),
        diagnostics,
        report,
    }
}
