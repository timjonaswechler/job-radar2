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

/// Caller-owned request budget for bounded `postingDiscovery` execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostingDiscoveryExecutionBudget {
    max_requests_per_strategy: u64,
}

impl PostingDiscoveryExecutionBudget {
    pub const fn new(max_requests_per_strategy: u64) -> Self {
        assert!(max_requests_per_strategy > 0);
        Self {
            max_requests_per_strategy,
        }
    }

    pub const fn max_requests_per_strategy(self) -> u64 {
        self.max_requests_per_strategy
    }
}

/// Execution controls shared by Profile DSL runtime primitives.
#[derive(Clone, Copy, Default)]
pub struct RuntimeExecutionContext<'a> {
    cancellation: Option<&'a dyn RuntimeCancellation>,
    posting_discovery_budget: Option<PostingDiscoveryExecutionBudget>,
}

impl<'a> RuntimeExecutionContext<'a> {
    pub const fn uncancellable() -> Self {
        Self {
            cancellation: None,
            posting_discovery_budget: None,
        }
    }

    pub const fn with_cancellation(cancellation: &'a dyn RuntimeCancellation) -> Self {
        Self {
            cancellation: Some(cancellation),
            posting_discovery_budget: None,
        }
    }

    pub const fn with_posting_discovery_budget(
        mut self,
        budget: PostingDiscoveryExecutionBudget,
    ) -> Self {
        self.posting_discovery_budget = Some(budget);
        self
    }

    pub(crate) fn posting_discovery_request_limit(
        self,
        configured_max_requests: u64,
    ) -> (u64, bool) {
        let Some(budget) = self.posting_discovery_budget else {
            return (configured_max_requests, false);
        };
        let budget_max_requests = budget.max_requests_per_strategy();
        let effective_max_requests = configured_max_requests.min(budget_max_requests);
        (
            effective_max_requests,
            budget_max_requests <= configured_max_requests,
        )
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
