use std::{fs, future::Future, path::Path, sync::Arc};

use crate::job_radar_lib::{
    CheckReport, CheckReportFreshnessState, CheckReportKind, CheckReportResult,
    CheckReportStaleReason, CheckReportSubjectType, DiagnosticCategory, DiagnosticSeverity,
    InstalledSourceStore, OperationContext, ProfileHttpRequest, Revision, RuntimeCancellation,
    ScriptedBrowserAcquisition, ScriptedHttpBodyEvent, ScriptedHttpEvent,
    ScriptedProfileHttpClient, SourceDocument, SourceLiveCheckOutcome, SourceLiveCheckReportState,
    SourceLiveCheckRequest, SourceOnboarding, SourceOnboardingError, SourceOnboardingErrorKind,
    SourceStatus, SourceView, SOURCE_LIVE_CHECK_LOGIC_VERSION,
};
use serde_json::json;

const SIMPLE_PROFILE: &str =
    include_str!("../fixtures/source-behavior/valid/simple-source-profile.json");
const SIMPLE_SOURCE: &str = include_str!(
    "../../crates/sources/tests/fixtures/sources/valid/source-selecting-access-path.json"
);

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

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("Source Live Check test runtime should build")
        .block_on(future)
}

fn onboarding(
    app_data_dir: impl AsRef<Path>,
    client: Arc<ScriptedProfileHttpClient>,
) -> SourceOnboarding {
    SourceOnboarding::new(
        app_data_dir.as_ref(),
        client,
        Arc::new(ScriptedBrowserAcquisition::new([])),
    )
}

fn run_checked(
    app_data_dir: impl AsRef<Path>,
    key: &str,
    client: Arc<ScriptedProfileHttpClient>,
) -> Result<(CheckReport, SourceView), SourceOnboardingError> {
    match block_on(onboarding(app_data_dir, client).live_check(
        SourceLiveCheckRequest::Run {
            source_key: key.to_string(),
        },
        OperationContext::default(),
    ))? {
        SourceLiveCheckOutcome::Checked { report, source } => Ok((report, source)),
        outcome => panic!("unexpected status-neutral outcome: {outcome:?}"),
    }
}

fn run_check(
    app_data_dir: impl AsRef<Path>,
    key: &str,
    client: Arc<ScriptedProfileHttpClient>,
) -> Result<CheckReport, SourceOnboardingError> {
    run_checked(app_data_dir, key, client).map(|(report, _)| report)
}

fn run_activation(
    app_data_dir: impl AsRef<Path>,
    key: &str,
    client: Arc<ScriptedProfileHttpClient>,
) -> Result<SourceLiveCheckOutcome, SourceOnboardingError> {
    block_on(onboarding(app_data_dir, client).live_check(
        SourceLiveCheckRequest::CheckAndActivate {
            source_key: key.to_string(),
        },
        OperationContext::default(),
    ))
}

fn latest_status(
    app_data_dir: impl AsRef<Path>,
    key: &str,
) -> Result<crate::job_radar_lib::SourceLiveCheckReportStatus, SourceOnboardingError> {
    match block_on(
        onboarding(app_data_dir, Arc::new(ScriptedProfileHttpClient::new([]))).live_check(
            SourceLiveCheckRequest::LatestReportStatus {
                source_key: key.to_string(),
            },
            OperationContext::default(),
        ),
    )? {
        SourceLiveCheckOutcome::LatestReportStatus(status) => Ok(status),
        outcome => panic!("unexpected latest-status outcome: {outcome:?}"),
    }
}

fn run_check_without_io(
    app_data_dir: impl AsRef<Path>,
    key: &str,
) -> Result<CheckReport, SourceOnboardingError> {
    run_check(
        app_data_dir,
        key,
        Arc::new(ScriptedProfileHttpClient::new([])),
    )
}

struct Cancelled;

impl RuntimeCancellation for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[test]
fn cancelled_check_and_activate_preserves_draft_with_controlled_time() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let client = Arc::new(ScriptedProfileHttpClient::new([]));
    let cancellation = Cancelled;

    let outcome = block_on(onboarding(temp_dir.path(), Arc::clone(&client)).live_check(
        SourceLiveCheckRequest::CheckAndActivate {
            source_key: "example_source".to_string(),
        },
        OperationContext {
            checked_at: Some("2025-01-02T03:04:05Z"),
            cancellation: Some(&cancellation),
        },
    ))
    .unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Checked { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Draft);
            report
        }
        other => panic!("expected checked outcome, got {other:?}"),
    };

    assert_eq!(report.checked_at, "2025-01-02T03:04:05Z");
    assert_eq!(report.result, CheckReportResult::Failed);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "runtime_execution_cancelled"));
    assert!(client.requests().is_empty());
}

#[test]
fn invalid_custom_profile_is_quarantined_before_source_live_check_http() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    let source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["where"] = json!([{
        "type": "regex",
        "field": { "type": "json_path", "jsonPath": "$.title" },
        "pattern": "["
    }]);
    write_profile(temp_dir.path(), &profile);
    write_source(temp_dir.path(), &source);
    let client = Arc::new(ScriptedProfileHttpClient::new([]));

    let error = run_check(temp_dir.path(), "example_source", Arc::clone(&client)).unwrap_err();

    assert!(error
        .to_string()
        .contains("references unresolved Source Profile `example_jobs`"));
    assert!(client.requests().is_empty());
}

fn simple_profile() -> serde_json::Value {
    serde_json::from_str(SIMPLE_PROFILE).unwrap()
}

fn simple_profile_without_pagination() -> serde_json::Value {
    let mut profile = simple_profile();
    profile["accessPaths"][0]["discovery"]["strategies"][0]
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

fn passing_live_check_fetcher() -> FakeLiveCheckFetcher {
    FakeLiveCheckFetcher::new([
        (
            "https://example.test/jobs.json",
            json!({
                "jobs": [
                    {
                        "id": "job-1",
                        "title": "Senior Rust Engineer",
                        "url": "https://example.test/jobs/job-1",
                        "locations": ["Remote"]
                    }
                ]
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This is a sufficiently detailed job description for live checks.</p>"
            })
            .to_string(),
        ),
    ])
}

fn create_passed_source_live_check(app_data_dir: &Path) -> crate::job_radar_lib::CheckReport {
    write_profile(app_data_dir, &simple_profile_without_pagination());
    write_source(app_data_dir, &simple_source_with_status("draft"));
    let fetcher = passing_live_check_fetcher();
    run_check(app_data_dir, "example_source", fetcher.client()).unwrap()
}

fn assert_stale_detail(
    status: &crate::job_radar_lib::SourceLiveCheckReportStatus,
    kind: &str,
    reason: CheckReportStaleReason,
) {
    let freshness = status.freshness.as_ref().unwrap();
    assert!(
        freshness.stale_fingerprints.iter().any(|detail| {
            (detail.kind == kind || detail.reference.as_deref() == Some(kind))
                && detail.reason == reason
        }),
        "missing stale detail {kind}/{reason:?}: {:?}",
        freshness.stale_fingerprints
    );
}

#[test]
fn budget_exhausted_check_and_activate_preserves_draft_without_partial_payload() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let fetcher = FakeLiveCheckFetcher::new([
        (
            "https://example.test/jobs.json?page=1",
            json!({
                "jobs": [
                    {
                        "id": "job-1",
                        "title": "Senior Rust Engineer",
                        "url": "https://example.test/jobs/job-1",
                        "locations": ["Remote"]
                    }
                ]
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This is a sufficiently detailed job description for live checks.</p>"
            })
            .to_string(),
        ),
    ]);

    let outcome = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Checked { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Draft);
            report
        }
        other => panic!("budget exhaustion must not activate: {other:?}"),
    };

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(
        fetcher.discovery_requested_urls(),
        vec!["https://example.test/jobs.json?page=1"]
    );
    assert_eq!(report.details["discoveryMode"], json!("bounded_smoke"));
    assert_eq!(report.details["maxDiscoveryRequests"], json!(1));
    assert_eq!(
        report.details["discoveryExecutionReport"]["usage"]["requests"],
        json!(1)
    );
    assert_eq!(
        report.details["discoveryExecutionReport"]["usage"]["pages"],
        json!(1)
    );
    assert_eq!(report.details["candidateCount"], json!(0));
    assert_eq!(
        report.details["discoveryExecutionReport"]["completion"]["type"],
        json!("budget_exhausted")
    );
    assert_eq!(
        report.details["discoveryExecutionReport"]["completion"]["exhaustion"]["dimension"],
        json!("requests")
    );
    let allowance_diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "phase_allowance_exhausted")
        .expect("bounded Source Live Check should report cumulative exhaustion");
    assert_eq!(allowance_diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(allowance_diagnostic.path, "/discovery");
}

#[test]
fn workday_source_live_check_exhausts_after_one_cumulative_request_without_detail() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_source(
        temp_dir.path(),
        &json!({
            "schemaVersion": 3,
            "key": "workday_smoke",
            "name": "Workday Smoke",
            "status": "draft",
            "sourceConfig": {
                "workdayHost": "acme.wd3.myworkdayjobs.com",
                "tenant": "acme",
                "site": "External"
            },
            "selectedAccessPath": {
                "type": "profile_access_path",
                "profileKey": "workday",
                "pathKey": "cxs_api"
            }
        }),
    );
    let discovery_url = "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/jobs";
    let detail_url =
        "https://acme.wd3.myworkdayjobs.com/wday/cxs/acme/External/job/Germany-Berlin/job-1";
    let fetcher = FakeLiveCheckFetcher::new([
        (
            discovery_url,
            json!({
                "total": 372,
                "jobPostings": [{
                    "title": "Senior Rust Engineer",
                    "externalPath": "/job/Germany-Berlin/job-1",
                    "locationsText": "Berlin, Germany"
                }]
            })
            .to_string(),
        ),
        (
            detail_url,
            json!({
                "jobPostingInfo": {
                    "jobDescription": "<p>This is a sufficiently detailed Workday job description.</p>"
                }
            })
            .to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "workday_smoke", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let requests = fetcher.discovery_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, discovery_url);
    let body = requests[0].body.as_ref().expect("rendered JSON body");
    assert_eq!(
        body.bytes(),
        br#"{"appliedFacets":{},"limit":20,"offset":0}"#
    );
    assert_eq!(body.default_content_type(), Some("application/json"));
    assert!(fetcher.detail_requested_urls().is_empty());
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "phase_allowance_exhausted"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
    assert_eq!(
        report.details["discoveryExecutionReport"]["usage"]["requests"],
        json!(1)
    );
}

#[test]
fn check_source_creates_and_persists_passed_report_for_valid_draft_source() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("draft");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeLiveCheckFetcher::new([
        (
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
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This is a sufficiently detailed job description for live checks.</p>"
            })
            .to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

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
    assert_eq!(report.details["detailChecked"], json!(true));
    assert_eq!(report.details["detailPassed"], json!(true));

    for expected_reference in [
        "base_source_profile",
        "direct_source_specialization",
        "effective_source_profile",
        "compiler_provenance",
        "source_config",
        "selected_access_path",
        "profile_compiler",
        "profile_runtime",
        "immutable_globals",
    ] {
        assert!(
            report.fingerprints.iter().any(|fingerprint| {
                fingerprint.reference.as_deref() == Some(expected_reference)
            }),
            "missing fingerprint reference {expected_reference}: {:?}",
            report.fingerprints
        );
    }
    assert_eq!(
        fetcher.discovery_requested_urls(),
        vec!["https://example.test/jobs.json"]
    );
    assert_eq!(fetcher.detail_requested_urls(), vec!["job-1"]);

    let status = latest_status(temp_dir.path(), "example_source").unwrap();
    assert_eq!(status.report.as_ref(), Some(&report));
}

#[test]
fn source_onboarding_status_neutral_check_preserves_every_source_status() {
    for status in ["draft", "active", "disabled"] {
        let temp_dir = tempfile::tempdir().unwrap();
        write_profile(temp_dir.path(), &simple_profile_without_pagination());
        write_source(temp_dir.path(), &simple_source_with_status(status));
        let fetcher = passing_live_check_fetcher();

        let (report, source) =
            run_checked(temp_dir.path(), "example_source", fetcher.client()).unwrap();

        assert_eq!(report.result, CheckReportResult::Passed);
        assert_eq!(
            source.document.status,
            match status {
                "draft" => SourceStatus::Draft,
                "active" => SourceStatus::Active,
                "disabled" => SourceStatus::Disabled,
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn check_source_rejects_invalid_source_key_without_writing_outside_report_dir() {
    let temp_dir = tempfile::tempdir().unwrap();

    let fetcher = passing_live_check_fetcher();
    let error = run_check(temp_dir.path(), "../outside", fetcher.client()).unwrap_err();

    assert_eq!(error.kind, SourceOnboardingErrorKind::InvalidKey);
    assert!(!temp_dir.path().join("outside.json").exists());
}

#[test]
fn source_live_check_report_status_rejects_invalid_source_key_before_reading_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    fs::write(temp_dir.path().join("outside.json"), "{}").unwrap();

    let error = latest_status(temp_dir.path(), "../outside").unwrap_err();

    assert_eq!(error.kind, SourceOnboardingErrorKind::InvalidKey);
}

#[test]
fn source_live_check_report_status_is_unknown_without_persisted_report() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Unknown);
    assert!(status.report.is_none());
    assert!(status.freshness.is_none());
}

#[test]
fn source_live_check_report_status_marks_persisted_report_fresh() {
    let temp_dir = tempfile::tempdir().unwrap();
    let report = create_passed_source_live_check(temp_dir.path());

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Fresh);
    assert_eq!(status.report.as_ref(), Some(&report));
    let freshness = status.freshness.as_ref().unwrap();
    assert_eq!(freshness.state, CheckReportFreshnessState::Fresh);
    assert!(freshness.stale_fingerprints.is_empty());
}

#[test]
fn source_live_check_report_status_excludes_source_name_metadata() {
    let temp_dir = tempfile::tempdir().unwrap();
    create_passed_source_live_check(temp_dir.path());
    let mut source = simple_source_with_status("draft");
    source["name"] = json!("Renamed Example Source");
    let document: SourceDocument = serde_json::from_value(source).unwrap();
    let revised = InstalledSourceStore::new(temp_dir.path())
        .revise(Revision {
            key: document.key,
            name: document.name,
            source_config: document.source_config,
            selected_access_path: document.selected_access_path,
            access_paths: document.access_paths,
            source_support: document.source_support,
        })
        .unwrap();
    assert_eq!(revised.document.status, SourceStatus::Draft);

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Fresh);
    assert_eq!(
        status.freshness.as_ref().unwrap().state,
        CheckReportFreshnessState::Fresh
    );
    assert_eq!(
        status.report.as_ref().unwrap().result,
        CheckReportResult::Passed
    );
}

#[test]
fn source_live_check_report_status_marks_changed_profile_document_stale_without_mutating_source_status(
) {
    let temp_dir = tempfile::tempdir().unwrap();
    create_passed_source_live_check(temp_dir.path());
    let mut profile = simple_profile_without_pagination();
    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["url"] =
        json!("https://changed.example.test/jobs");
    write_profile(temp_dir.path(), &profile);

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Stale);
    assert_eq!(
        status.report.as_ref().unwrap().result,
        CheckReportResult::Passed
    );
    assert_stale_detail(
        &status,
        "base_source_profile",
        CheckReportStaleReason::ChangedFingerprintSha256,
    );
}

#[test]
fn source_live_check_report_status_marks_changed_source_config_and_direct_specialization_stale() {
    let temp_dir = tempfile::tempdir().unwrap();
    create_passed_source_live_check(temp_dir.path());
    let mut source = simple_source_with_status("draft");
    source["sourceConfig"]["language"] = json!("de");
    source["accessPaths"][0]["discovery"]["strategies"][0]["acceptWhen"]["minResults"] = json!(2);
    let document: SourceDocument = serde_json::from_value(source).unwrap();
    let revised = InstalledSourceStore::new(temp_dir.path())
        .revise(Revision {
            key: document.key,
            name: document.name,
            source_config: document.source_config,
            selected_access_path: document.selected_access_path,
            access_paths: document.access_paths,
            source_support: document.source_support,
        })
        .unwrap();
    assert_eq!(revised.document.status, SourceStatus::Draft);

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Stale);
    assert_stale_detail(
        &status,
        "source_config",
        CheckReportStaleReason::ChangedFingerprintSha256,
    );
    assert_stale_detail(
        &status,
        "direct_source_specialization",
        CheckReportStaleReason::ChangedFingerprintSha256,
    );
    assert_eq!(
        status.report.as_ref().unwrap().result,
        CheckReportResult::Passed
    );
}

#[test]
fn source_live_check_report_status_marks_changed_logic_version_stale() {
    let temp_dir = tempfile::tempdir().unwrap();
    create_passed_source_live_check(temp_dir.path());
    let report_path = temp_dir
        .path()
        .join("source-live-checks/example_source.json");
    let mut report: CheckReport = serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    report.logic_version = "source-live-check/v0".to_string();
    fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();

    let status = latest_status(temp_dir.path(), "example_source").unwrap();

    assert_eq!(status.state, SourceLiveCheckReportState::Stale);
    assert_eq!(
        status.report.as_ref().unwrap().result,
        CheckReportResult::Passed
    );
    assert_stale_detail(
        &status,
        "logic_version",
        CheckReportStaleReason::LogicVersionChanged,
    );
}

#[test]
fn check_source_rejects_unknown_source_without_persisting_a_report() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile());

    let run_error = run_check_without_io(temp_dir.path(), "missing_source").unwrap_err();
    let status_error = latest_status(temp_dir.path(), "missing_source").unwrap_err();

    assert_eq!(run_error.kind, SourceOnboardingErrorKind::NotFound);
    assert_eq!(status_error.kind, SourceOnboardingErrorKind::NotFound);
}

#[test]
fn check_source_maps_invalid_values_to_source_validation_diagnostics() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut source = simple_source_with_status("active");
    source["sourceConfig"] = json!({ "language": "en" });
    write_profile(temp_dir.path(), &simple_profile());
    write_source(temp_dir.path(), &source);

    let report = run_check_without_io(temp_dir.path(), "example_source").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(report.details["sourceStatusAtCheck"], json!("active"));
    assert_eq!(report.details["liveCheckState"], json!("live_check_failed"));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::SourceValidation
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
    let fetcher = FakeLiveCheckFetcher::new([(
        "https://example.test/jobs.json",
        json!({ "jobs": [] }).to_string(),
    )]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

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
        FakeLiveCheckFetcher::new([("https://example.test/jobs.json", "not json".to_string())]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

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
    let fetcher = FakeLiveCheckFetcher::new([
        (
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
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This detail text is long enough to pass acceptance checks.</p>"
            })
            .to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(report.details["candidateCount"], json!(1));
}

#[test]
fn check_source_emits_detail_failed_when_one_candidate_detail_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeLiveCheckFetcher::new([
        (
            "https://example.test/jobs.json",
            json!({
                "jobs": [
                    {
                        "id": "job-1",
                        "title": "Senior Rust Engineer",
                        "url": "https://example.test/jobs/job-1"
                    }
                ]
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({ "descriptionHtml": "too short" }).to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(report.details["liveCheckState"], json!("live_check_failed"));
    assert_eq!(report.details["detailChecked"], json!(true));
    assert_eq!(report.details["detailPassed"], json!(false));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Runtime
            && diagnostic.code == "description_too_short"
            && diagnostic.severity == DiagnosticSeverity::Error
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.category == DiagnosticCategory::Runtime
            && diagnostic.code == "source_live_check.detail_failed"
            && diagnostic.severity == DiagnosticSeverity::Error
            && diagnostic.details.as_ref()
                == Some(&json!({
                    "sourceKey": "example_source",
                    "profileKey": "example_jobs",
                    "accessPathKey": "json_feed",
                    "candidateUrl": "https://example.test/jobs/job-1",
                    "cause": "detail_description_text_unavailable"
                }))
    }));
}

#[test]
fn check_source_passes_detail_when_fallback_strategy_extracts_description() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    let mut profile = simple_profile_without_pagination();
    let mut failing_detail_strategy = profile["accessPaths"][0]["detail"]["strategies"][0].clone();
    failing_detail_strategy["key"] = json!("missing_description");
    failing_detail_strategy["extract"]["fields"]["descriptionText"]["jsonPath"] =
        json!("$.missingDescriptionHtml");
    let mut fallback_detail_strategy = profile["accessPaths"][0]["detail"]["strategies"][0].clone();
    fallback_detail_strategy["key"] = json!("fallback_detail_api");
    profile["accessPaths"][0]["detail"]["strategies"] =
        json!([failing_detail_strategy, fallback_detail_strategy]);
    write_profile(temp_dir.path(), &profile);
    write_source(temp_dir.path(), &source);
    let fetcher = FakeLiveCheckFetcher::new([
        (
            "https://example.test/jobs.json",
            json!({
                "jobs": [
                    {
                        "id": "job-1",
                        "title": "Senior Rust Engineer",
                        "url": "https://example.test/jobs/job-1"
                    }
                ]
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This fallback detail text is long enough to pass acceptance checks.</p>"
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This fallback detail text is long enough to pass acceptance checks.</p>"
            })
            .to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(report.details["liveCheckState"], json!("live_check_passed"));
    assert_eq!(report.details["detailChecked"], json!(true));
    assert_eq!(report.details["detailPassed"], json!(true));
    assert!(report.diagnostics.iter().all(|diagnostic| {
        diagnostic.severity != DiagnosticSeverity::Error
            && diagnostic.code != "source_live_check.detail_failed"
    }));
}

#[test]
fn check_source_leaves_detail_unchecked_when_access_path_has_no_detail() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    let mut profile = simple_profile_without_pagination();
    profile["accessPaths"][0]
        .as_object_mut()
        .unwrap()
        .remove("detail");
    write_profile(temp_dir.path(), &profile);
    write_source(temp_dir.path(), &source);
    let fetcher = FakeLiveCheckFetcher::new([(
        "https://example.test/jobs.json",
        json!({
            "jobs": [
                {
                    "id": "job-1",
                    "title": "Senior Rust Engineer",
                    "url": "https://example.test/jobs/job-1"
                }
            ]
        })
        .to_string(),
    )]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(report.details["detailChecked"], json!(false));
    assert_eq!(report.details["detailPassed"], serde_json::Value::Null);
    assert!(fetcher.detail_requested_urls().is_empty());
}

#[test]
fn check_source_checks_detail_for_no_more_than_one_candidate() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = simple_source_with_status("active");
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &source);
    let fetcher = FakeLiveCheckFetcher::new([
        (
            "https://example.test/jobs.json",
            json!({
                "jobs": [
                    {
                        "id": "job-1",
                        "title": "Senior Rust Engineer",
                        "url": "https://example.test/jobs/job-1"
                    },
                    {
                        "id": "job-2",
                        "title": "Staff Rust Engineer",
                        "url": "https://example.test/jobs/job-2"
                    }
                ]
            })
            .to_string(),
        ),
        (
            "job-1",
            json!({
                "descriptionHtml": "<p>This detail text is long enough to pass acceptance checks.</p>"
            })
            .to_string(),
        ),
        (
            "job-2",
            json!({ "descriptionHtml": "<p>This second detail must not be fetched.</p>" })
                .to_string(),
        ),
    ]);

    let report = run_check(temp_dir.path(), "example_source", fetcher.client()).unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(report.details["candidateCount"], json!(2));
    assert_eq!(report.details["detailChecked"], json!(true));
    assert_eq!(report.details["detailPassed"], json!(true));
    assert_eq!(fetcher.detail_requested_urls(), vec!["job-1"]);
}

#[test]
fn check_and_activate_source_changes_draft_to_active_after_passed_live_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let fetcher = passing_live_check_fetcher();

    let outcome = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Activated { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Active);
            report
        }
        other => panic!("expected activation, got {other:?}"),
    };

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(
        fetcher.discovery_requested_urls(),
        vec!["https://example.test/jobs.json"]
    );
    let status = latest_status(temp_dir.path(), "example_source").unwrap();
    assert_eq!(status.report.as_ref(), Some(&report));
    assert_eq!(status.state, SourceLiveCheckReportState::Fresh);
}

#[test]
fn check_and_activate_source_leaves_draft_unchanged_after_failed_live_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let fetcher = FakeLiveCheckFetcher::new([(
        "https://example.test/jobs.json",
        json!({ "jobs": [] }).to_string(),
    )]);

    let outcome = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Checked { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Draft);
            report
        }
        other => panic!("failed check must not activate: {other:?}"),
    };

    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(
        latest_status(temp_dir.path(), "example_source")
            .unwrap()
            .report
            .as_ref(),
        Some(&report)
    );
}

#[test]
fn check_and_reactivate_source_changes_disabled_to_active_after_passed_live_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("disabled"));
    let fetcher = passing_live_check_fetcher();

    let outcome = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Activated { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Active);
            report
        }
        other => panic!("expected reactivation, got {other:?}"),
    };

    assert_eq!(report.result, CheckReportResult::Passed);
    assert_eq!(
        fetcher.discovery_requested_urls(),
        vec!["https://example.test/jobs.json"]
    );
    assert_eq!(
        latest_status(temp_dir.path(), "example_source")
            .unwrap()
            .state,
        SourceLiveCheckReportState::Fresh
    );
}

#[test]
fn check_and_reactivate_source_leaves_disabled_unchanged_after_failed_live_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("disabled"));
    let fetcher = FakeLiveCheckFetcher::new([(
        "https://example.test/jobs.json",
        json!({ "jobs": [] }).to_string(),
    )]);

    let outcome = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap();
    let report = match outcome {
        SourceLiveCheckOutcome::Checked { report, source } => {
            assert_eq!(source.document.status, SourceStatus::Disabled);
            report
        }
        other => panic!("failed check must not reactivate: {other:?}"),
    };

    assert_eq!(report.result, CheckReportResult::Failed);
}

#[test]
fn check_and_activate_rejects_active_source_before_external_work() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("active"));
    let fetcher = passing_live_check_fetcher();

    let error = run_activation(temp_dir.path(), "example_source", fetcher.client()).unwrap_err();

    assert_eq!(error.kind, SourceOnboardingErrorKind::InvalidLifecycle);
    assert!(fetcher.client.requests().is_empty());
}

struct FakeLiveCheckFetcher {
    client: Arc<ScriptedProfileHttpClient>,
}

impl FakeLiveCheckFetcher {
    fn new<'a>(responses: impl IntoIterator<Item = (&'a str, String)>) -> Self {
        Self {
            client: Arc::new(ScriptedProfileHttpClient::new(responses.into_iter().map(
                |(url, body)| ScriptedHttpEvent::Response {
                    status: 200,
                    final_url: url.to_string(),
                    headers: Vec::new(),
                    body: vec![ScriptedHttpBodyEvent::Chunk(body.into_bytes())],
                    content_length: None,
                },
            ))),
        }
    }

    fn client(&self) -> Arc<ScriptedProfileHttpClient> {
        Arc::clone(&self.client)
    }

    fn discovery_requests(&self) -> Vec<ProfileHttpRequest> {
        self.client
            .requests()
            .into_iter()
            .filter(|request| request.url != "job-1")
            .collect()
    }

    fn discovery_requested_urls(&self) -> Vec<String> {
        self.discovery_requests()
            .into_iter()
            .map(|request| request.url)
            .collect()
    }

    fn detail_requested_urls(&self) -> Vec<String> {
        self.client
            .requests()
            .into_iter()
            .filter(|request| request.url == "job-1")
            .map(|request| request.url)
            .collect()
    }
}

#[test]
fn source_onboarding_check_and_activate_returns_the_persisted_report_fingerprints() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let fetcher = passing_live_check_fetcher();
    // Scripted clients are stateful, so this interface test supplies its own shared script.
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(json!({"jobs":[{"id":"job-1","title":"Rust Engineer","url":"https://example.test/jobs/job-1"}]}).to_string().into_bytes())],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(json!({"descriptionHtml":"<p>This description is sufficiently long for activation.</p>"}).to_string().into_bytes())],
            content_length: None,
        },
    ]));
    drop(fetcher);
    let onboarding = onboarding(temp_dir.path(), client);

    let outcome = block_on(onboarding.live_check(
        SourceLiveCheckRequest::CheckAndActivate {
            source_key: "example_source".to_string(),
        },
        OperationContext {
            checked_at: Some("2025-01-02T03:04:05Z"),
            cancellation: None,
        },
    ))
    .unwrap();
    let (report, source) = match outcome {
        SourceLiveCheckOutcome::Activated { report, source } => (report, source),
        other => panic!("expected activation, got {other:?}"),
    };
    assert_eq!(source.document.status, SourceStatus::Active);
    let persisted = latest_status(temp_dir.path(), "example_source")
        .unwrap()
        .report
        .expect("activation report should be persisted");
    assert_eq!(persisted.fingerprints, report.fingerprints);
    assert_eq!(persisted.checked_at, "2025-01-02T03:04:05Z");
}

#[test]
fn source_onboarding_report_storage_failure_cannot_leave_source_active() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    fs::write(
        temp_dir.path().join("source-live-checks"),
        "not a directory",
    )
    .unwrap();
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(json!({"jobs":[{"id":"job-1","title":"Rust Engineer","url":"https://example.test/jobs/job-1"}]}).to_string().into_bytes())],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(json!({"descriptionHtml":"<p>This description is sufficiently long for activation.</p>"}).to_string().into_bytes())],
            content_length: None,
        },
    ]));
    let onboarding = onboarding(temp_dir.path(), client);

    let error = block_on(onboarding.live_check(
        SourceLiveCheckRequest::CheckAndActivate {
            source_key: "example_source".to_string(),
        },
        OperationContext::default(),
    ))
    .unwrap_err();

    assert_eq!(error.kind, SourceOnboardingErrorKind::Storage);
    let document: SourceDocument = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    let revised = InstalledSourceStore::new(temp_dir.path())
        .revise(Revision {
            key: document.key,
            name: document.name,
            source_config: document.source_config,
            selected_access_path: document.selected_access_path,
            access_paths: document.access_paths,
            source_support: document.source_support,
        })
        .unwrap();
    assert_eq!(revised.document.status, SourceStatus::Draft);
}

#[test]
fn live_check_releases_source_coordination_and_rejects_admission_after_concurrent_revision() {
    let temp_dir = tempfile::tempdir().unwrap();
    write_profile(temp_dir.path(), &simple_profile_without_pagination());
    write_source(temp_dir.path(), &simple_source_with_status("draft"));
    let store = InstalledSourceStore::new(temp_dir.path());
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Gate("live-check".to_string()),
                ScriptedHttpBodyEvent::Chunk(json!({"jobs":[{"id":"job-1","title":"Rust Engineer","company":"ACME","url":"https://example.test/jobs/job-1"}]}).to_string().into_bytes()),
            ],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(json!({"descriptionHtml":"<p>This description is sufficiently long for admission.</p>"}).to_string().into_bytes())],
            content_length: None,
        },
    ]));
    let application = SourceOnboarding::new(
        temp_dir.path(),
        client.clone(),
        Arc::new(ScriptedBrowserAcquisition::new([])),
    );
    let run = std::thread::spawn(move || {
        block_on(application.live_check(
            SourceLiveCheckRequest::CheckAndActivate {
                source_key: "example_source".to_string(),
            },
            OperationContext::default(),
        ))
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !client.gate_is_waiting("live-check") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(client.gate_is_waiting("live-check"));
    let document: SourceDocument = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    store
        .revise(Revision {
            key: document.key,
            name: "Concurrent revision".to_string(),
            source_config: document.source_config,
            selected_access_path: document.selected_access_path,
            access_paths: document.access_paths,
            source_support: document.source_support,
        })
        .unwrap();
    assert!(client.release_gate("live-check"));

    let error = run.join().unwrap().unwrap_err();
    assert_eq!(error.kind, SourceOnboardingErrorKind::GenerationMismatch);
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}
