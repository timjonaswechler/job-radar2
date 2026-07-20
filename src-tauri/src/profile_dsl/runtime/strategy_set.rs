use std::{future::Future, pin::Pin};

use crate::profile_dsl::diagnostics::Diagnostics;

use super::cancellation::TypedCancellation;

pub(crate) enum StrategyAttemptCompletion<O> {
    Accepted(O),
    Rejected,
    Failed,
    Cancelled(TypedCancellation),
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
    Accepted { attempt_index: usize },
    Cancelled(TypedCancellation),
    Exhausted,
}

pub(crate) struct StrategySetExecution<O> {
    pub(crate) attempts: Vec<StrategyAttempt<O>>,
    pub(crate) terminal: StrategySetTerminal,
}

/// Executes the closed, ordered `first_accepted` policy for a typed phase output.
///
/// Phase adapters own strategy execution, acceptance, failure classification, and public
/// projection. This kernel alone owns attempt identity/history and deterministic stopping.
pub(crate) async fn execute_first_accepted<'a, S, K, C, F, O>(
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
            StrategyAttemptCompletion::Accepted(_) => Some(StrategySetTerminal::Accepted {
                attempt_index: attempts.len(),
            }),
            StrategyAttemptCompletion::Cancelled(cancellation) => {
                Some(StrategySetTerminal::Cancelled(cancellation.clone()))
            }
            StrategyAttemptCompletion::Rejected | StrategyAttemptCompletion::Failed => None,
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
        terminal: StrategySetTerminal::Exhausted,
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
        let result = execute_first_accepted(
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

        assert!(matches!(
            result.terminal,
            StrategySetTerminal::Accepted { attempt_index: 1 }
        ));
        assert_eq!(result.attempts.len(), 2);
    }
}
