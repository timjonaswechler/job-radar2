use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDocumentOrigin {
    BuiltIn,
    Custom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDocumentKind {
    SourceProfile,
    Source,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRegistryDiagnosticCode {
    InvalidJson,
    InvalidShape,
    FilenameKeyMismatch,
    DuplicateKey,
    MissingProfileRef,
    MissingPathRef,
    ReadError,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRegistryDiagnostic {
    pub code: SourceRegistryDiagnosticCode,
    pub document_kind: SourceRegistryDocumentKind,
    pub origin: SourceRegistryDocumentOrigin,
    pub path: String,
    pub key: Option<String>,
    pub message: String,
}
