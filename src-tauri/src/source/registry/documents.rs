//! Legacy v1 Source/Profile registry document model.
//!
//! Kept temporarily while the declarative Source Profile DSL hard cut is
//! introduced. New code should use `crate::source_profile::documents`,
//! `crate::source::documents`, and `crate::profile_dsl::documents`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProfileKind {
    RecruitingSystem,
    JobPortal,
    WebsiteFamily,
    Generic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionPhase {
    Http,
    Browser,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<DetectionPhase>,
    pub required: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<Vec<Value>>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProfileIdentity {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name_candidates: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_source_config: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailabilityBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_captures: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BrowserInteraction {
    #[serde(rename = "waitFor")]
    WaitFor {
        selector: String,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    #[serde(rename = "clickIfVisible")]
    ClickIfVisible {
        selector: String,
        #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    #[serde(rename = "clickUpToN")]
    ClickUpToN {
        selector: String,
        #[serde(rename = "maxClicks")]
        max_clicks: u64,
        #[serde(rename = "waitAfterClickMs", skip_serializing_if = "Option::is_none")]
        wait_after_click_ms: Option<u64>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileAccessPathDefinition {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub adapter_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<AvailabilityBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_detail: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Vec<BrowserInteraction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_release: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProfileDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub kind: SourceProfileKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect: Option<DetectionBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<SourceProfileIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config_schema: Option<Value>,
    pub access_paths: Vec<ProfileAccessPathDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentStatus {
    Draft,
    Active,
    Disabled,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectedAccessPath {
    #[serde(rename_all = "camelCase")]
    Profile {
        profile_key: String,
        path_key: String,
    },
    #[serde(rename_all = "camelCase")]
    SourceSpecific {
        adapter_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_config_schema: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inventory: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        interactions: Option<Vec<BrowserInteraction>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        manual_release: Option<Value>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDocument {
    pub schema_version: u64,
    pub key: String,
    pub name: String,
    pub status: SourceDocumentStatus,
    pub source_config: Value,
    pub selected_access_path: SelectedAccessPath,
}
