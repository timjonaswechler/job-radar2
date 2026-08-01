use serde::{Deserialize, Serialize};

use super::{
    AccessPathFragment, DetailStep, DiscoveryStep, JsonObject, JsonSchemaObject, SupportMetadata,
};

pub type SourceConfig = JsonObject;

/// Lifecycle-free Source behavior accepted by the Profile Compiler.
///
/// Persisted schema version, lifecycle status, and authored diagnostics are
/// deliberately owned by the installed Source module and never enter this
/// interface.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceBehavior {
    pub key: String,
    pub name: String,
    pub source_config: SourceConfig,
    pub selected_access_path: SelectedAccessPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_paths: Option<Vec<AccessPathFragment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_support: Option<SupportMetadata>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SelectedAccessPath {
    ProfileAccessPath {
        #[serde(rename = "profileKey")]
        profile_key: String,
        #[serde(rename = "pathKey")]
        path_key: String,
    },
    SourceOwnedAccessPath {
        key: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "sourceConfigSchema", skip_serializing_if = "Option::is_none")]
        source_config_schema: Option<JsonSchemaObject>,
        discovery: DiscoveryStep,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<DetailStep>,
        #[serde(skip_serializing_if = "Option::is_none")]
        diagnostics: Option<crate::definition::Diagnostics>,
    },
}
