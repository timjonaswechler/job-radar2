pub(crate) mod registry {
    mod loading;
    mod snapshot;

    pub use loading::load_snapshot;
    pub use snapshot::{RegistrySource, SourceProfileRegistrySnapshot};
}
