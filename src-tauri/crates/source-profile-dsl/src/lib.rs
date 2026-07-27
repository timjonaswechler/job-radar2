pub mod profile_dsl;
pub mod source;
pub mod source_profile;

pub use profile_dsl::compiler::{
    compile_source, CompileSourceOutcome, CompiledSource, CompiledSourceAccess,
    CompiledSourceProvenance, EffectiveSourceProfile, ProfileCompilerInput, ProvenanceEntry,
    ProvenanceOrigin, ProvenancePath, ProvenancePathSegment, SourceOwnedAccessPath,
    SourceProfileLookup, SourceRuntimeBinding, SourceRuntimeBindingDependencies,
};
pub use profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
pub use profile_dsl::documents::*;
pub use profile_dsl::execution_plan::capabilities::*;
pub use profile_dsl::execution_plan::{
    ExecutionPlanAccessPath, ExecutionPlanSource, SourceExecutionPlan,
};
pub use profile_dsl::policy::StrategyPolicy;
#[allow(ambiguous_glob_reexports)]
pub use profile_dsl::primitives::acceptance::*;
pub use profile_dsl::primitives::capture::*;
pub use profile_dsl::primitives::cardinality::*;
pub use profile_dsl::primitives::fetch::browser::*;
pub use profile_dsl::primitives::fetch::http::*;
pub use profile_dsl::primitives::pagination::*;
pub use profile_dsl::primitives::parse::*;
pub use profile_dsl::primitives::predicate::*;
pub use profile_dsl::primitives::select::*;
pub use profile_dsl::primitives::transform::*;
pub use profile_dsl::primitives::value::*;
pub use profile_dsl::template::*;
pub use source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
pub use source_profile::detection::{compile_detection_plan, CompiledDetectionPlan};
pub use source_profile::documents::SourceProfileDocument;
