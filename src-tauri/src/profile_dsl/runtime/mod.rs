pub(crate) mod allowance;
pub(crate) mod browser;
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
pub use browser::{
    ManagedProfileBrowserClient, ProfileBrowserClient, ProfileBrowserFetchError,
    ProfileBrowserFetchErrorKind, ProfileBrowserFetchRequest, ProfileBrowserFetchResponse,
    UnavailableProfileBrowserClient,
};
pub use cancellation::{RuntimeCancellation, RuntimeExecutionContext};
pub use discovery::execute_discovery;
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
