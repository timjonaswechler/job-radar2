//! Prepared, bounded Profile Detection and Source Proposal reconciliation.
//!
//! Detection attempt and prepared-profile internals remain private:
//! ```compile_fail
//! use source_profile_dsl::detection::reconciliation::DetectionAttempt;
//! ```

pub(crate) mod plan;
pub(crate) mod reconciliation;
pub(crate) mod runtime;

pub use plan::{compile_detection_plan, CompiledDetectionPlan};
pub use reconciliation::{
    DetectionRunStatus, ReconciledDetectionRunResult, ReconciledSourceProposal,
    UnsupportedReconciledDetection,
};
pub use runtime::{execute_detection_operation, DetectionOperationResult};

pub(crate) use plan::completeness_compiled_registrations;
