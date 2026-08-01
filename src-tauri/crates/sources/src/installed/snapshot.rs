use std::{fmt, path::Path};

use serde::{Deserialize, Serialize};
use source_profile_dsl::{
    definition::{
        CompileSourceOutcome, CompiledSource, DetectionDocument, Diagnostic, Diagnostics,
        JsonSchemaObject, ReusableAccessPathDocument, SourceProfileDocument, SourceProfileKind,
        SourceProfileLookup, SupportMetadata,
    },
    detection::CompiledDetectionPlan,
};

use super::{loading, profiles, sources::SourceDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    BuiltIn,
    Custom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    Admitted,
    Rejected,
}

/// Intentional inspectable definition projection. Unlike the authored
/// document it excludes schema/parser concerns and compiler material.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDefinitionView {
    pub key: String,
    pub name: String,
    pub kind: SourceProfileKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub support: SupportMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection: Option<DetectionDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    pub access_paths: Vec<ReusableAccessPathDocument>,
}

/// Stable host projection. It deliberately omits raw paths, authored JSON,
/// and prepared compiler material.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub origin: Origin,
    pub admission: Admission,
    pub file_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<ProfileDefinitionView>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesView {
    pub profiles: Vec<ProfileView>,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadErrorKind {
    InvalidBuiltIn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadError {
    kind: LoadErrorKind,
    diagnostics: Diagnostics,
}

impl LoadError {
    pub fn kind(&self) -> LoadErrorKind {
        self.kind
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn invalid_builtin(diagnostics: Diagnostics) -> Self {
        Self {
            kind: LoadErrorKind::InvalidBuiltIn,
            diagnostics,
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "installed Built-in Source Profiles are invalid ({} diagnostics)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for LoadError {}

#[derive(Clone, Debug)]
pub(crate) struct AdmittedProfile {
    pub document: SourceProfileDocument,
}

/// One immutable operation-local admitted Profile set.
///
/// Loading performs local blocking filesystem work bounded by the exported
/// limits. Callers should create it outside async executor worker threads.
#[derive(Clone, Debug)]
pub struct Profiles {
    pub(crate) admitted: Vec<AdmittedProfile>,
    pub(crate) prepared_detection: Vec<CompiledDetectionPlan>,
    pub(crate) view: ProfilesView,
}

impl Profiles {
    pub fn load(app_data_dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        profiles::load(app_data_dir.as_ref())
    }

    pub fn view(&self) -> &ProfilesView {
        &self.view
    }

    pub(crate) fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.admitted
            .iter()
            .find(|profile| profile.document.key == key)
            .map(|profile| &profile.document)
    }

    pub(crate) fn lookup(&self) -> ProfileLookup<'_> {
        ProfileLookup(self)
    }
}

pub(crate) struct ProfileLookup<'a>(&'a Profiles);

impl SourceProfileLookup for ProfileLookup<'_> {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.0.profile(key)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStateKind {
    Unknown,
    Valid,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationState {
    pub source_key: String,
    pub state: ValidationStateKind,
    pub can_compile: bool,
    pub can_execute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBehaviorView {
    pub access_path_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_source_config_schema: Option<JsonSchemaObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_path_source_config_schema: Option<JsonSchemaObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support: Option<SupportMetadata>,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub origin: Origin,
    pub file_name: String,
    pub document: SourceDocument,
    pub validation_state: ValidationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedBehaviorView>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub profiles: ProfilesView,
    pub sources: Vec<SourceView>,
    pub diagnostics: Diagnostics,
}

/// Opaque identity of the complete Source and selected admitted Profile
/// behavior used by one exact preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Generation(pub(crate) String);

#[derive(Clone, Debug)]
pub struct PreparedSource {
    pub(crate) origin: Origin,
    pub(crate) file_name: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) document: SourceDocument,
    pub(crate) validation: ValidationState,
    pub(crate) outcome: CompileSourceOutcome,
    pub(crate) generation: Generation,
    pub(crate) resolved: Option<ResolvedBehaviorView>,
}

impl PreparedSource {
    pub fn origin(&self) -> Origin {
        self.origin
    }
    pub fn document(&self) -> &SourceDocument {
        &self.document
    }
    pub fn validation(&self) -> &ValidationState {
        &self.validation
    }
    pub fn generation(&self) -> &Generation {
        &self.generation
    }
    pub fn compiled(&self) -> Option<&CompiledSource> {
        match &self.outcome {
            CompileSourceOutcome::Compiled {
                source,
                diagnostics,
            } if !diagnostics.iter().any(|item| {
                item.severity == source_profile_dsl::definition::DiagnosticSeverity::Error
            }) =>
            {
                Some(source)
            }
            _ => None,
        }
    }
    /// Temporary exact preparation material for Desktop Live Check; removed when
    /// check ownership moves into this crate in #319.
    #[doc(hidden)]
    pub fn compiler_outcome(&self) -> &CompileSourceOutcome {
        &self.outcome
    }
    pub fn preparation_diagnostics(&self) -> &Diagnostics {
        match &self.outcome {
            CompileSourceOutcome::Compiled { diagnostics, .. }
            | CompileSourceOutcome::Rejected { diagnostics } => diagnostics,
        }
    }
}

/// Immutable operation-local installed Profile/Source state. It intentionally
/// has no serialization implementation; hosts receive [`SnapshotView`].
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub(crate) profiles: Profiles,
    pub(crate) sources: Vec<PreparedSource>,
    pub(crate) view: SnapshotView,
}

impl Snapshot {
    pub fn load(app_data_dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        let profiles = Profiles::load(app_data_dir.as_ref())?;
        Ok(loading::load(app_data_dir.as_ref(), profiles))
    }
    pub fn view(&self) -> &SnapshotView {
        &self.view
    }
    pub fn source(&self, key: &str) -> Option<&PreparedSource> {
        self.sources
            .iter()
            .find(|source| source.document.key == key)
    }
    /// Temporary exact Profile material for Desktop Live Check; removed when
    /// check ownership moves into this crate in #319.
    #[doc(hidden)]
    pub fn profile_for_live_check(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.profiles.profile(key)
    }
}
