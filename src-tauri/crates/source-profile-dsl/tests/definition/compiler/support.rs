#![allow(dead_code)]

use source_profile_dsl::test_support::{
    Diagnostics, SourceDocument, SourceProfileDocument, SourceProfileLookup,
};

#[derive(Default)]
pub struct SourceProfileRegistrySnapshot {
    pub profiles: Vec<RegistrySourceProfile>,
    pub sources: Vec<RegistrySource>,
    pub diagnostics: Diagnostics,
}

impl SourceProfileLookup for SourceProfileRegistrySnapshot {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles
            .iter()
            .find(|profile| profile.document.key == key)
            .map(|profile| &profile.document)
    }
}

pub struct RegistrySourceProfile {
    pub origin: String,
    pub path: String,
    pub document: SourceProfileDocument,
}

pub struct RegistrySource {
    pub origin: String,
    pub path: String,
    pub document: SourceDocument,
    pub validation_state: SourceValidationState,
    pub effective_profile: Option<()>,
    pub compile_outcome: Option<()>,
}

pub struct SourceValidationState {
    pub source_key: String,
    pub state: ValidationStateKind,
    pub can_compile: bool,
    pub can_execute: bool,
    pub diagnostics: Diagnostics,
}

pub enum ValidationStateKind {
    Valid,
}
