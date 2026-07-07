use std::{fs, path::Path};

use job_radar_lib::{
    read_latest_check_report, source_profile_verification_report_path, verify_source_profile,
    CheckReportKind, CheckReportResult, CheckReportSubjectType, DiagnosticCategory,
    DiagnosticSeverity, PROFILE_VERIFICATION_LOGIC_VERSION,
};
use serde_json::json;

const SIMPLE_PROFILE: &str =
    include_str!("fixtures/source-profile-dsl/valid/simple-source-profile.json");

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

fn profile_with_fixture_evidence() -> serde_json::Value {
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["support"] = json!({
        "level": "verified",
        "summary": "Fixture backed.",
        "evidence": [{ "kind": "fixture", "reference": "fixture.json" }]
    });
    profile
}

fn profile_with_fixture_discovery_url(fetch_url: &str) -> serde_json::Value {
    let mut profile = profile_with_fixture_evidence();
    profile["accessPaths"][0]["postingDiscovery"]["strategies"][0]["fetch"]["url"] =
        json!(fetch_url);
    profile["accessPaths"][0]["postingDiscovery"]["strategies"][0]
        .as_object_mut()
        .unwrap()
        .remove("pagination");
    profile
}

fn write_fixture_manifest(app_data_dir: &Path, profile_key: &str, manifest: serde_json::Value) {
    write_raw_fixture_manifest(
        app_data_dir,
        profile_key,
        &serde_json::to_string_pretty(&manifest).unwrap(),
    );
}

fn write_raw_fixture_manifest(app_data_dir: &Path, profile_key: &str, contents: &str) {
    let fixture_dir = app_data_dir
        .join("source-profile-fixtures")
        .join(profile_key);
    fs::create_dir_all(&fixture_dir).unwrap();
    fs::write(fixture_dir.join("fixture.json"), contents).unwrap();
}

fn write_fixture_file(app_data_dir: &Path, profile_key: &str, reference: &str, contents: &str) {
    let path = app_data_dir
        .join("source-profile-fixtures")
        .join(profile_key)
        .join(reference);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn discovery_body() -> &'static str {
    r#"{
        "jobs": [
            {
                "id": "job-1",
                "title": " Software Engineer ",
                "url": "https://example.test/jobs/job-1",
                "locations": ["Berlin"]
            }
        ]
    }"#
}

fn representative_manifest(
    profile_key: &str,
    access_path_key: &str,
    source_config: serde_json::Value,
) -> serde_json::Value {
    representative_manifest_with_request_url(
        profile_key,
        access_path_key,
        source_config,
        "https://example.test/jobs.json",
    )
}

fn representative_manifest_with_request_url(
    profile_key: &str,
    access_path_key: &str,
    source_config: serde_json::Value,
    request_url: &str,
) -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "profileKey": profile_key,
        "accessPathKey": access_path_key,
        "sourceConfig": source_config,
        "requests": [{
            "key": "discovery_jobs",
            "match": {
                "method": "GET",
                "url": request_url
            },
            "response": {
                "status": 200,
                "headers": { "content-type": "application/json" },
                "bodyFile": "responses/jobs.json"
            }
        }],
        "checks": {
            "postingDiscovery": {
                "expect": {
                    "minCandidates": 1,
                    "requiredFields": ["title", "company", "url"]
                }
            }
        }
    })
}

#[test]
fn verify_source_profile_creates_and_persists_passed_report_for_valid_profile() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile_dir = temp_dir.path().join("source-profiles");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(profile_dir.join("example_jobs.json"), SIMPLE_PROFILE).unwrap();

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.kind, CheckReportKind::SourceProfileVerification);
    assert_eq!(
        report.subject.subject_type,
        CheckReportSubjectType::SourceProfile
    );
    assert_eq!(report.subject.key, "example_jobs");
    assert_eq!(report.logic_version, PROFILE_VERIFICATION_LOGIC_VERSION);
    assert_eq!(report.result, CheckReportResult::Passed);
    assert!(report.diagnostics.is_empty());
    assert!(report
        .fingerprints
        .iter()
        .any(|fingerprint| fingerprint.kind == "source_profile_document"));
    assert!(report
        .fingerprints
        .iter()
        .any(|fingerprint| fingerprint.kind == "verification_logic"));
    assert_eq!(
        report.details.get("declaredSupportLevel"),
        Some(&json!("experimental"))
    );
    assert_eq!(
        report.details.get("effectiveVerificationState"),
        Some(&json!("unknown"))
    );
    assert_eq!(report.details.get("fixtureChecks"), Some(&json!([])));

    let path = source_profile_verification_report_path(temp_dir.path(), "example_jobs");
    assert!(
        path.exists(),
        "expected persisted report at {}",
        path.display()
    );
    let persisted = read_latest_check_report(&path).unwrap();
    assert_eq!(persisted, report);
}

#[test]
fn verify_source_profile_wires_valid_fixture_manifest_evidence_into_report_details() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile_dir = temp_dir.path().join("source-profiles");
    fs::create_dir_all(&profile_dir).unwrap();

    let profile = profile_with_fixture_discovery_url("https://example.test/jobs.json");
    fs::write(
        profile_dir.join("example_jobs.json"),
        serde_json::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest(
            "example_jobs",
            "json_feed",
            json!({
                "feedUrl": "https://example.test/jobs.json",
                "language": "de"
            }),
        ),
    );
    write_fixture_file(
        temp_dir.path(),
        "example_jobs",
        "responses/jobs.json",
        discovery_body(),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert!(report.diagnostics.is_empty());
    assert!(report.fingerprints.iter().any(|fingerprint| {
        fingerprint.kind == "fixture_manifest"
            && fingerprint.reference.as_deref() == Some("fixture.json")
    }));
    assert!(report.fingerprints.iter().any(|fingerprint| {
        fingerprint.kind == "fixture_file"
            && fingerprint.reference.as_deref() == Some("responses/jobs.json")
    }));
    assert_eq!(
        report.details.get("fixtureChecks"),
        Some(&json!([{
            "reference": "fixture.json",
            "result": "passed",
            "accessPathKey": "json_feed",
            "coverage": {
                "postingDiscovery": false,
                "postingDetailDescriptionText": false
            }
        }]))
    );
}

#[test]
fn verify_source_profile_serves_mapped_profile_fetch_from_fixture_file_without_live_network() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_discovery_url("https://127.0.0.1:9/jobs.json");
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest_with_request_url(
            "example_jobs",
            "json_feed",
            json!({ "feedUrl": "https://unused.example.test/from-manifest-source-config" }),
            "https://127.0.0.1:9/jobs.json",
        ),
    );
    write_fixture_file(
        temp_dir.path(),
        "example_jobs",
        "responses/jobs.json",
        discovery_body(),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert!(
        report.diagnostics.is_empty(),
        "expected offline fixture replay to avoid live fetch diagnostics, got {:?}",
        report.diagnostics
    );
}

#[test]
fn verify_source_profile_reports_unmapped_fixture_request_with_details() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_discovery_url("https://fixture.example.test/unmapped.json");
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest_with_request_url(
            "example_jobs",
            "json_feed",
            json!({ "feedUrl": "https://unused.example.test/from-manifest-source-config" }),
            "https://fixture.example.test/mapped.json",
        ),
    );
    write_fixture_file(
        temp_dir.path(),
        "example_jobs",
        "responses/jobs.json",
        discovery_body(),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "fixture.unmapped_request")
        .expect("unmapped request diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Fixture);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.details,
        Some(json!({
            "profileKey": "example_jobs",
            "accessPathKey": "json_feed",
            "method": "GET",
            "url": "https://fixture.example.test/unmapped.json"
        }))
    );
}

#[test]
fn verify_source_profile_matches_fixture_requests_independent_of_query_parameter_order() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_discovery_url("https://fixture.example.test/jobs?b=2&a=1");
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest_with_request_url(
            "example_jobs",
            "json_feed",
            json!({ "feedUrl": "https://unused.example.test/from-manifest-source-config" }),
            "https://fixture.example.test/jobs?a=1&b=2",
        ),
    );
    write_fixture_file(
        temp_dir.path(),
        "example_jobs",
        "responses/jobs.json",
        discovery_body(),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Passed);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn verify_source_profile_reports_missing_and_invalid_fixture_manifests() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_evidence();
    write_profile(temp_dir.path(), &profile);
    fs::create_dir_all(
        temp_dir
            .path()
            .join("source-profile-fixtures")
            .join("example_jobs"),
    )
    .unwrap();

    let missing_report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(missing_report.result, CheckReportResult::Failed);
    assert!(missing_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "fixture.manifest_missing"));
    assert_eq!(
        missing_report.details.get("fixtureChecks"),
        Some(&json!([{
            "reference": "fixture.json",
            "result": "failed",
            "coverage": {
                "postingDiscovery": false,
                "postingDetailDescriptionText": false
            }
        }]))
    );

    write_raw_fixture_manifest(temp_dir.path(), "example_jobs", "{ not json");
    let invalid_report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(invalid_report.result, CheckReportResult::Failed);
    let diagnostic = invalid_report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "fixture.manifest_invalid_json")
        .expect("invalid manifest diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Fixture);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic
            .details
            .as_ref()
            .and_then(|details| details.get("profileKey")),
        Some(&json!("example_jobs"))
    );
}

#[test]
fn verify_source_profile_reports_fixture_profile_key_mismatch() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_evidence();
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest(
            "other_profile",
            "json_feed",
            json!({
                "feedUrl": "https://example.test/jobs.json"
            }),
        ),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "fixture.profile_key_mismatch")
        .expect("profile key mismatch diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Fixture);
    assert_eq!(
        diagnostic.details,
        Some(json!({
            "expectedProfileKey": "example_jobs",
            "actualProfileKey": "other_profile",
            "reference": "fixture.json"
        }))
    );
    assert_eq!(
        report.details["fixtureChecks"][0]["result"],
        json!("failed")
    );
}

#[test]
fn verify_source_profile_reports_fixture_access_path_missing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_evidence();
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest(
            "example_jobs",
            "missing_path",
            json!({
                "feedUrl": "https://example.test/jobs.json"
            }),
        ),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "fixture.access_path_missing")
        .expect("missing access path diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Fixture);
    assert_eq!(
        diagnostic.details,
        Some(json!({
            "profileKey": "example_jobs",
            "accessPathKey": "missing_path",
            "reference": "fixture.json"
        }))
    );
    assert_eq!(
        report.details["fixtureChecks"][0]["accessPathKey"],
        json!("missing_path")
    );
    assert_eq!(
        report.details["fixtureChecks"][0]["result"],
        json!("failed")
    );
}

#[test]
fn verify_source_profile_reports_invalid_fixture_source_config() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile = profile_with_fixture_evidence();
    write_profile(temp_dir.path(), &profile);
    write_fixture_manifest(
        temp_dir.path(),
        "example_jobs",
        representative_manifest(
            "example_jobs",
            "json_feed",
            json!({
                "feedUrl": 42
            }),
        ),
    );

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "fixture.source_config_invalid")
        .expect("invalid sourceConfig diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Fixture);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic
            .details
            .as_ref()
            .and_then(|details| details.get("profileKey")),
        Some(&json!("example_jobs"))
    );
    assert_eq!(
        diagnostic
            .details
            .as_ref()
            .and_then(|details| details.get("accessPathKey")),
        Some(&json!("json_feed"))
    );
    assert_eq!(
        report.details["fixtureChecks"][0]["result"],
        json!("failed")
    );
}

#[test]
fn verify_source_profile_reports_verified_support_without_fixture_evidence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["support"] = json!({ "level": "verified" });
    write_profile(temp_dir.path(), &profile);

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == "verification.verified_support_missing_fixture_evidence"
        })
        .expect("verification missing fixture evidence diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Verification);
    assert_eq!(
        diagnostic.details,
        Some(json!({
            "profileKey": "example_jobs",
            "supportLevel": "verified"
        }))
    );
}

#[test]
fn verify_source_profile_reports_invalid_support_evidence_kind_url() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["support"] = json!({
        "level": "best_effort",
        "evidence": [{ "kind": "url", "reference": "https://example.test/jobs" }]
    });
    write_profile(temp_dir.path(), &profile);

    let report = verify_source_profile(temp_dir.path(), "example_jobs").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "verification.invalid_support_evidence_kind")
        .expect("invalid support evidence kind diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Verification);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic
            .details
            .as_ref()
            .and_then(|details| details.get("kind")),
        Some(&json!("url"))
    );
    assert!(diagnostic
        .details
        .as_ref()
        .and_then(|details| details.get("hint"))
        .and_then(|hint| hint.as_str())
        .is_some_and(|hint| hint.contains("detect.evidence.kind")));
}

#[test]
fn verify_source_profile_persists_failed_report_for_unknown_profile_key() {
    let temp_dir = tempfile::tempdir().unwrap();

    let report = verify_source_profile(temp_dir.path(), "missing_profile").unwrap();

    assert_eq!(report.kind, CheckReportKind::SourceProfileVerification);
    assert_eq!(
        report.subject.subject_type,
        CheckReportSubjectType::SourceProfile
    );
    assert_eq!(report.subject.key, "missing_profile");
    assert_eq!(report.result, CheckReportResult::Failed);
    assert!(report
        .fingerprints
        .iter()
        .any(|fingerprint| fingerprint.kind == "verification_logic"));
    assert!(!report
        .fingerprints
        .iter()
        .any(|fingerprint| fingerprint.kind == "source_profile_document"));
    let diagnostic = report
        .diagnostics
        .first()
        .expect("unknown profile diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Verification);
    assert_eq!(diagnostic.code, "verification.source_profile_not_found");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.details,
        Some(json!({ "profileKey": "missing_profile" }))
    );

    let path = source_profile_verification_report_path(temp_dir.path(), "missing_profile");
    let persisted = read_latest_check_report(&path).unwrap();
    assert_eq!(persisted, report);
}

#[test]
fn verify_source_profile_fails_when_available_profile_validation_reports_errors() {
    let temp_dir = tempfile::tempdir().unwrap();
    let profile_dir = temp_dir.path().join("source-profiles");
    fs::create_dir_all(&profile_dir).unwrap();

    let mut profile: serde_json::Value = serde_json::from_str(SIMPLE_PROFILE).unwrap();
    profile["key"] = json!("invalid_verified");
    profile["name"] = json!("Invalid Verified");
    profile["support"] = json!({ "level": "verified" });
    fs::write(
        profile_dir.join("invalid_verified.json"),
        serde_json::to_string_pretty(&profile).unwrap(),
    )
    .unwrap();

    let report = verify_source_profile(temp_dir.path(), "invalid_verified").unwrap();

    assert_eq!(report.result, CheckReportResult::Failed);
    assert!(report
        .fingerprints
        .iter()
        .any(|fingerprint| fingerprint.kind == "source_profile_document"));
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "verified_support_missing_fixture_evidence")
        .expect("support validation diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Compiler);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(
        diagnostic.details,
        Some(json!({ "sourceProfileKey": "invalid_verified" }))
    );

    let path = source_profile_verification_report_path(temp_dir.path(), "invalid_verified");
    let persisted = read_latest_check_report(&path).unwrap();
    assert_eq!(persisted, report);
}
