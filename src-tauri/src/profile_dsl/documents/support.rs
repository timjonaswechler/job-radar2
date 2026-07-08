use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Untyped JSON object used by document-layer shapes whose schema is supplied
/// by the selected Source Profile and Access Path.
pub type JsonObject = Map<String, Value>;

/// Untyped JSON Schema object at the document layer.
pub type JsonSchemaObject = Map<String, Value>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Stable,
    BestEffort,
    Experimental,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportMetadata {
    pub level: SupportLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_issues: Option<Vec<SupportNote>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Vec<SupportEvidence>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportNote {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportEvidenceKind {
    Smoke,
    ManualReview,
    SchemaCheck,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportEvidence {
    pub kind: SupportEvidenceKind,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}
