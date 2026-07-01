#![allow(unused_imports)]

//! Legacy v1 Source registry. Kept temporarily until the new
//! `source_profile` registry and declarative Profile DSL integration replace it.

mod builtins;
mod diagnostics;
mod documents;
mod loading;
mod snapshot;
#[cfg(test)]
mod tests;

pub use builtins::{
    EmbeddedSourceRegistryDocument, BUILTIN_SOURCE_JSON_FILES, BUILTIN_SOURCE_PROFILE_JSON_FILES,
};
pub use diagnostics::{
    SourceRegistryDiagnostic, SourceRegistryDiagnosticCode, SourceRegistryDocumentKind,
    SourceRegistryDocumentOrigin,
};
pub use documents::{
    AvailabilityBlock, BrowserInteraction, DetectionBlock, DetectionPhase,
    ProfileAccessPathDefinition, SelectedAccessPath, SourceDocument, SourceDocumentStatus,
    SourceProfileDocument, SourceProfileIdentity, SourceProfileKind,
};
pub use loading::{load_snapshot, load_snapshot_with_builtins};
pub use snapshot::{
    RegistrySource, RegistrySourceProfile, ResolvedSelectedAccessPath, ResolvedSourceExecutionPlan,
    SourceRegistrySnapshot,
};
