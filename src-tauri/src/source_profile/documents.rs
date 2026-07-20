use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    DetectionDocument, JsonSchemaObject, ReusableAccessPathDocument, SupportMetadata,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProfileDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub kind: SourceProfileKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub support: SupportMetadata,
    #[serde(rename = "detect", skip_serializing_if = "Option::is_none")]
    pub detection: Option<DetectionDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<JsonSchemaObject>,
    pub access_paths: Vec<ReusableAccessPathDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProfileKind {
    RecruitingSystem,
    JobPortal,
    WebsiteFamily,
    CareerSite,
    Generic,
}
