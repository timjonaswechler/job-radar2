use std::time::Duration;

use serde_json::json;

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};

pub const RUNTIME_EXECUTION_CANCELLED_CODE: &str = "runtime_execution_cancelled";

/// Domain-owned cooperative cancellation signal for Profile DSL runtime work.
pub trait RuntimeCancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// Execution controls shared by Profile DSL runtime primitives.
#[derive(Clone, Copy, Default)]
pub struct RuntimeExecutionContext<'a> {
    cancellation: Option<&'a dyn RuntimeCancellation>,
}

impl<'a> RuntimeExecutionContext<'a> {
    pub const fn uncancellable() -> Self {
        Self { cancellation: None }
    }

    pub const fn with_cancellation(cancellation: &'a dyn RuntimeCancellation) -> Self {
        Self {
            cancellation: Some(cancellation),
        }
    }

    pub fn is_cancelled(self) -> bool {
        self.cancellation
            .is_some_and(RuntimeCancellation::is_cancelled)
    }

    pub async fn cancelled(self) {
        let Some(cancellation) = self.cancellation else {
            std::future::pending::<()>().await;
            return;
        };
        while !cancellation.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

pub(crate) fn runtime_execution_cancelled_diagnostic(
    path: impl Into<String>,
    strategy_key: Option<&str>,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: RUNTIME_EXECUTION_CANCELLED_CODE.to_string(),
        message: "Profile DSL runtime execution cancelled".to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(json!({ "reason": "user_cancelled" })),
    }
}

pub(crate) fn contains_runtime_execution_cancelled(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == RUNTIME_EXECUTION_CANCELLED_CODE)
}

pub(crate) fn push_runtime_execution_cancelled(
    diagnostics: &mut Diagnostics,
    path: impl Into<String>,
    strategy_key: Option<&str>,
) {
    if !contains_runtime_execution_cancelled(diagnostics) {
        diagnostics.push(runtime_execution_cancelled_diagnostic(path, strategy_key));
    }
}
