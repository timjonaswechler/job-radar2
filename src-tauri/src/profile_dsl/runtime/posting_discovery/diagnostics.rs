use super::*;

pub(super) fn runtime_error(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    runtime_diagnostic(
        code,
        message,
        DiagnosticSeverity::Error,
        path,
        strategy_key,
        details,
    )
}

pub(super) fn runtime_warning(
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    runtime_diagnostic(
        code,
        message,
        DiagnosticSeverity::Warning,
        path,
        strategy_key,
        details,
    )
}

fn runtime_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    severity: DiagnosticSeverity,
    path: impl Into<String>,
    strategy_key: Option<&str>,
    details: Value,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.into(),
        message: message.into(),
        severity,
        path: path.into(),
        strategy_key: strategy_key.map(ToString::to_string),
        details: Some(details),
    }
}
