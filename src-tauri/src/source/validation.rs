use serde::{Deserialize, Serialize};

use crate::profile_dsl::diagnostics::Diagnostics;

/// Derived Source validation state. This is prepared for compiler/registry
/// integration and must not be persisted as `SourceStatus::Invalid`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceValidationState {
    pub source_key: String,
    pub state: ValidationStateKind,
    pub can_compile: bool,
    pub can_execute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStateKind {
    Unknown,
    Valid,
    Invalid,
}
