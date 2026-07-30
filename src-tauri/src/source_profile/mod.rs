pub(crate) use source_profile_dsl::source_profile::documents;
pub(crate) mod detection {
    pub use source_profile_dsl::source_profile::detection::*;
}
pub(crate) mod registry {
    mod builtins;
    mod loading;
    mod snapshot;

    pub use loading::load_snapshot;
    pub use snapshot::{RegistrySource, RegistrySourceProfile, SourceProfileRegistrySnapshot};
}
