use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;
use crate::profile_dsl::documents::{
    BrowserWait, JsonObject, JsonSchemaObject, ReusableAccessPathDocument, SupportMetadata,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect: Option<ProfileDetectionDocument>,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDetectionDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_url_patterns: Option<Vec<InputUrlPattern>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_access_path_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config: Option<JsonObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_candidates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_candidates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_checks: Option<Vec<DetectionHttpCheck>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_probes: Option<Vec<DetectionBrowserProbe>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Vec<DetectionEvidence>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputUrlPattern {
    pub pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captures: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionEvidence {
    pub kind: DetectionEvidenceKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionHttpCheck {
    pub key: String,
    pub url: String,
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(rename = "expectStatus", skip_serializing_if = "Option::is_none")]
    pub expect_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetectionBrowserProbe {
    pub key: String,
    pub url: String,
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waits: Option<Vec<BrowserWait>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactions: Option<Vec<DetectionBrowserInteraction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_contains: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_regex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DetectionBrowserInteraction {
    ClickIfVisible {
        selector: String,
        #[serde(rename = "maxCount", skip_serializing_if = "Option::is_none")]
        max_count: Option<u64>,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
    ClickUntilGone {
        selector: String,
        #[serde(rename = "maxCount", skip_serializing_if = "Option::is_none")]
        max_count: Option<u64>,
        #[serde(rename = "waitAfterMs", skip_serializing_if = "Option::is_none")]
        wait_after_ms: Option<u64>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionEvidenceKind {
    Url,
    Http,
    Html,
    Browser,
}
