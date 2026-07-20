use serde::{Deserialize, Serialize};

use crate::profile_dsl::compiler::CompileSourceOutcome;
use crate::profile_dsl::diagnostics::Diagnostics;
use crate::source::documents::SourceDocument;
use crate::source::validation::SourceValidationState;
use crate::source_profile::documents::SourceProfileDocument;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySourceProfile {
    pub origin: String,
    pub path: String,
    pub document: SourceProfileDocument,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    /// Productive callers reuse it instead of recompiling or reconstructing plans.
    #[serde(skip)]
    pub compile_outcome: Option<CompileSourceOutcome>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProfileRegistrySnapshot {
    pub profiles: Vec<RegistrySourceProfile>,
    pub sources: Vec<RegistrySource>,
    pub diagnostics: Diagnostics,
}

impl SourceProfileRegistrySnapshot {
    pub fn profile(&self, key: &str) -> Option<&RegistrySourceProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.document.key == key)
    }

    pub fn source(&self, key: &str) -> Option<&RegistrySource> {
        self.sources
            .iter()
            .find(|source| source.document.key == key)
    }
}
