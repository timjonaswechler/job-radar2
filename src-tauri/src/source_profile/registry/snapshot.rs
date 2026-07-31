use serde::Serialize;
use sources::installed::Profiles;

use crate::source::validation::SourceValidationState;
use source_profile_dsl::definition::{
    CompileSourceOutcome, Diagnostics, SourceDocument, SourceProfileDocument, SourceProfileLookup,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySource {
    pub origin: String,
    pub path: String,
    pub document: SourceDocument,
    pub validation_state: SourceValidationState,
    /// Compiler-owned effective behavior for profile-selected Sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_profile: Option<SourceProfileDocument>,
    /// Exact outcome prepared while loading this immutable registry snapshot.
    #[serde(skip)]
    pub compile_outcome: Option<CompileSourceOutcome>,
}

/// Temporary Desktop-owned Source snapshot. Installed Profile admission is
/// owned by `sources::installed` and is deliberately not serialized here.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProfileRegistrySnapshot {
    #[serde(skip)]
    profiles: Profiles,
    pub sources: Vec<RegistrySource>,
    pub diagnostics: Diagnostics,
}

impl SourceProfileRegistrySnapshot {
    pub fn new(profiles: Profiles, sources: Vec<RegistrySource>, diagnostics: Diagnostics) -> Self {
        Self {
            profiles,
            sources,
            diagnostics,
        }
    }

    pub fn installed_profiles(&self) -> &Profiles {
        &self.profiles
    }

    pub fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles.profile(key)
    }

    pub fn source(&self, key: &str) -> Option<&RegistrySource> {
        self.sources
            .iter()
            .find(|source| source.document.key == key)
    }
}

impl SourceProfileLookup for SourceProfileRegistrySnapshot {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles.profile(key)
    }
}
