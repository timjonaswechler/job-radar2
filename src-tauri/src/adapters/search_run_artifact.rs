use std::path::{Path, PathBuf};

use search_runs::Outcome;
use source_engine::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

pub(crate) fn default_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap_or(manifest_dir.as_path())
        .join("search-run-result.json")
}

pub(crate) async fn write(path: &Path, outcome: &mut Outcome) {
    if write_bounded(path, outcome).await.is_err() {
        outcome.diagnostics.push(Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "search_run_result_artifact_write_failed".to_string(),
            message: "Search Run committed successfully, but its non-authoritative result artifact could not be written".to_string(),
            severity: DiagnosticSeverity::Warning,
            path: "/artifact".to_string(),
            strategy_key: None,
            details: None,
        });
    }
}

async fn write_bounded(path: &Path, outcome: &Outcome) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| error.to_string())?;
        }
    }
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Summary<'a> {
        search_request_id: i64,
        status: search_runs::Status,
        generated_at: &'a str,
        source_runs: &'a [search_runs::SourceOutcome],
        posting_count: usize,
    }
    let summary = Summary {
        search_request_id: outcome.search_request_id,
        status: outcome.status,
        generated_at: &outcome.generated_at,
        source_runs: &outcome.source_runs,
        posting_count: outcome.matched_posting_count,
    };
    let json = serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())?;
    tokio::fs::write(path, json)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome() -> Outcome {
        Outcome {
            search_request_id: 7,
            status: search_runs::Status::Completed,
            generated_at: "2026-01-01T00:00:00Z".into(),
            diagnostics: Vec::new(),
            source_runs: Vec::new(),
            matched_posting_count: 0,
        }
    }

    #[test]
    fn artifact_failure_adds_warning_without_changing_domain_status() {
        tauri::async_runtime::block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let parent_file = temp.path().join("not-a-directory");
            std::fs::write(&parent_file, "occupied").unwrap();
            let mut outcome = outcome();

            write(&parent_file.join("result.json"), &mut outcome).await;

            assert_eq!(outcome.status, search_runs::Status::Completed);
            assert_eq!(
                outcome.diagnostics[0].code,
                "search_run_result_artifact_write_failed"
            );
        });
    }

    #[test]
    fn artifact_is_a_bounded_projection_without_postings() {
        tauri::async_runtime::block_on(async {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("result.json");
            let mut outcome = outcome();

            write(&path, &mut outcome).await;

            let value: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(value["searchRequestId"], 7);
            assert!(value.get("postings").is_none());
            assert!(outcome.diagnostics.is_empty());
        });
    }
}
