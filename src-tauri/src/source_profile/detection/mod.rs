mod strategy;

pub use source_profile_dsl::source_profile::detection::{
    aggregate_detection_attempts, compile_detection_plan, detection_descriptor_for_authored_kind,
    detection_descriptor_for_url_input_kind, detection_shape_descriptors,
    validate_detection_shape_descriptors, CompiledDetectionPlan, DetectionAttempt,
    DetectionConfigContribution, DetectionContribution, DetectionDefinitionError,
    DetectionDescriptorShape, DetectionEvidenceContribution, DetectionOptionDescriptor,
    DetectionOrigin, DetectionProfileContext, DetectionProposalProvenance,
    DetectionReconciliationError, DetectionRunStatus, DetectionShapeDescriptor,
    DetectionStateConflict, DetectionStateConflictKind, PreparedDetectionOutput, ProposalEvidence,
    ReconciledCapture, ReconciledDetectionRunResult, ReconciledDetectionState, ReconciledEvidence,
    ReconciledRecommendation, ReconciledSourceConfigValue, ReconciledSourceProposal,
    UnsupportedReconciledDetection, DETECTION_BROWSER_DESCRIPTOR, DETECTION_HTTP_DESCRIPTOR,
    DETECTION_INPUT_URL_PATTERN_DESCRIPTOR, DETECTION_URL_ABSOLUTE_DESCRIPTOR,
    DETECTION_URL_DESCRIPTOR, DETECTION_URL_PATTERN_ALTERNATIVES_DESCRIPTOR,
};
pub use strategy::{
    execute_detection_operation, DetectionBrowserFailureKind, DetectionOperationResult,
    DetectionProfileCompletion, DetectionProfileExecutionFailureKind, DetectionProfileOutcome,
    DetectionProfileRejectionKind,
};
