use job_radar_lib::{
    evaluate_check_report_freshness, latest_check_report_path, persist_latest_check_report,
    read_latest_check_report, source_live_check_report_path, CheckFingerprint, CheckReport,
    CheckReportFreshnessState, CheckReportKind, CheckReportResult, CheckReportStaleReason,
    CheckReportSubject, CHECK_REPORT_SCHEMA_VERSION,
};
use serde_json::json;

#[test]
fn source_live_check_report_round_trips_through_json() {
    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Failed,
    );
    report.fingerprints = vec![CheckFingerprint::new(
        "source_document",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )];
    report
        .details
        .insert("candidateCount".to_string(), json!(0));

    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value,
        json!({
            "schemaVersion": 1,
            "kind": "source_live_check",
            "subject": {
                "type": "source",
                "key": "acme_jobs"
            },
            "checkedAt": "2026-07-07T12:00:00Z",
            "logicVersion": "source-live-check/v1",
            "result": "failed",
            "fingerprints": [
                {
                    "kind": "source_document",
                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                }
            ],
            "diagnostics": [],
            "details": {
                "candidateCount": 0
            }
        })
    );

    let round_tripped: CheckReport = serde_json::from_value(value).unwrap();
    assert_eq!(round_tripped, report);
    assert_eq!(round_tripped.schema_version, CHECK_REPORT_SCHEMA_VERSION);
}

#[test]
fn check_report_deserialization_enforces_report_contract() {
    let unsupported_result = json!({
        "schemaVersion": 1,
        "kind": "source_live_check",
        "subject": {
            "type": "source",
            "key": "acme_jobs"
        },
        "checkedAt": "2026-07-07T12:00:00Z",
        "logicVersion": "source-live-check/v1",
        "result": "stale",
        "fingerprints": [],
        "diagnostics": [],
        "details": {}
    });

    assert!(serde_json::from_value::<CheckReport>(unsupported_result).is_err());

    let unsupported_schema_version = json!({
        "schemaVersion": 2,
        "kind": "source_live_check",
        "subject": {
            "type": "source",
            "key": "acme_jobs"
        },
        "checkedAt": "2026-07-07T12:00:00Z",
        "logicVersion": "source-live-check/v1",
        "result": "passed",
        "fingerprints": [],
        "diagnostics": [],
        "details": {}
    });
    let error = serde_json::from_value::<CheckReport>(unsupported_schema_version).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported Check Report schemaVersion 2"),
        "unexpected error: {error}"
    );
}

#[test]
fn freshness_is_fresh_when_logic_version_and_fingerprints_match() {
    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Passed,
    );
    report.fingerprints = vec![
        CheckFingerprint::new(
            "source_document",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        CheckFingerprint::with_reference(
            "source_config",
            "sourceConfig",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
    ];

    let freshness = evaluate_check_report_freshness(
        &report,
        "source-live-check/v1",
        &report.fingerprints.clone(),
    );

    assert_eq!(freshness.state, CheckReportFreshnessState::Fresh);
    assert!(freshness.stale_fingerprints.is_empty());
    assert!(freshness.is_fresh());
}

#[test]
fn freshness_is_stale_when_expected_fingerprint_is_missing() {
    let report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Passed,
    );
    let current_fingerprints = vec![CheckFingerprint::new(
        "source_document",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )];

    let freshness =
        evaluate_check_report_freshness(&report, "source-live-check/v1", &current_fingerprints);

    assert_eq!(freshness.state, CheckReportFreshnessState::Stale);
    assert_eq!(freshness.stale_fingerprints.len(), 1);
    assert_eq!(
        freshness.stale_fingerprints[0].reason,
        CheckReportStaleReason::MissingReportFingerprint
    );
    assert_eq!(freshness.stale_fingerprints[0].kind, "source_document");
}

#[test]
fn freshness_is_stale_when_fingerprint_hash_changed() {
    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Passed,
    );
    report.fingerprints = vec![CheckFingerprint::with_reference(
        "source_config",
        "sourceConfig",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )];
    let current_fingerprints = vec![CheckFingerprint::with_reference(
        "source_config",
        "sourceConfig",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )];

    let freshness =
        evaluate_check_report_freshness(&report, "source-live-check/v1", &current_fingerprints);

    assert_eq!(freshness.state, CheckReportFreshnessState::Stale);
    assert_eq!(freshness.stale_fingerprints.len(), 1);
    assert_eq!(
        freshness.stale_fingerprints[0].reason,
        CheckReportStaleReason::ChangedFingerprintSha256
    );
    assert_eq!(
        freshness.stale_fingerprints[0].reference.as_deref(),
        Some("sourceConfig")
    );
}

#[test]
fn freshness_is_stale_when_check_logic_changed() {
    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Passed,
    );
    report.fingerprints = vec![CheckFingerprint::new(
        "live_check_logic",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )];
    let current_fingerprints = vec![CheckFingerprint::new(
        "live_check_logic",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )];

    let freshness =
        evaluate_check_report_freshness(&report, "source-live-check/v2", &current_fingerprints);

    assert_eq!(freshness.state, CheckReportFreshnessState::Stale);
    assert_eq!(freshness.stale_fingerprints.len(), 2);
    assert_eq!(
        freshness.stale_fingerprints[0].reason,
        CheckReportStaleReason::LogicVersionChanged
    );
    assert_eq!(freshness.stale_fingerprints[0].kind, "logic_version");
    assert_eq!(
        freshness.stale_fingerprints[1].reason,
        CheckReportStaleReason::ChangedFingerprintSha256
    );
    assert_eq!(freshness.stale_fingerprints[1].kind, "live_check_logic");
}

#[test]
fn freshness_derives_stale_without_changing_persisted_result() {
    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Failed,
    );
    report.fingerprints = vec![CheckFingerprint::new(
        "source_document",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )];
    let current_fingerprints = vec![CheckFingerprint::new(
        "source_document",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )];

    let freshness =
        evaluate_check_report_freshness(&report, "source-live-check/v1", &current_fingerprints);
    let serialized = serde_json::to_value(&report).unwrap();

    assert_eq!(freshness.state, CheckReportFreshnessState::Stale);
    assert_eq!(report.result, CheckReportResult::Failed);
    assert_eq!(serialized["result"], "failed");
    assert!(serialized.get("stale").is_none());
}

#[test]
fn latest_report_paths_use_overwriteable_source_live_check_location() {
    let app_data_dir = std::path::PathBuf::from("/tmp/job-radar-check-report-test");

    assert_eq!(
        source_live_check_report_path(&app_data_dir, "acme_jobs"),
        app_data_dir.join("source-live-checks/acme_jobs.json")
    );

    let source_report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Failed,
    );
    assert_eq!(
        latest_check_report_path(&app_data_dir, &source_report).unwrap(),
        app_data_dir.join("source-live-checks/acme_jobs.json")
    );
}

#[test]
fn persistence_overwrites_latest_source_live_check_report() {
    let temp_dir = tempfile::tempdir().unwrap();
    let app_data_dir = temp_dir.path();

    let first = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:00:00Z",
        "source-live-check/v1",
        CheckReportResult::Failed,
    );
    let path = persist_latest_check_report(app_data_dir, &first).unwrap();
    assert_eq!(
        read_latest_check_report(&path).unwrap().result,
        CheckReportResult::Failed
    );

    let second = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source("acme_jobs"),
        "2026-07-07T12:05:00Z",
        "source-live-check/v1",
        CheckReportResult::Passed,
    );
    let overwritten_path = persist_latest_check_report(app_data_dir, &second).unwrap();

    assert_eq!(overwritten_path, path);
    assert_eq!(read_latest_check_report(&path).unwrap(), second);
}
