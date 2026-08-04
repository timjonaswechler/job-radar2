use crate::catalog::Source;
use source_engine::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics};

const MAX_OPEN_DIAGNOSTICS: usize = 256;

pub(super) fn append(target: &mut Diagnostics, incoming: Diagnostics) {
    let remaining = MAX_OPEN_DIAGNOSTICS.saturating_sub(target.len());
    target.extend(incoming.into_iter().take(remaining));
}

pub(super) fn push(target: &mut Diagnostics, diagnostic: Diagnostic) {
    if target.len() < MAX_OPEN_DIAGNOSTICS {
        target.push(diagnostic);
    }
}

pub(super) fn contextualize(diagnostics: Diagnostics, source: &Source) -> Diagnostics {
    diagnostics
        .into_iter()
        .map(|diagnostic| contextualize_one(diagnostic, source))
        .collect()
}

pub(super) fn source(
    source: &Source,
    code: impl Into<String>,
    message: impl Into<String>,
    path: impl Into<String>,
    details: serde_json::Value,
) -> Diagnostic {
    contextualize_one(
        Diagnostic {
            category: DiagnosticCategory::SourceValidation,
            code: code.into(),
            message: message.into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            strategy_key: None,
            details: Some(details),
        },
        source,
    )
}

pub(super) fn posting(
    posting_id: i64,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::SourceValidation,
        code: code.into(),
        message: message.into(),
        severity: DiagnosticSeverity::Error,
        path: String::new(),
        strategy_key: None,
        details: Some(serde_json::json!({ "postingId": posting_id })),
    }
}

pub(super) fn summary(diagnostics: &Diagnostics, fallback: &str) -> String {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .or_else(|| diagnostics.first())
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn contextualize_one(mut diagnostic: Diagnostic, source: &Source) -> Diagnostic {
    let original_details = diagnostic.details.take();
    let mut details = original_details
        .as_ref()
        .and_then(|details| details.as_object().cloned())
        .unwrap_or_default();
    if details.is_empty() {
        if let Some(original_details) = original_details.filter(|details| !details.is_object()) {
            details.insert("originalDetails".to_string(), original_details);
        }
    }
    details.insert("postingSourceId".to_string(), serde_json::json!(source.id));
    details.insert(
        "postingSourceKey".to_string(),
        serde_json::json!(source.source_key),
    );
    details.insert("postingUrl".to_string(), serde_json::json!(source.url));
    diagnostic.details = Some(serde_json::Value::Object(details));
    diagnostic
}
