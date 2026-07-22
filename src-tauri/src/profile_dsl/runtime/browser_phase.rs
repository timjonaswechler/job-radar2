use serde_json::json;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity},
    execution_plan::capabilities::{ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait},
};

use super::{
    browser_acquisition::{
        BrowserAcquisition, BrowserAcquisitionFailureKind, BrowserAcquisitionRequest,
        BrowserAcquisitionTerminal,
    },
    cancellation::{
        CancellationOperation, RuntimeExecutionContext, RuntimePhase, TypedCancellation,
    },
};

/// Exhaustive Browser capability supplied by a posting-phase caller.
///
/// Browser-free plans carry no Browser dependency. Plans containing Browser Fetch
/// carry exactly one phase-specific adapter.
pub enum PhaseBrowser<A> {
    BrowserFree,
    Browser(A),
}

pub(crate) struct BrowserPhaseFetchInput<'a> {
    pub(crate) target: String,
    pub(crate) timeout_ms: u64,
    pub(crate) waits: Vec<ExecutionPlanBrowserWait>,
    pub(crate) interactions: Vec<ExecutionPlanBrowserInteraction>,
    pub(crate) base_path: String,
    pub(crate) strategy_key: String,
    pub(crate) strategy_index: usize,
    pub(crate) control: RuntimeExecutionContext<'a>,
}

pub(crate) enum BrowserPhaseFetchProjection {
    Rendered(String),
    AttemptFailed(Diagnostic),
    PhaseFatal(Diagnostic),
    AllowanceStopped,
    Cancelled(TypedCancellation),
}

pub(crate) async fn execute_canonical_browser_fetch(
    acquisition: &dyn BrowserAcquisition,
    phase: RuntimePhase,
    input: BrowserPhaseFetchInput<'_>,
) -> BrowserPhaseFetchProjection {
    let request = match BrowserAcquisitionRequest::new(
        input.target,
        input.timeout_ms,
        input.waits,
        input.interactions,
        input.control,
    ) {
        Ok(request) => request,
        Err(_) => {
            input.control.mark_internal_failure();
            return BrowserPhaseFetchProjection::PhaseFatal(diagnostic(
                "browser_acquisition_invariant_failed",
                "Browser acquisition input violated a compiled runtime invariant",
                format!("{}/fetch", input.base_path),
                &input.strategy_key,
                json!({}),
            ));
        }
    };
    match acquisition.acquire(request).await {
        Ok(rendered) => BrowserPhaseFetchProjection::Rendered(rendered.into_string()),
        Err(BrowserAcquisitionTerminal::Failure(failure)) => {
            let (code, path, details) = failure_diagnostic(&failure.kind, &input.base_path);
            BrowserPhaseFetchProjection::AttemptFailed(diagnostic(
                code,
                "Browser acquisition failed",
                path,
                &input.strategy_key,
                details,
            ))
        }
        Err(BrowserAcquisitionTerminal::InfrastructureFailure(_)) => {
            input.control.mark_internal_failure();
            BrowserPhaseFetchProjection::PhaseFatal(diagnostic(
                "browser_infrastructure_failure",
                "Browser infrastructure could not be finalized safely",
                format!("{}/fetch", input.base_path),
                &input.strategy_key,
                json!({}),
            ))
        }
        Err(BrowserAcquisitionTerminal::AllowanceStopped) => {
            BrowserPhaseFetchProjection::AllowanceStopped
        }
        Err(BrowserAcquisitionTerminal::Cancelled(_)) => {
            BrowserPhaseFetchProjection::Cancelled(TypedCancellation::strategy(
                phase,
                input.strategy_index,
                &input.strategy_key,
                CancellationOperation::Browser,
            ))
        }
    }
}

fn failure_diagnostic(
    kind: &BrowserAcquisitionFailureKind,
    base_path: &str,
) -> (&'static str, String, serde_json::Value) {
    match kind {
        BrowserAcquisitionFailureKind::RuntimeLaunch => (
            "browser_runtime_unavailable",
            format!("{base_path}/fetch"),
            json!({ "kind": "runtime_launch" }),
        ),
        BrowserAcquisitionFailureKind::Navigation => (
            "browser_navigation_failed",
            format!("{base_path}/fetch/url"),
            json!({ "kind": "navigation" }),
        ),
        BrowserAcquisitionFailureKind::Wait { wait_index } => (
            "browser_wait_failed",
            format!("{base_path}/fetch/waits/{wait_index}"),
            json!({ "kind": "wait", "waitIndex": wait_index }),
        ),
        BrowserAcquisitionFailureKind::Interaction { interaction_index } => (
            "browser_interaction_failed",
            format!("{base_path}/fetch/interactions/{interaction_index}"),
            json!({ "kind": "interaction", "interactionIndex": interaction_index }),
        ),
        BrowserAcquisitionFailureKind::ContentRead => (
            "browser_content_read_failed",
            format!("{base_path}/fetch"),
            json!({ "kind": "content_read" }),
        ),
        BrowserAcquisitionFailureKind::Deadline => (
            "browser_render_timeout",
            format!("{base_path}/fetch/timeoutMs"),
            json!({ "kind": "deadline" }),
        ),
    }
}

fn diagnostic(
    code: &str,
    message: &str,
    path: String,
    strategy_key: &str,
    details: serde_json::Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path,
        strategy_key: Some(strategy_key.to_string()),
        details: Some(details),
    }
}
