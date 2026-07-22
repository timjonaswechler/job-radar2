use std::time::Duration;

use serde_json::json;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity},
    documents::PhaseLimits,
};

use super::allowance::{AllowanceCharge, AllowanceStop, InvocationAllowance};

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

pub trait RuntimeCancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// Caller controls for a phase invocation. Limits may only tighten the compiled plan.
#[derive(Clone, Copy, Default)]
pub struct RuntimeExecutionContext<'a> {
    cancellation: Option<&'a dyn RuntimeCancellation>,
    caller_limits: Option<PhaseLimits>,
    allowance: Option<&'a InvocationAllowance>,
    page_request: bool,
    pagination_max_requests: Option<u64>,
}

impl<'a> RuntimeExecutionContext<'a> {
    pub const fn uncancellable() -> Self {
        Self {
            cancellation: None,
            caller_limits: None,
            allowance: None,
            page_request: false,
            pagination_max_requests: None,
        }
    }
    pub const fn with_cancellation(cancellation: &'a dyn RuntimeCancellation) -> Self {
        Self {
            cancellation: Some(cancellation),
            caller_limits: None,
            allowance: None,
            page_request: false,
            pagination_max_requests: None,
        }
    }
    pub const fn with_limits(mut self, limits: PhaseLimits) -> Self {
        self.caller_limits = Some(limits);
        self
    }
    pub(crate) const fn caller_limits(self) -> Option<PhaseLimits> {
        self.caller_limits
    }
    pub(crate) fn for_invocation<'b>(
        &self,
        allowance: &'b InvocationAllowance,
    ) -> RuntimeExecutionContext<'b>
    where
        'a: 'b,
    {
        RuntimeExecutionContext {
            cancellation: self.cancellation,
            caller_limits: self.caller_limits,
            allowance: Some(allowance),
            page_request: self.page_request,
            pagination_max_requests: self.pagination_max_requests,
        }
    }
    pub(crate) const fn with_page_request(mut self, page_request: bool) -> Self {
        self.page_request = page_request;
        self
    }
    pub(crate) const fn with_pagination_limit(mut self, max_requests: u64) -> Self {
        self.pagination_max_requests = Some(max_requests);
        self
    }
    pub(crate) const fn page_request(self) -> bool {
        self.page_request
    }
    pub fn is_cancelled(self) -> bool {
        self.cancellation
            .is_some_and(RuntimeCancellation::is_cancelled)
    }
    pub(crate) fn debit(self, charge: AllowanceCharge) -> Result<(), AllowanceStop> {
        self.allowance.map_or(Ok(()), |allowance| {
            allowance.debit_with_pagination_limit(charge, self.pagination_max_requests)
        })
    }
    pub(crate) fn stop(self) -> Option<AllowanceStop> {
        self.allowance.and_then(InvocationAllowance::stop)
    }
    pub(crate) fn admit_browser_rendered_bytes(self, observed: u64) -> Result<(), AllowanceStop> {
        self.allowance.map_or(Ok(()), |allowance| {
            allowance.admit_browser_rendered_bytes(observed)
        })
    }
    pub(crate) fn remaining_browser_rendered_bytes(self) -> u64 {
        self.allowance.map_or(
            PhaseLimits::BACKEND.max_browser_rendered_bytes,
            InvocationAllowance::remaining_browser_rendered_bytes,
        )
    }
    pub(crate) fn remaining_response_bytes(self) -> u64 {
        self.allowance.map_or(
            PhaseLimits::BACKEND.max_response_bytes,
            InvocationAllowance::remaining_response_bytes,
        )
    }
    pub(crate) fn commit_response_bytes(self, admitted: u64, exceeded: Option<u64>) {
        if let Some(allowance) = self.allowance {
            allowance.commit_response_bytes(admitted, exceeded);
        }
    }
    pub(crate) fn mark_deadline(self) {
        if let Some(allowance) = self.allowance {
            allowance.mark_deadline();
        }
    }
    pub(crate) fn mark_internal_failure(self) {
        if let Some(allowance) = self.allowance {
            allowance.mark_internal_failure();
        }
    }
    pub(crate) fn mark_deadline_if_expired(self) {
        if let Some(allowance) = self.allowance {
            allowance.mark_deadline_if_expired();
        }
    }
    pub(crate) fn effective_limits(self) -> Option<PhaseLimits> {
        self.allowance.map(InvocationAllowance::effective_limits)
    }
    pub(crate) fn deadline(self) -> Option<tokio::time::Instant> {
        self.allowance.map(InvocationAllowance::deadline)
    }
    pub(crate) fn deadline_is_expired(self) -> bool {
        self.deadline()
            .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
    }
    pub(crate) fn browser_work_deadline(self) -> Option<tokio::time::Instant> {
        self.allowance
            .map(InvocationAllowance::browser_work_deadline)
    }
    pub(crate) fn browser_graceful_deadline(self) -> Option<tokio::time::Instant> {
        self.allowance
            .map(InvocationAllowance::browser_graceful_deadline)
    }
    pub(crate) fn browser_force_deadline(self) -> Option<tokio::time::Instant> {
        self.allowance
            .map(InvocationAllowance::browser_force_deadline)
    }
    pub(crate) fn browser_handler_deadline(self) -> Option<tokio::time::Instant> {
        self.allowance
            .map(InvocationAllowance::browser_handler_deadline)
    }
    pub(crate) async fn deadline_reached(self) {
        sleep_until_optional(self.deadline()).await;
    }
    pub(crate) async fn browser_work_deadline_reached(self) {
        sleep_until_optional(self.browser_work_deadline()).await;
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

async fn sleep_until_optional(deadline: Option<tokio::time::Instant>) {
    let Some(deadline) = deadline else {
        std::future::pending::<()>().await;
        return;
    };
    tokio::time::sleep_until(deadline).await;
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
