mod plan;
mod reconciliation;

pub use plan::{
    compile_detection_plan, detection_descriptor_for_authored_kind,
    detection_descriptor_for_url_input_kind, detection_shape_descriptors,
    validate_detection_shape_descriptors, CompiledDetectionPlan, DetectionDescriptorShape,
    DetectionOptionDescriptor, DetectionShapeDescriptor, DETECTION_BROWSER_DESCRIPTOR,
    DETECTION_HTTP_DESCRIPTOR, DETECTION_INPUT_URL_PATTERN_DESCRIPTOR,
    DETECTION_URL_ABSOLUTE_DESCRIPTOR, DETECTION_URL_DESCRIPTOR,
    DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR,
};
#[doc(hidden)]
pub use plan::{CompiledDetectionJsonValue, CompiledDetectionStrategy, CompiledUrlInput};
pub use reconciliation::{
    aggregate_detection_attempts, DetectionAttempt, DetectionConfigContribution,
    DetectionContribution, DetectionDefinitionError, DetectionEvidenceContribution,
    DetectionOrigin, DetectionProfileContext, DetectionProposalProvenance,
    DetectionReconciliationError, DetectionRunStatus, DetectionStateConflict,
    DetectionStateConflictKind, PreparedDetectionOutput, ProposalEvidence, ReconciledCapture,
    ReconciledDetectionRunResult, ReconciledDetectionState, ReconciledEvidence,
    ReconciledRecommendation, ReconciledSourceConfigValue, ReconciledSourceProposal,
    UnsupportedReconciledDetection,
};

pub(crate) use plan::completeness_compiled_registrations;
