use serde::{Deserialize, Serialize};
use source_profile_dsl::definition::{
    AccessPathFragment, Diagnostics, JsonObject, SelectedAccessPath, SourceBehavior,
    SupportMetadata,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Draft,
    Active,
    Disabled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDocument {
    #[serde(deserialize_with = "deserialize_schema_version_3")]
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub status: SourceStatus,
    pub source_config: JsonObject,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_support: Option<SupportMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

impl SourceDocument {
    /// Projects the persisted document into the lifecycle-free compiler input.
    pub(crate) fn behavior_input(&self) -> SourceBehavior {
        SourceBehavior {
            key: self.key.clone(),
            name: self.name.clone(),
            source_config: self.source_config.clone(),
            selected_access_path: self.selected_access_path.clone(),
            access_paths: self.access_paths.clone(),
            source_support: self.source_support.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthoredViolation {
    pub code: &'static str,
    pub message: String,
    pub path: &'static str,
}

pub(crate) fn authored_violations(document: &SourceDocument) -> Vec<AuthoredViolation> {
    let mut violations = Vec::new();
    if !is_key(&document.key) {
        violations.push(AuthoredViolation {
            code: "invalid_document_key",
            message: format!("Source key `{}` is invalid", document.key),
            path: "/key",
        });
    }
    if document.name.is_empty() {
        violations.push(AuthoredViolation {
            code: "invalid_source_name",
            message: "Source name must not be empty".to_string(),
            path: "/name",
        });
    }
    match &document.selected_access_path {
        SelectedAccessPath::ProfileAccessPath {
            profile_key,
            path_key,
        } => {
            if !is_key(profile_key) {
                violations.push(AuthoredViolation {
                    code: "invalid_selected_profile_key",
                    message: format!("selected Source Profile key `{profile_key}` is invalid"),
                    path: "/selectedAccessPath/profileKey",
                });
            }
            if !is_key(path_key) {
                violations.push(AuthoredViolation {
                    code: "invalid_selected_access_path_key",
                    message: format!("selected Access Path key `{path_key}` is invalid"),
                    path: "/selectedAccessPath/pathKey",
                });
            }
            if document.source_support.is_some() {
                violations.push(AuthoredViolation {
                    code: "profile_selected_source_support_forbidden",
                    message: "Profile-selected Sources cannot author sourceSupport".to_string(),
                    path: "/sourceSupport",
                });
            }
        }
        SelectedAccessPath::SourceOwnedAccessPath { key, name, .. } => {
            if !is_key(key) {
                violations.push(AuthoredViolation {
                    code: "invalid_source_owned_access_path_key",
                    message: format!("Source-owned Access Path key `{key}` is invalid"),
                    path: "/selectedAccessPath/key",
                });
            }
            if name.is_empty() {
                violations.push(AuthoredViolation {
                    code: "invalid_source_owned_access_path_name",
                    message: "Source-owned Access Path name must not be empty".to_string(),
                    path: "/selectedAccessPath/name",
                });
            }
            if document.source_support.is_none() {
                violations.push(AuthoredViolation {
                    code: "source_owned_support_required",
                    message: "Source-owned Access Paths require sourceSupport".to_string(),
                    path: "/sourceSupport",
                });
            }
            if document.access_paths.is_some() {
                violations.push(AuthoredViolation {
                    code: "source_owned_direct_access_paths_forbidden",
                    message: "Source-owned Access Paths cannot also author direct accessPaths"
                        .to_string(),
                    path: "/accessPaths",
                });
            }
        }
    }
    violations
}

pub(crate) fn is_key(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn deserialize_schema_version_3<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    let version = u64::deserialize(deserializer)?;
    if version == 3 {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 3"))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InactiveStatus {
    Draft,
    Disabled,
}

impl From<InactiveStatus> for SourceStatus {
    fn from(value: InactiveStatus) -> Self {
        match value {
            InactiveStatus::Draft => Self::Draft,
            InactiveStatus::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateDraft {
    pub key: String,
    pub name: String,
    pub source_config: JsonObject,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default)]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(default)]
    pub source_support: Option<SupportMetadata>,
}

impl CreateDraft {
    pub(crate) fn into_document(self) -> SourceDocument {
        SourceDocument {
            schema_version: 3,
            key: self.key,
            name: self.name,
            status: SourceStatus::Draft,
            source_config: self.source_config,
            selected_access_path: self.selected_access_path,
            access_paths: self.access_paths,
            source_support: self.source_support,
            diagnostics: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Revision {
    pub key: String,
    pub name: String,
    pub source_config: JsonObject,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default)]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(default)]
    pub source_support: Option<SupportMetadata>,
}

impl Revision {
    pub(crate) fn into_document(self, status: SourceStatus) -> SourceDocument {
        SourceDocument {
            schema_version: 3,
            key: self.key,
            name: self.name,
            status,
            source_config: self.source_config,
            selected_access_path: self.selected_access_path,
            access_paths: self.access_paths,
            source_support: self.source_support,
            diagnostics: None,
        }
    }
}
