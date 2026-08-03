//! Bounded Discovery and candidate-scoped Detail execution.

pub(crate) mod allowance;
pub(crate) mod browser_acquisition;
pub(crate) mod browser_phase;
pub(crate) mod cancellation;
pub(crate) mod detail;
pub(crate) mod discovery;
pub(crate) mod http;
pub(crate) mod occurrence;
pub(crate) mod outcome;
pub(crate) mod reducers;
pub(crate) mod source_detail;
pub(crate) mod strategy_set;

pub use allowance::{
    AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource, PhaseCancellationReason,
    PhaseCompletion, PhaseExecutionReport, PhaseUsage, BROWSER_FORCE_TERMINATE_REAP_MS,
    BROWSER_GRACEFUL_CLOSE_MS, BROWSER_HANDLER_COMPLETION_MS, BROWSER_SESSION_FINALIZATION_MS,
};
pub use browser_acquisition::{
    probe_browser_acquisition, BoxedBrowserAcquisitionFuture, BrowserAcquisition,
    BrowserAcquisitionCancellation, BrowserAcquisitionCancellationReason,
    BrowserAcquisitionFailure, BrowserAcquisitionFailureKind, BrowserAcquisitionRequest,
    BrowserAcquisitionRequestSnapshot, BrowserAcquisitionTerminal, BrowserInfrastructureFailure,
    BrowserInteractionInstruction, BrowserLifecycleEvent, BrowserRenderedContent,
    BrowserWaitInstruction,
};
pub use browser_phase::PhaseBrowser;
pub use cancellation::{RuntimeCancellation, RuntimeExecutionContext};
pub use detail::DetailBrowserAdapter;
pub use discovery::DiscoveryBrowserAdapter;
pub use http::{
    collect_profile_http_response, ProfileHttpClient, ProfileHttpError, ProfileHttpFailureKind,
    ProfileHttpHeader, ProfileHttpRequest, ProfileHttpResponse, SensitiveRequestBody,
};
pub use occurrence::{
    validate_posting_reference, ContributionOrigin, DetailContributionEvidence, DetailField,
    DetailFieldCapabilities, DetailPatch, DetailRejection, DiscoveryContributionEvidence,
    DiscoveryHint, DiscoveryRejection, DiscoveryResponsibility, HintUse, OccurrenceReferenceError,
    PostingOccurrence, PostingOccurrenceIdentity, PostingReference, ProviderValues,
    RequestedDetailFields,
};
pub use outcome::{
    DetailPhasePayload, DiscoveryPhasePayload, PhaseCancelled, PhaseExecutionFailure, PhaseOutcome,
    PhasePreStartFailure, PhaseRunError, PhaseRunResult, PolicyOutcome, PolicyUnsatisfiedCause,
};
pub use source_detail::{
    CandidateDetailFailure, DetailCancelled, RequestedFieldDisposition,
    SourceBehaviorDetailExecution, SourceDetailExecution, SourceDetailFailure, SourceDetailOutcome,
    SourceDetailPhaseEvidence, SourceDetailRequest, SourceDetailRequestSnapshot,
    SourceDetailResult,
};

impl crate::definition::CompiledSource {
    pub fn discovery_limits(&self) -> crate::definition::PhaseLimits {
        self.execution_plan.discovery.limits
    }

    pub fn supports_detail(&self) -> bool {
        self.execution_plan.detail.is_some()
    }

    /// Accepted limits for one candidate-scoped Detail execution, when Detail is supported.
    pub fn detail_limits(&self) -> Option<crate::definition::PhaseLimits> {
        self.execution_plan
            .detail
            .as_ref()
            .map(|detail| detail.limits)
    }

    pub fn discovery_uses_browser(&self) -> bool {
        self.execution_plan.discovery.strategies.iter().any(|strategy| {
            matches!(
                strategy.fetch,
                crate::definition::execution_plan::capabilities::ExecutionPlanFetch::Browser { .. }
            )
        })
    }
}

/// Executes Discovery without exposing the prepared plan to callers.
pub async fn discover(
    source: &crate::definition::CompiledSource,
    fetcher: &(dyn ProfileHttpClient + Sync),
    acquisition: &dyn BrowserAcquisition,
    context: RuntimeExecutionContext<'_>,
) -> PhaseRunResult<DiscoveryPhasePayload> {
    let browser = if source.discovery_uses_browser() {
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(acquisition))
    } else {
        PhaseBrowser::BrowserFree
    };
    discovery::execute_discovery(
        &source.execution_plan,
        &source.source_config,
        fetcher,
        browser,
        context,
    )
    .await
}
