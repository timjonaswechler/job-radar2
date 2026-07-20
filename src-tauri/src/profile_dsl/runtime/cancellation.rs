use std::time::Duration;

use serde_json::json;

use crate::profile_dsl::diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

const RUNTIME_EXECUTION_CANCELLED_CODE: &str = "runtime_execution_cancelled";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePhase {
    Discovery,
    Detail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CancellationOperation {
    Phase,
    Fetch,
    Browser,
    Pagination,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedCancellation {
    phase: RuntimePhase,
    strategy_index: Option<usize>,
    strategy_key: Option<String>,
    operation: CancellationOperation,
}

impl TypedCancellation {
    pub(crate) fn phase(phase: RuntimePhase) -> Self {
        Self {
            phase,
            strategy_index: None,
            strategy_key: None,
            operation: CancellationOperation::Phase,
        }
    }

    pub(crate) fn strategy(
        phase: RuntimePhase,
        strategy_index: usize,
        strategy_key: &str,
        operation: CancellationOperation,
    ) -> Self {
        Self {
            phase,
            strategy_index: Some(strategy_index),
            strategy_key: Some(strategy_key.to_string()),
            operation,
        }
    }

    fn path(&self) -> String {
        let phase = match self.phase {
            RuntimePhase::Discovery => "discovery",
            RuntimePhase::Detail => "detail",
        };
        let Some(strategy_index) = self.strategy_index else {
            return format!("/{phase}");
        };
        let suffix = match self.operation {
            CancellationOperation::Phase => "",
            CancellationOperation::Fetch | CancellationOperation::Browser => "/fetch",
            CancellationOperation::Pagination => "/pagination",
        };
        format!("/{phase}/strategies/{strategy_index}{suffix}")
    }

    fn strategy_key(&self) -> Option<&str> {
        self.strategy_key.as_deref()
    }
}

/// Domain-owned cooperative cancellation signal for Profile DSL runtime work.
pub trait RuntimeCancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// Caller-owned request budget for bounded Discovery execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscoveryExecutionBudget {
    max_requests_per_strategy: u64,
}

impl DiscoveryExecutionBudget {
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
    discovery_budget: Option<DiscoveryExecutionBudget>,
}

impl<'a> RuntimeExecutionContext<'a> {
    pub const fn uncancellable() -> Self {
        Self {
            cancellation: None,
            discovery_budget: None,
        }
    }

    pub const fn with_cancellation(cancellation: &'a dyn RuntimeCancellation) -> Self {
        Self {
            cancellation: Some(cancellation),
            discovery_budget: None,
        }
    }

    pub const fn with_discovery_budget(mut self, budget: DiscoveryExecutionBudget) -> Self {
        self.discovery_budget = Some(budget);
        self
    }

    pub(crate) fn discovery_request_limit(self, configured_max_requests: u64) -> (u64, bool) {
        let Some(budget) = self.discovery_budget else {
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
    cancellation: &TypedCancellation,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: RUNTIME_EXECUTION_CANCELLED_CODE.to_string(),
        message: "Profile DSL runtime execution cancelled".to_string(),
        severity: DiagnosticSeverity::Error,
        path: cancellation.path(),
        strategy_key: cancellation.strategy_key().map(ToString::to_string),
        details: Some(json!({ "reason": "user_cancelled" })),
    }
}
