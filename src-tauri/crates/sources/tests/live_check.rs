use std::{fs, path::Path, sync::Arc};

use serde_json::json;
use source_engine::{
    execution::RuntimeCancellation,
    test_support::{
        ScriptedBrowserAcquisition, ScriptedHttpBodyEvent, ScriptedHttpEvent,
        ScriptedProfileHttpClient,
    },
};
use sources::{
    installed::{Revision, SourceDocument, SourceStatus, Store},
    live_check::{
        AdmissionOutcome, Clock, Context, ErrorKind, Operation, ReportResult, ReportState,
    },
};

const SIMPLE_PROFILE: &str =
    include_str!("../../../tests/fixtures/source-behavior/valid/simple-source-profile.json");
const SIMPLE_SOURCE: &str =
    include_str!("fixtures/sources/valid/source-selecting-access-path.json");

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> String {
        "2025-01-02T03:04:05Z".to_string()
    }
}

struct Cancelled;

impl RuntimeCancellation for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

struct CancelAfterReport {
    path: std::path::PathBuf,
}

impl RuntimeCancellation for CancelAfterReport {
    fn is_cancelled(&self) -> bool {
        self.path.exists()
    }
}

fn write_document(directory: &Path, collection: &str, document: &str) {
    write_value(
        directory,
        collection,
        &serde_json::from_str(document).unwrap(),
    );
}

fn write_value(directory: &Path, collection: &str, document: &serde_json::Value) {
    let collection = directory.join(collection);
    fs::create_dir_all(&collection).unwrap();
    fs::write(
        collection.join(format!("{}.json", document["key"].as_str().unwrap())),
        serde_json::to_string_pretty(document).unwrap(),
    )
    .unwrap();
}

fn install_checkable(directory: &Path, status: &str) {
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    write_value(directory, "source-profiles", &profile);
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!(status);
    write_value(directory, "sources", &source);
}

fn passing_client() -> Arc<ScriptedProfileHttpClient> {
    Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"jobs":[
                    {"id":"job-1","title":"Rust Engineer","url":"https://example.test/jobs/job-1"},
                    {"id":"job-2","title":"TypeScript Engineer","url":"https://example.test/jobs/job-2"}
                ]})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"descriptionHtml":"<p>A sufficiently detailed description for this live check.</p>"})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
    ]))
}

fn operation(directory: &Path) -> Operation {
    operation_with_client(directory, Arc::new(ScriptedProfileHttpClient::new([])))
}

fn operation_with_client(directory: &Path, client: Arc<ScriptedProfileHttpClient>) -> Operation {
    Operation::new(
        Store::new(directory),
        client,
        Arc::new(ScriptedBrowserAcquisition::new([])),
        Arc::new(FixedClock),
    )
}

#[tokio::test]
async fn latest_report_is_unknown_before_a_source_has_been_checked() {
    let directory = tempfile::tempdir().unwrap();
    write_document(directory.path(), "source-profiles", SIMPLE_PROFILE);
    write_document(directory.path(), "sources", SIMPLE_SOURCE);

    let status = operation(directory.path())
        .status("example_source")
        .await
        .unwrap();

    assert_eq!(status.state, ReportState::Unknown);
    assert!(status.report.is_none());
    assert!(status.freshness.is_none());
}

#[tokio::test]
async fn status_neutral_run_persists_a_passed_report_without_activating_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    write_value(directory.path(), "source-profiles", &profile);
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!("draft");
    write_value(directory.path(), "sources", &source);
    let client = passing_client();

    let outcome = operation_with_client(directory.path(), Arc::clone(&client))
        .run("example_source", Context::default())
        .await
        .unwrap();

    assert_eq!(outcome.report.result, ReportResult::Passed);
    assert_eq!(outcome.report.checked_at, "2025-01-02T03:04:05Z");
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
    let status = operation(directory.path())
        .status("example_source")
        .await
        .unwrap();
    assert_eq!(status.state, ReportState::Fresh);
    assert_eq!(status.report, Some(outcome.report));
    assert_eq!(client.requests().len(), 2);
    assert_eq!(
        status.report.as_ref().unwrap().details["candidateCount"],
        json!(2)
    );
    assert_eq!(
        status.report.as_ref().unwrap().details["maxDiscoveryRequests"],
        json!(1)
    );
    assert_eq!(
        status.report.as_ref().unwrap().details["detailChecked"],
        json!(true)
    );

    profile["accessPaths"][0]["discovery"]["strategies"][0]["fetch"]["url"] =
        json!("https://changed.test/jobs.json");
    write_value(directory.path(), "source-profiles", &profile);
    assert_eq!(
        operation(directory.path())
            .status("example_source")
            .await
            .unwrap()
            .state,
        ReportState::Stale
    );
}

#[tokio::test]
async fn successful_check_and_activate_admits_a_custom_draft_source() {
    let directory = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    write_value(directory.path(), "source-profiles", &profile);
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!("draft");
    write_value(directory.path(), "sources", &source);
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"jobs":[{"id":"job-1","title":"Rust Engineer","url":"https://example.test/jobs/job-1"}]})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"descriptionHtml":"<p>A sufficiently detailed description for activation.</p>"})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
    ]));

    let outcome = operation_with_client(directory.path(), client)
        .check_and_activate("example_source", Context::default())
        .await
        .unwrap();

    let AdmissionOutcome::Activated { report, source } = outcome else {
        panic!("a passing draft Source should be activated")
    };
    assert_eq!(report.result, ReportResult::Passed);
    assert_eq!(source.document.status, SourceStatus::Active);
    assert_eq!(
        operation(directory.path())
            .status("example_source")
            .await
            .unwrap()
            .report,
        Some(report)
    );
}

#[tokio::test]
async fn cancelled_admission_persists_evidence_without_activating() {
    let directory = tempfile::tempdir().unwrap();
    write_document(directory.path(), "source-profiles", SIMPLE_PROFILE);
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!("draft");
    write_value(directory.path(), "sources", &source);
    let cancellation = Arc::new(Cancelled);

    let outcome = operation(directory.path())
        .check_and_activate(
            "example_source",
            Context {
                cancellation: Some(cancellation),
            },
        )
        .await
        .unwrap();

    let AdmissionOutcome::Checked { report } = outcome else {
        panic!("a cancelled check must not activate")
    };
    assert_eq!(report.result, ReportResult::Failed);
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}

#[tokio::test]
async fn cancellation_observed_at_final_admission_point_never_activates() {
    let directory = tempfile::tempdir().unwrap();
    install_checkable(directory.path(), "draft");
    let cancellation = Arc::new(CancelAfterReport {
        path: directory
            .path()
            .join("source-live-checks/example_source.json"),
    });

    let outcome = operation_with_client(directory.path(), passing_client())
        .check_and_activate(
            "example_source",
            Context {
                cancellation: Some(cancellation),
            },
        )
        .await
        .unwrap();

    let AdmissionOutcome::Checked { report } = outcome else {
        panic!("cancellation at final admission must not activate")
    };
    assert_eq!(report.result, ReportResult::Failed);
    assert_eq!(
        operation(directory.path())
            .status("example_source")
            .await
            .unwrap()
            .report
            .unwrap()
            .result,
        ReportResult::Failed
    );
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}

#[tokio::test]
async fn active_source_is_rejected_before_external_work() {
    let directory = tempfile::tempdir().unwrap();
    write_document(directory.path(), "source-profiles", SIMPLE_PROFILE);
    write_document(directory.path(), "sources", SIMPLE_SOURCE);
    let client = Arc::new(ScriptedProfileHttpClient::new([]));

    let error = operation_with_client(directory.path(), Arc::clone(&client))
        .check_and_activate("example_source", Context::default())
        .await
        .unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidLifecycle);
    assert!(client.requests().is_empty());
}

#[tokio::test]
async fn failed_discovery_never_activates() {
    let directory = tempfile::tempdir().unwrap();
    install_checkable(directory.path(), "draft");
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"jobs": []}).to_string().into_bytes(),
            )],
            content_length: None,
        },
    ]));

    let outcome = operation_with_client(directory.path(), client)
        .check_and_activate("example_source", Context::default())
        .await
        .unwrap();

    let AdmissionOutcome::Checked { report } = outcome else {
        panic!("failed Discovery must not activate")
    };
    assert_eq!(report.result, ReportResult::Failed);
    assert_eq!(report.details["detailChecked"], json!(false));
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}

#[tokio::test]
async fn discovery_budget_exhaustion_never_activates_or_runs_detail() {
    let directory = tempfile::tempdir().unwrap();
    write_document(directory.path(), "source-profiles", SIMPLE_PROFILE);
    let mut source: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source["status"] = json!("draft");
    write_value(directory.path(), "sources", &source);
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json?page=1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"jobs":[{"id":"job-1","title":"Rust Engineer","url":"https://example.test/jobs/job-1"}]})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
    ]));

    let outcome = operation_with_client(directory.path(), Arc::clone(&client))
        .check_and_activate("example_source", Context::default())
        .await
        .unwrap();

    let AdmissionOutcome::Checked { report } = outcome else {
        panic!("budget exhaustion must not activate")
    };
    assert_eq!(report.result, ReportResult::Failed);
    assert_eq!(report.details["detailChecked"], json!(false));
    assert_eq!(client.requests().len(), 1);
    assert_eq!(
        report.details["discoveryExecutionReport"]["completion"]["type"],
        json!("budget_exhausted")
    );
}

#[tokio::test]
async fn successful_status_neutral_checks_preserve_draft_active_and_disabled_sources() {
    for (status, expected) in [
        ("draft", SourceStatus::Draft),
        ("active", SourceStatus::Active),
        ("disabled", SourceStatus::Disabled),
    ] {
        let directory = tempfile::tempdir().unwrap();
        install_checkable(directory.path(), status);
        let report = operation_with_client(directory.path(), passing_client())
            .run("example_source", Context::default())
            .await
            .unwrap()
            .report;
        assert_eq!(report.result, ReportResult::Passed);
        assert_eq!(
            Store::new(directory.path())
                .snapshot()
                .unwrap()
                .source("example_source")
                .unwrap()
                .document()
                .status,
            expected
        );
    }
}

#[tokio::test]
async fn successful_check_and_activate_reactivates_a_disabled_source() {
    let directory = tempfile::tempdir().unwrap();
    install_checkable(directory.path(), "disabled");

    let outcome = operation_with_client(directory.path(), passing_client())
        .check_and_activate("example_source", Context::default())
        .await
        .unwrap();

    let AdmissionOutcome::Activated { source, .. } = outcome else {
        panic!("a passing disabled Source should be reactivated")
    };
    assert_eq!(source.document.status, SourceStatus::Active);
}

#[tokio::test]
async fn report_write_failure_never_changes_source_status() {
    let directory = tempfile::tempdir().unwrap();
    install_checkable(directory.path(), "draft");
    fs::write(
        directory.path().join("source-live-checks"),
        "not a directory",
    )
    .unwrap();
    let cancellation = Arc::new(Cancelled);

    let error = operation(directory.path())
        .check_and_activate(
            "example_source",
            Context {
                cancellation: Some(cancellation),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ErrorKind::Storage);
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}

#[cfg(unix)]
#[tokio::test]
async fn source_write_failure_happens_after_report_persistence_without_false_activation() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    install_checkable(directory.path(), "draft");
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Gate("source-write-failure".to_string()),
                ScriptedHttpBodyEvent::Chunk(
                    json!({"jobs":[{"id":"job-1","title":"Rust Engineer","company":"ACME","url":"https://example.test/jobs/job-1"}]})
                        .to_string()
                        .into_bytes(),
                ),
            ],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"descriptionHtml":"<p>A sufficiently detailed description for admission.</p>"})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
    ]));
    let live_check = operation_with_client(directory.path(), Arc::clone(&client));
    let run = tokio::spawn(async move {
        live_check
            .check_and_activate("example_source", Context::default())
            .await
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !client.gate_is_waiting("source-write-failure") && std::time::Instant::now() < deadline {
        tokio::task::yield_now().await;
    }
    assert!(client.gate_is_waiting("source-write-failure"));
    let sources_directory = directory.path().join("sources");
    fs::set_permissions(&sources_directory, fs::Permissions::from_mode(0o500)).unwrap();
    assert!(client.release_gate("source-write-failure"));

    let error = run.await.unwrap().unwrap_err();
    fs::set_permissions(&sources_directory, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error.kind, ErrorKind::Storage);
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
    let status = operation(directory.path())
        .status("example_source")
        .await
        .unwrap();
    assert_eq!(status.state, ReportState::Fresh);
    assert_eq!(status.report.unwrap().result, ReportResult::Passed);
}

#[tokio::test]
async fn source_revision_during_await_returns_typed_stale_generation_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["accessPaths"][0]["discovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    write_value(directory.path(), "source-profiles", &profile);
    let mut source_value: serde_json::Value = serde_json::from_str(SIMPLE_SOURCE).unwrap();
    source_value["status"] = json!("draft");
    write_value(directory.path(), "sources", &source_value);
    let client = Arc::new(ScriptedProfileHttpClient::new([
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "https://example.test/jobs.json".to_string(),
            headers: Vec::new(),
            body: vec![
                ScriptedHttpBodyEvent::Gate("live-check".to_string()),
                ScriptedHttpBodyEvent::Chunk(
                    json!({"jobs":[{"id":"job-1","title":"Rust Engineer","company":"ACME","url":"https://example.test/jobs/job-1"}]})
                        .to_string()
                        .into_bytes(),
                ),
            ],
            content_length: None,
        },
        ScriptedHttpEvent::Response {
            status: 200,
            final_url: "job-1".to_string(),
            headers: Vec::new(),
            body: vec![ScriptedHttpBodyEvent::Chunk(
                json!({"descriptionHtml":"<p>A sufficiently detailed description for admission.</p>"})
                    .to_string()
                    .into_bytes(),
            )],
            content_length: None,
        },
    ]));
    let live_check = operation_with_client(directory.path(), Arc::clone(&client));
    let run = tokio::spawn(async move {
        live_check
            .check_and_activate("example_source", Context::default())
            .await
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !client.gate_is_waiting("live-check") && std::time::Instant::now() < deadline {
        tokio::task::yield_now().await;
    }
    assert!(client.gate_is_waiting("live-check"));
    let source: SourceDocument = serde_json::from_value(source_value).unwrap();
    Store::new(directory.path())
        .revise(Revision {
            key: source.key,
            name: "Concurrent revision".to_string(),
            source_config: source.source_config,
            selected_access_path: source.selected_access_path,
            access_paths: source.access_paths,
            source_support: source.source_support,
        })
        .unwrap();
    assert!(client.release_gate("live-check"));

    let error = run.await.unwrap().unwrap_err();
    assert_eq!(error.kind, ErrorKind::StaleGeneration);
    assert_eq!(
        Store::new(directory.path())
            .snapshot()
            .unwrap()
            .source("example_source")
            .unwrap()
            .document()
            .status,
        SourceStatus::Draft
    );
}
