use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCandidate {
    pub title: String,
    pub company: String,
    pub url: String,
    pub locations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRunStatus {
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRunStatus {
    Completed,
    Failed,
    Cancelled,
}

/// Current-result Suchlauf written to `search-run-result.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRunResult {
    pub search_request_id: i64,
    pub status: SearchRunStatus,
    pub generated_at: String,
    pub source_runs: Vec<SourceRunResult>,
    pub postings: Vec<NormalizedPosting>,
}

/// Quellenlauf outcome for one selected Quelle.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRunResult {
    pub source_key: String,
    pub source_name: String,
    pub status: SourceRunStatus,
    pub candidate_count: usize,
    pub matched_count: usize,
    pub error: Option<String>,
}

/// Normalized Stellenanzeige after Trefferregel/Ausschlussregel filtering and dedupe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedPosting {
    pub title: String,
    pub company: String,
    pub url: String,
    pub locations: Vec<String>,
    pub sources: Vec<PostingSource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostingSource {
    pub source_key: String,
    pub source_name: String,
    pub url: String,
}
