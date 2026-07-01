use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    JsonObject, JsonSchemaObject, PostingDetailStep, PostingDiscoveryStep, SourceOverrides,
    SupportMetadata,
};

pub type SourceConfig = JsonObject;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub status: SourceStatus,
    pub source_config: SourceConfig,
    pub selected_access_path: SelectedAccessPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_overrides: Option<SourceOverrides>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_support: Option<SupportMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Draft,
    Active,
    Disabled,
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
    /// Inline Access Path owned by this Source. It reuses shared Profile DSL
    /// steps but is not a reusable Source Profile Access Path.
    SourceOwnedAccessPath {
        key: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "sourceConfigSchema", skip_serializing_if = "Option::is_none")]
        source_config_schema: Option<JsonSchemaObject>,
        #[serde(rename = "postingDiscovery")]
        posting_discovery: PostingDiscoveryStep,
        #[serde(rename = "postingDetail", skip_serializing_if = "Option::is_none")]
        posting_detail: Option<PostingDetailStep>,
        #[serde(skip_serializing_if = "Option::is_none")]
        diagnostics: Option<Diagnostics>,
    },
}
