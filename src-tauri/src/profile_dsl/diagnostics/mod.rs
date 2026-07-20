use serde::{Deserialize, Serialize};

use serde_json::Value;

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
    /// Profile Compiler / Execution Plan compilation diagnostics emitted while
    /// compiling a concrete Source and its selected Source Profile, Access
    /// Path, Source Config, and Direct Source Specialization. This is not a Rust compiler
    /// diagnostic category.
    Compiler,
    Runtime,
    Detection,
    SourceValidation,
}

/// Shared machine-readable diagnostic contract for schema validation,
/// registry loading, Profile Compiler validation, source validation,
/// detection, live checks, and runtime execution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub category: DiagnosticCategory,
    pub code: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    /// JSON Pointer into the diagnosed document. The document root is the empty
    /// JSON Pointer (`""`).
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy_key: Option<String>,
    /// Optional machine-readable context for consumers such as UI surfaces,
    /// Tauri commands, tests, and agent feedback loops.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Diagnostic {
    pub fn duplicate_builtin_custom_source_profile_key(key: impl Into<String>) -> Self {
        let key = key.into();

        Self {
            category: DiagnosticCategory::Registry,
            code: "duplicate_source_profile_key".to_string(),
            message: format!(
                "Custom Source Profile key `{key}` duplicates a built-in Source Profile key"
            ),
            severity: DiagnosticSeverity::Error,
            path: "/key".to_string(),
            strategy_key: None,
            details: Some(serde_json::json!({
                "sourceProfileKey": key,
                "existingOrigin": "built_in",
                "duplicateOrigin": "custom"
            })),
        }
    }
}

pub type Diagnostics = Vec<Diagnostic>;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

    #[test]
    fn structured_diagnostic_serializes_for_ui_and_agent_consumers() {
        let diagnostic = Diagnostic {
            category: DiagnosticCategory::Compiler,
            code: "missing_template_variable".to_string(),
            message: "Source Config is missing required template variable tenant".to_string(),
            severity: DiagnosticSeverity::Error,
            path: "/sourceConfig/tenant".to_string(),
            strategy_key: Some("json_api".to_string()),
            details: Some(json!({
                "missingVariable": "tenant",
                "requiredBy": "fetch.url"
            })),
        };

        assert_eq!(
            serde_json::to_value(diagnostic).unwrap(),
            json!({
                "category": "compiler",
                "code": "missing_template_variable",
                "message": "Source Config is missing required template variable tenant",
                "severity": "error",
                "path": "/sourceConfig/tenant",
                "strategyKey": "json_api",
                "details": {
                    "missingVariable": "tenant",
                    "requiredBy": "fetch.url"
                }
            })
        );
    }

    #[test]
    fn duplicate_builtin_custom_source_profile_key_diagnostic_has_structured_details() {
        let diagnostic = Diagnostic::duplicate_builtin_custom_source_profile_key("greenhouse");

        assert_eq!(diagnostic.category, DiagnosticCategory::Registry);
        assert_eq!(diagnostic.code, "duplicate_source_profile_key");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.path, "/key");
        assert_eq!(diagnostic.strategy_key, None);
        assert_eq!(
            diagnostic.details,
            Some(json!({
                "sourceProfileKey": "greenhouse",
                "existingOrigin": "built_in",
                "duplicateOrigin": "custom"
            }))
        );
    }
}
