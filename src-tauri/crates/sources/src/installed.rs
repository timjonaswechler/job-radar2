//! Admission and immutable views of installed Source Profiles.
//!
//! [`Profiles::load`] owns bounded local filesystem work. A successful value
//! contains only admitted definitions in productive lookup and prepared
//! Detection material; rejected Custom Profiles remain inspectable through
//! [`Profiles::view`].

mod limits;
mod profiles;
mod snapshot;

pub use limits::{
    MAX_AGGREGATE_PROFILE_BYTES, MAX_CUSTOM_PROFILE_DOCUMENTS, MAX_DIAGNOSTICS_PER_DOCUMENT,
    MAX_DIAGNOSTICS_PER_SNAPSHOT, MAX_PROFILE_BYTES,
};
pub use snapshot::{
    Admission, LoadError, LoadErrorKind, Origin, ProfileDefinitionView, ProfileView, Profiles,
    ProfilesView,
};
