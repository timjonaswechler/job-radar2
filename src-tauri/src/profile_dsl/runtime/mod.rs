pub(crate) mod allowance;
pub(crate) mod browser_acquisition;
pub(crate) mod browser_phase;
pub(crate) mod cancellation;
pub(crate) mod detail;
pub(crate) mod discovery;
pub mod http;
pub mod outcome;
pub(crate) mod reducers;
mod source_detail;
pub(crate) mod strategy_set;

pub use crate::profile_dsl::occurrence::{
    validate_posting_reference, ContributionOrigin, DetailContributionEvidence, DetailField,
    DetailFieldCapabilities, DetailPatch, DetailRejection, DiscoveryContributionEvidence,
    DiscoveryHint, DiscoveryRejection, DiscoveryResponsibility, HintUse, OccurrenceReferenceError,
    PostingOccurrence, PostingOccurrenceIdentity, PostingReference, ProviderValues,
    RequestedDetailFields,
};
pub use allowance::{
    AllowanceDimension, AllowanceExhaustion, AllowanceLimitSource, PhaseCancellationReason,
    PhaseCompletion, PhaseExecutionReport, PhaseUsage,
};

#[doc(hidden)]
pub use browser_acquisition::BrowserAcquisitionTestInvocation as __TestBrowserAcquisitionInvocation;
pub use browser_acquisition::{
    BrowserAcquisition, BrowserAcquisitionCancellation, BrowserAcquisitionCancellationReason,
    BrowserAcquisitionFailure, BrowserAcquisitionFailureKind, BrowserAcquisitionRequest,
    BrowserAcquisitionRequestSnapshot, BrowserAcquisitionTerminal, BrowserInfrastructureFailure,
    BrowserLifecycleEvent, BrowserRenderedContent, ScriptedBrowserAcquisition,
    ScriptedBrowserAcquisitionEvent, ScriptedBrowserAcquisitionExpectation,
    ScriptedBrowserFinalization,
};
pub use browser_phase::PhaseBrowser;
pub use cancellation::{RuntimeCancellation, RuntimeExecutionContext};
pub use detail::DetailBrowserAdapter;
pub use discovery::{execute_discovery, DiscoveryBrowserAdapter};
pub use http::{
    ProfileHttpClient, ProfileHttpError, ProfileHttpFailureKind, ProfileHttpHeader,
    ProfileHttpRequest, ProfileHttpResponse, ReqwestProfileHttpClient, ScriptedHttpBodyEvent,
    ScriptedHttpEvent, ScriptedProfileHttpClient, SensitiveRequestBody,
};
pub use outcome::{
    DetailPhasePayload, DiscoveryPhasePayload, PhaseCancelled, PhaseExecutionFailure, PhaseOutcome,
    PhasePreStartFailure, PhaseRunError, PhaseRunResult, PolicyOutcome, PolicyUnsatisfiedCause,
};
pub use source_detail::{
    CandidateDetailFailure, DetailCancelled, ProfileDslSourceDetailExecution,
    RequestedFieldDisposition, ScriptedSourceDetailExecution, SourceDetailExecution,
    SourceDetailFailure, SourceDetailOutcome, SourceDetailPhaseEvidence, SourceDetailRequest,
    SourceDetailRequestSnapshot, SourceDetailResult,
};
