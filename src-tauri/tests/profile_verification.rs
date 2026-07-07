use std::fs;

use job_radar_lib::{
    read_latest_check_report, source_profile_verification_report_path, verify_source_profile,
    CheckReportKind, CheckReportResult, CheckReportSubjectType, DiagnosticCategory,
    DiagnosticSeverity, PROFILE_VERIFICATION_LOGIC_VERSION,
};
use serde_json::json;

const SIMPLE_PROFILE: &str =
    include_str!("fixtures/source-profile-dsl/valid/simple-source-profile.json");

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
