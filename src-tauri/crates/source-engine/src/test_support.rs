//! Explicit opt-in support for deterministic tests of private engine edges.

pub use crate::definition::compiler::*;
pub use crate::definition::diagnostics::*;
pub use crate::definition::documents::*;
pub use crate::definition::execution_plan::capabilities::*;
pub use crate::definition::execution_plan::*;
pub use crate::definition::policy::*;
pub use crate::definition::primitives::acceptance::*;
pub use crate::definition::primitives::capture::*;
pub use crate::definition::primitives::cardinality::*;
pub use crate::definition::primitives::completeness::*;
pub use crate::definition::primitives::fetch::browser::*;
pub use crate::definition::primitives::fetch::http::*;
pub use crate::definition::primitives::pagination::*;
pub use crate::definition::primitives::parse::*;
pub use crate::definition::primitives::predicate::*;
pub use crate::definition::primitives::select::*;
pub use crate::definition::primitives::transform::*;
pub use crate::definition::primitives::value::*;
pub use crate::definition::template::*;
pub use crate::definition::*;
pub use crate::detection::plan::{
    detection_descriptor_for_authored_kind, detection_descriptor_for_url_input_kind,
    detection_shape_descriptors, validate_detection_shape_descriptors, CompiledDetectionJsonValue,
    CompiledDetectionStrategy, CompiledUrlInput, DetectionDescriptorShape,
    DetectionOptionDescriptor, DetectionShapeDescriptor, DETECTION_BROWSER_DESCRIPTOR,
    DETECTION_HTTP_DESCRIPTOR, DETECTION_INPUT_URL_PATTERN_DESCRIPTOR,
    DETECTION_URL_ABSOLUTE_DESCRIPTOR, DETECTION_URL_DESCRIPTOR,
    DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR,
};
pub use crate::detection::reconciliation::{
    aggregate_detection_attempts, DetectionAttempt, DetectionConfigContribution,
    DetectionContribution, DetectionDefinitionError, DetectionEvidenceContribution,
    DetectionOrigin, DetectionProfileContext, DetectionProposalProvenance,
    DetectionReconciliationError, DetectionStateConflict, DetectionStateConflictKind,
    PreparedDetectionOutput, ProposalEvidence, ReconciledCapture, ReconciledDetectionState,
    ReconciledEvidence, ReconciledRecommendation, ReconciledSourceConfigValue,
};
pub use crate::detection::runtime::{
    DetectionBrowserFailureKind, DetectionProfileCompletion, DetectionProfileExecutionFailureKind,
    DetectionProfileOutcome, DetectionProfileRejectionKind,
};
pub use crate::detection::*;
pub use crate::execution::browser_acquisition::{
    BrowserAcquisitionTestInvocation as __TestBrowserAcquisitionInvocation,
    ScriptedBrowserAcquisition, ScriptedBrowserAcquisitionEvent,
    ScriptedBrowserAcquisitionExpectation, ScriptedBrowserFinalization,
};
pub use crate::execution::detail::execute_detail;
pub use crate::execution::discovery::execute_discovery;
pub use crate::execution::http::{
    ScriptedHttpBodyEvent, ScriptedHttpEvent, ScriptedProfileHttpClient,
};
pub use crate::execution::source_detail::ScriptedSourceDetailExecution;
pub use crate::execution::*;

pub fn test_execution_plan(
    source: &crate::definition::CompiledSource,
) -> crate::definition::execution_plan::SourceExecutionPlan {
    source.execution_plan.clone()
}

pub fn test_access(
    source: &crate::definition::CompiledSource,
) -> crate::definition::compiler::CompiledSourceAccess {
    source.access.clone()
}

pub fn test_provenance(
    source: &crate::definition::CompiledSource,
) -> &crate::definition::compiler::CompiledSourceProvenance {
    &source.provenance
}

pub fn test_source_config(
    source: &crate::definition::CompiledSource,
) -> &crate::definition::JsonObject {
    &source.source_config
}
pub use crate::execution::browser_acquisition::BrowserAcquisitionTestInvocation;
pub use crate::execution::detail::execute_detail as __test_execute_detail_phase;
