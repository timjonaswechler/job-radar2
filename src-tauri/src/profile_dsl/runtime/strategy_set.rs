use std::{future::Future, pin::Pin};

use serde_json::json;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
    policy::StrategyPolicy,
};

use super::{
    allowance::AllowanceStop,
    cancellation::{RuntimePhase, TypedCancellation},
};

pub(crate) enum StrategyAttemptCompletion<O> {
    Accepted(O),
    Rejected,
    Failed,
    Cancelled(TypedCancellation),
    Stopped(AllowanceStop),
}

pub(crate) struct StrategyExecution<O> {
    pub(crate) diagnostics: Diagnostics,
    pub(crate) completion: StrategyAttemptCompletion<O>,
}

pub(crate) struct StrategyAttempt<O> {
    pub(crate) strategy_index: usize,
    pub(crate) strategy_key: String,
    pub(crate) diagnostics: Diagnostics,
    pub(crate) completion: StrategyAttemptCompletion<O>,
}

pub(crate) enum StrategySetTerminal {
    Satisfied,
    PolicyUnsatisfied,
    Cancelled(TypedCancellation),
    Stopped(AllowanceStop),
}

pub(crate) struct StrategySetExecution<O> {
    pub(crate) attempts: Vec<StrategyAttempt<O>>,
    pub(crate) terminal: StrategySetTerminal,
}

pub(crate) fn policy_unsatisfied_diagnostic(
    policy: StrategyPolicy,
    phase: RuntimePhase,
) -> Diagnostic {
    let phase_name = match phase {
        RuntimePhase::Discovery => "discovery",
        RuntimePhase::Detail => "detail",
    };
    let (code, message, path, details) = match policy {
        StrategyPolicy::FirstAccepted => (
            "fallback_exhausted",
            format!("{phase_name} fallback strategies were exhausted without an accepted result"),
            format!("/{phase_name}/strategies"),
            json!({}),
        ),
        StrategyPolicy::AllRequired => (
            "strategy_policy_all_required_unsatisfied",
            "all_required policy was not satisfied".to_string(),
            format!("/{phase_name}/policy"),
            json!({ "policy": "all_required" }),
        ),
    };
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.to_string(),
        message,
        severity: DiagnosticSeverity::Error,
        path,
        strategy_key: None,
        details: Some(details),
    }
}

/// Executes a closed, ordered Strategy Policy for a typed phase output.
///
/// Phase adapters own strategy execution, acceptance, failure classification, reduction, and
/// public projection. This kernel alone owns attempt identity/history and deterministic Policy
/// transitions.
pub(crate) async fn execute_strategy_set<'a, S, K, C, F, O>(
    policy: StrategyPolicy,
    strategies: &'a [S],
    strategy_key: K,
    cancellation_before_attempt: C,
    mut execute: F,
) -> StrategySetExecution<O>
where
    K: Fn(&'a S) -> &'a str,
    C: Fn(usize, &'a S) -> Option<TypedCancellation>,
    F: FnMut(usize, &'a S) -> Pin<Box<dyn Future<Output = StrategyExecution<O>> + Send + 'a>>,
{
    let mut attempts = Vec::new();
    for (strategy_index, strategy) in strategies.iter().enumerate() {
        let key = strategy_key(strategy).to_string();
        let execution =
            if let Some(cancellation) = cancellation_before_attempt(strategy_index, strategy) {
                StrategyExecution {
                    diagnostics: Vec::new(),
                    completion: StrategyAttemptCompletion::Cancelled(cancellation),
                }
            } else {
                execute(strategy_index, strategy).await
            };
        let terminal = match &execution.completion {
            StrategyAttemptCompletion::Accepted(_) => match policy {
                StrategyPolicy::FirstAccepted => Some(StrategySetTerminal::Satisfied),
                StrategyPolicy::AllRequired if strategy_index + 1 == strategies.len() => {
                    Some(StrategySetTerminal::Satisfied)
                }
                StrategyPolicy::AllRequired => None,
            },
            StrategyAttemptCompletion::Rejected | StrategyAttemptCompletion::Failed => match policy
            {
                StrategyPolicy::FirstAccepted => None,
                StrategyPolicy::AllRequired => Some(StrategySetTerminal::PolicyUnsatisfied),
            },
            StrategyAttemptCompletion::Cancelled(cancellation) => {
                Some(StrategySetTerminal::Cancelled(cancellation.clone()))
            }
            StrategyAttemptCompletion::Stopped(stop) => {
                Some(StrategySetTerminal::Stopped(stop.clone()))
            }
        };
        attempts.push(StrategyAttempt {
            strategy_index,
            strategy_key: key,
            diagnostics: execution.diagnostics,
            completion: execution.completion,
        });
        if let Some(terminal) = terminal {
            return StrategySetExecution { attempts, terminal };
        }
    }

    StrategySetExecution {
        attempts,
        terminal: StrategySetTerminal::PolicyUnsatisfied,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

    use super::*;

    #[tokio::test]
    async fn cancellation_like_diagnostic_is_not_control_state() {
        let diagnostic = Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "runtime_execution_cancelled".to_string(),
            message: "fake cancellation diagnostic".to_string(),
            severity: DiagnosticSeverity::Error,
            path: "/fake".to_string(),
            strategy_key: Some("first".to_string()),
            details: Some(json!({})),
        };
        let strategies = ["first", "second"];
        let result = execute_strategy_set(
            StrategyPolicy::FirstAccepted,
            &strategies,
            |strategy| *strategy,
            |_, _| None,
            |index, _| {
                let diagnostic = diagnostic.clone();
                Box::pin(async move {
                    if index == 0 {
                        StrategyExecution::<u8> {
                            diagnostics: vec![diagnostic],
                            completion: StrategyAttemptCompletion::Rejected,
                        }
                    } else {
                        StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Accepted(7),
                        }
                    }
                })
            },
        )
        .await;

        assert!(matches!(result.terminal, StrategySetTerminal::Satisfied));
        assert_eq!(result.attempts.len(), 2);
    }

    #[tokio::test]
    async fn all_required_transition_is_generic_and_fails_fast() {
        let strategies = ["first", "second", "never"];
        let result = execute_strategy_set(
            StrategyPolicy::AllRequired,
            &strategies,
            |strategy| *strategy,
            |_, _| None,
            |index, _| {
                Box::pin(async move {
                    if index == 0 {
                        StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Accepted(7_u8),
                        }
                    } else {
                        StrategyExecution {
                            diagnostics: Vec::new(),
                            completion: StrategyAttemptCompletion::Rejected,
                        }
                    }
                })
            },
        )
        .await;

        assert!(matches!(
            result.terminal,
            StrategySetTerminal::PolicyUnsatisfied
        ));
        assert_eq!(result.attempts.len(), 2);
        assert!(matches!(
            result.attempts[0].completion,
            StrategyAttemptCompletion::Accepted(7)
        ));
    }
}
