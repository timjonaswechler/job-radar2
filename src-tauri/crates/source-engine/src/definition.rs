//! Authored Source Behavior Language meaning and authoritative preparation.
//!
//! Implementation modules remain private even with `test-support` enabled:
//! ```compile_fail
//! use source_engine::definition::compiler::compile_source;
//! ```
//! Prepared behavior cannot be inspected as raw fields:
//! ```compile_fail
//! use source_engine::definition::CompiledSource;
//! fn inspect(source: &CompiledSource) { let _ = &source.execution_plan; }
//! ```
//! Edge fixtures are available only through the opt-in `test_support` namespace.

pub(crate) mod compiler;
pub(crate) mod diagnostics;
pub(crate) mod documents;
pub(crate) mod execution_plan;
pub(crate) mod policy;
pub(crate) mod primitives;
pub(crate) mod profile;
pub(crate) mod source_config;
pub(crate) mod template;

pub use compiler::{
    compile_source, compile_source_with_admitted_profiles, forbidden_request_key_behavior,
    prepare_source_profile_document, validate_source_profile_document, CompileSourceOutcome,
    CompiledSource, CompiledSourceAccess, CompiledSourceProvenance, EffectiveSourceProfile,
    ProfileCompilerInput, ProvenanceEntry, ProvenanceOrigin, ProvenancePath, ProvenancePathSegment,
    SourceOwnedAccessPath, SourceProfileLookup, SourceRuntimeBinding,
    SourceRuntimeBindingDependencies, MAX_FALLBACK_STRATEGIES,
};
pub use diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics};
pub use documents::*;
pub use documents::{SelectedAccessPath, SourceBehavior, SourceConfig};
pub use profile::{SourceProfileDocument, SourceProfileKind};
