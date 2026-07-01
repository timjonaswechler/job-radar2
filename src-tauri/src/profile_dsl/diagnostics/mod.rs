use serde::{Deserialize, Serialize};

use crate::profile_dsl::documents::JsonObject;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    Schema,
    Registry,
    Compiler,
    Runtime,
    SourceValidation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<DiagnosticCategory>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<JsonObject>,
}

pub type Diagnostics = Vec<Diagnostic>;
