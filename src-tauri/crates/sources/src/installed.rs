//! Admission and immutable views of installed Source Profiles.
//!
//! [`Profiles::load`] owns bounded local filesystem work. A successful value
//! contains only admitted definitions in productive lookup and prepared
//! Detection material; rejected Custom Profiles remain inspectable through
//! [`Profiles::view`].

mod limits;
mod loading;
mod mutations;
pub(crate) mod persistence;
mod preparation;
mod profiles;
mod snapshot;
mod sources;

pub use limits::{
    MAX_AGGREGATE_PROFILE_BYTES, MAX_AGGREGATE_SOURCE_BYTES, MAX_CUSTOM_PROFILE_DOCUMENTS,
    MAX_CUSTOM_SOURCE_DOCUMENTS, MAX_DIAGNOSTICS_PER_DOCUMENT, MAX_DIAGNOSTICS_PER_SNAPSHOT,
    MAX_PROFILE_BYTES, MAX_SOURCE_BYTES, MAX_SOURCE_DIAGNOSTICS_PER_DOCUMENT,
    MAX_SOURCE_DIAGNOSTICS_PER_SNAPSHOT,
};
pub use mutations::{Error, ErrorKind, Store};
pub use snapshot::{
    Admission, Generation, LoadError, LoadErrorKind, Origin, PreparedSource, ProfileDefinitionView,
    ProfileView, Profiles, ProfilesView, ResolvedBehaviorView, Snapshot, SnapshotView, SourceView,
    ValidationState, ValidationStateKind,
};
pub use source_profile_dsl::definition::SelectedAccessPath;
pub use sources::{CreateDraft, InactiveStatus, Revision, SourceDocument, SourceStatus};
