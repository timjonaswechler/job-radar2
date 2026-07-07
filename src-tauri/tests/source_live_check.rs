use std::{collections::BTreeMap, fs, future::Future, path::Path, pin::Pin, sync::Mutex};

use job_radar_lib::{
    check_source, check_source_with_fetcher, read_latest_check_report,
    source_live_check_report_path, CheckReportKind, CheckReportResult, CheckReportSubjectType,
    DiagnosticCategory, DiagnosticSeverity, PostingDiscoveryFetchError,
    PostingDiscoveryFetchRequest, PostingDiscoveryFetchResponse, PostingDiscoveryFetcher,
    SOURCE_LIVE_CHECK_LOGIC_VERSION,
};
use serde_json::json;

const SIMPLE_PROFILE: &str =
    include_str!("fixtures/source-profile-dsl/valid/simple-source-profile.json");
const SIMPLE_SOURCE: &str =
    include_str!("fixtures/source-profile-dsl/valid/source-selecting-access-path.json");

fn write_profile(app_data_dir: &Path, profile: &serde_json::Value) {
    let profile_dir = app_data_dir.join("source-profiles");
    fs::create_dir_all(&profile_dir).unwrap();
    let key = profile["key"].as_str().unwrap();
    fs::write(
        profile_dir.join(format!("{key}.json")),
        serde_json::to_string_pretty(profile).unwrap(),
    )
    .unwrap();
}

fn write_source(app_data_dir: &Path, source: &serde_json::Value) {
    let source_dir = app_data_dir.join("sources");
    fs::create_dir_all(&source_dir).unwrap();
    let key = source["key"].as_str().unwrap();
    fs::write(
        source_dir.join(format!("{key}.json")),
        serde_json::to_string_pretty(source).unwrap(),
    )
    .unwrap();
}

fn simple_profile() -> serde_json::Value {
    serde_json::from_str(SIMPLE_PROFILE).unwrap()
}

fn simple_profile_without_pagination() -> serde_json::Value {
    let mut profile = simple_profile();
    profile["accessPaths"][0]["postingDiscovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    profile
}

fn simple_source_with_status(status: &str) -> serde_json::Value {
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!(status);
    source
}

#[test]
fn check_source_creates_and_persists_passed_report_for_valid_draft_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("draft");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeDiscoveryFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                {
                    "id": "job-1",
                    "title": " Senior Rust Engineer ",
                    "url": "https://example.test/jobs/job-1",
                    "locations": ["Remote"]
                }
            ]
        })
        .to_string(),
    )]);

    let report = check_source_with_fetcher(temp_dir.path(), "example_source", &fetcher).unwrap();

    assert_eq!(report.kind, CheckReportKind::SourceLiveCheck);
    assert_eq!(report.subject.subject_type, CheckReportSubjectType::Source);
    assert_eq!(report.subject.key, "example_source");
    assert_eq!(report.logic_version, SOURCE_LIVE_CHECK_LOGIC_VERSION);
    assert_eq!(report.result, CheckReportResult::Passed);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.details["sourceStatusAtCheck"], json!("draft"));
    assert_eq!(report.details["liveCheckState"], json!("live_check_passed"));
    assert_eq!(report.details["accessPathKey"], json!("json_feed"));
    assert_eq!(report.details["candidateCount"], json!(1));
    assert_eq!(report.details["detailChecked"], json!(false));
    assert_eq!(report.details["detailPassed"], serde_json::Value::Null);

    for expected_kind in [
        "live_check_logic",
        "source_document",
        "source_profile_document",
        "source_config",
        "source_overrides",
    ] {
        assert!(
            report
                .fingerprints
                .iter()
                .any(|fingerprint| fingerprint.kind == expected_kind),
            "missing fingerprint kind {expected_kind}: {:?}",
            report.fingerprints
        );
    }
    assert_eq!(
        fetcher.requested_urls(),
        vec!["https://example.test/jobs.json"]
    );

    let persisted_path = source_live_check_report_path(temp_dir.path(), "example_source");
    let persisted = read_latest_check_report(&persisted_path).unwrap();
    assert_eq!(persisted, report);

    let stored_source: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(temp_dir.path().join("sources/example_source.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stored_source["status"], json!("draft"));
}

#[test]
fn check_source_persists_failed_report_for_unknown_source_key() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile());

    let report = check_source(temp_dir.path(), "missing_source").unwrap();

    assert_eq!(report.kind, CheckReportKind::SourceLiveCheck);
    assert_eq!(report.subject.subject_type, CheckReportSubjectType::Source);
    assert_eq!(report.subject.key, "missing_source");
    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(report.details["liveCheckState"], json!("live_check_failed"));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::SourceValidation
            && diagnostic.code == "source_not_found"
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic
                .details
                .as_ref()
                .and_then(serde_json::Value::as_object)
                .and_then(|details| details.get("sourceKey"))
                == Some(&json!("missing_source"))
    }));

    let persisted_path = source_live_check_report_path(temp_dir.path(), "missing_source");
    assert_eq!(read_latest_check_report(&persisted_path).unwrap(), report);
}

#[test]
fn check_source_includes_compiler_and_source_validation_diagnostics_for_invalid_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut source = simple_source_with_status("active");
    source["sourceConfig"] = json!({ "language": "en" });
    write_profile(temp_dir.path(), &simple_profile());
    write_source(temp_dir.path(), &source);

    let report = check_source(temp_dir.path(), "example_source").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(report.details["sourceStatusAtCheck"], json!("active"));
    assert_eq!(report.details["liveCheckState"], json!("live_check_failed"));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Compiler
            && diagnostic.code == "missing_source_config_required_property"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::SourceValidation
            && diagnostic.code == "source_validation_failed"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
}

#[test]
fn check_source_emits_no_candidates_diagnostic_for_empty_live_discovery() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeDiscoveryFetcher::new([(
        "https://example.test/jobs.json",
        json!({ "jobs": [] }).to_string(),
    )]);

    let report = check_source_with_fetcher(temp_dir.path(), "example_source", &fetcher).unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(report.details["liveCheckState"], json!("live_check_failed"));
    assert_eq!(report.details["candidateCount"], json!(0));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Runtime
            && diagnostic.code == "source_live_check.no_candidates"
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.details.as_ref()
                == Some(&json!({
                    "sourceKey": "example_source",
                    "profileKey": "example_jobs",
                    "accessPathKey": "json_feed",
                    "candidateCount": 0,
                    "acceptableCandidateCount": 0,
                    "requiredFields": ["title", "company", "url"]
                }))
    }));
}

#[test]
fn check_source_preserves_runtime_diagnostics_from_failed_live_discovery() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher =
        FakeDiscoveryFetcher::new([("https://example.test/jobs.json", "not json".to_string())]);

    let report = check_source_with_fetcher(temp_dir.path(), "example_source", &fetcher).unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Runtime
            && diagnostic.code == "json_parse_failed"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
}

#[test]
fn check_source_does_not_need_search_request_or_match_rule_context() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeDiscoveryFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                {
                    "id": "job-1",
                    "title": "Unrelated title that no Search Request criteria selected",
                    "url": "https://example.test/jobs/job-1"
                }
            ]
        })
        .to_string(),
    )]);

    let report = check_source_with_fetcher(temp_dir.path(), "example_source", &fetcher).unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(report.details["candidateCount"], json!(1));
}

struct FakeDiscoveryFetcher {
    responses: BTreeMap<String, String>,
    requests: Mutex<Vec<PostingDiscoveryFetchRequest>>,
}

impl FakeDiscoveryFetcher {
    fn new<'a>(responses: impl IntoIterator<Item = (&'a str, String)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(url, body)| (url.to_string(), body))
                .collect(),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }
}

impl PostingDiscoveryFetcher for FakeDiscoveryFetcher {
    fn fetch<'a>(
        &'a self,
        request: PostingDiscoveryFetchRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PostingDiscoveryFetchResponse, PostingDiscoveryFetchError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let body = self.responses.get(&request.url).cloned().ok_or_else(|| {
                PostingDiscoveryFetchError::new(format!(
                    "missing fake response for {}",
                    request.url
                ))
            })?;
            self.requests.lock().unwrap().push(request);
            Ok(PostingDiscoveryFetchResponse { body })
        })
    }
}
