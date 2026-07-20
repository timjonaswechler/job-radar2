use super::*;

pub(super) fn runtime_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(details),
    }
}
