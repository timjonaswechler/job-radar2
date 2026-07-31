use std::{fmt, path::Path};

use serde::Serialize;
use source_profile_dsl::{
    definition::{
        DetectionDocument, Diagnostic, Diagnostics, JsonSchemaObject, ReusableAccessPathDocument,
        SourceProfileDocument, SourceProfileKind, SourceProfileLookup, SupportMetadata,
    },
    detection::CompiledDetectionPlan,
};

use super::profiles;

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

    /// Temporary prepared capability for Desktop Detection; internalized by #318.
    pub fn prepared_detection(&self) -> &[CompiledDetectionPlan] {
        &self.prepared_detection
    }

    pub fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        SourceProfileLookup::profile(self, key)
    }
}

impl SourceProfileLookup for Profiles {
    fn profile(&self, key: &str) -> Option<&SourceProfileDocument> {
        self.admitted
            .iter()
            .find(|profile| profile.document.key == key)
            .map(|profile| &profile.document)
    }
}
