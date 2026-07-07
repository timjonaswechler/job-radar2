use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::JsonObject;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

use super::{
    persist_latest_check_report, CheckFingerprint, CheckReport, CheckReportKind, CheckReportResult,
    CheckReportSubject,
};

pub(crate) mod fixture_manifest;
pub(crate) mod fixture_pack;

pub const PROFILE_VERIFICATION_LOGIC_VERSION: &str = "profile-verification/v1";

pub fn verify_source_profile(
    app_data_dir: impl AsRef<Path>,
    profile_key: impl AsRef<str>,
) -> Result<CheckReport, String> {
    let app_data_dir = app_data_dir.as_ref();
    let profile_key = profile_key.as_ref();
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    let report = build_source_profile_verification_report(&snapshot, profile_key)?;
    persist_latest_check_report(app_data_dir, &report).map_err(|error| error.to_string())?;
    Ok(report)
}

pub(crate) fn build_source_profile_verification_report(
    snapshot: &SourceProfileRegistrySnapshot,
    profile_key: &str,
) -> Result<CheckReport, String> {
    let mut diagnostics = profile_registry_diagnostics(snapshot, profile_key);
    let mut fingerprints = vec![verification_logic_fingerprint()];
    let mut details = JsonObject::new();
    details.insert(
        "effectiveVerificationState".to_string(),
        serde_json::json!("unknown"),
    );
    details.insert("fixtureChecks".to_string(), serde_json::json!([]));

    if let Some(profile) = snapshot.profile(profile_key) {
        details.insert(
            "declaredSupportLevel".to_string(),
            serde_json::to_value(profile.document.support.level)
                .map_err(|error| format!("Support Level could not be serialized: {error}"))?,
        );
        fingerprints.push(source_profile_document_fingerprint(&profile.document)?);
        diagnostics.extend(
            crate::profile_dsl::compiler::validate_source_profile_document(&profile.document),
        );
    } else {
        diagnostics.push(unknown_source_profile_diagnostic(profile_key));
    }

    let result = if has_error_diagnostics(&diagnostics) {
        CheckReportResult::Failed
    } else {
        CheckReportResult::Passed
    };

    let mut report = CheckReport::new(
        CheckReportKind::SourceProfileVerification,
        CheckReportSubject::source_profile(profile_key),
        current_utc_timestamp(),
        PROFILE_VERIFICATION_LOGIC_VERSION,
        result,
    );
    report.fingerprints = fingerprints;
    report.diagnostics = diagnostics;
    report.details = details;
    Ok(report)
}

fn profile_registry_diagnostics(
    snapshot: &SourceProfileRegistrySnapshot,
    profile_key: &str,
) -> Diagnostics {
    snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.category,
                DiagnosticCategory::Schema | DiagnosticCategory::Registry
            ) && diagnostic_details_match_profile_key(diagnostic, profile_key)
        })
        .cloned()
        .collect()
}

fn diagnostic_details_match_profile_key(diagnostic: &Diagnostic, profile_key: &str) -> bool {
    let Some(details) = diagnostic.details.as_ref() else {
        return false;
    };

    ["sourceProfileKey", "profileKey", "key"]
        .iter()
        .any(|field| details.get(field).and_then(|value| value.as_str()) == Some(profile_key))
        || details
            .get("path")
            .and_then(|value| value.as_str())
            .is_some_and(|path| {
                Path::new(path).file_stem().and_then(|stem| stem.to_str()) == Some(profile_key)
            })
}

fn unknown_source_profile_diagnostic(profile_key: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Verification,
        code: "verification.source_profile_not_found".to_string(),
        message: format!("Source Profile `{profile_key}` was not found"),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({ "profileKey": profile_key })),
    }
}

fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn source_profile_document_fingerprint(
    profile: &crate::source_profile::documents::SourceProfileDocument,
) -> Result<CheckFingerprint, String> {
    let bytes = serde_json::to_vec(profile)
        .map_err(|error| format!("Source Profile document could not be fingerprinted: {error}"))?;
    Ok(CheckFingerprint::new(
        "source_profile_document",
        sha256_hex(&bytes),
    ))
}

fn verification_logic_fingerprint() -> CheckFingerprint {
    CheckFingerprint::new(
        "verification_logic",
        sha256_hex(PROFILE_VERIFICATION_LOGIC_VERSION.as_bytes()),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn current_utc_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    format_unix_timestamp(seconds)
}

fn format_unix_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_phase = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_phase + 2) / 5 + 1;
    let month = month_phase + if month_phase < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

pub use fixture_manifest::{
    FixtureManifest, FixtureManifestChecks, FixtureManifestDiscoveryExpect,
    FixtureManifestExpectedCandidate, FixtureManifestPostingDetailCase,
    FixtureManifestPostingDetailCheck, FixtureManifestPostingDetailExpect,
    FixtureManifestPostingDiscoveryCheck, FixtureManifestPostingField, FixtureManifestPostingInput,
    FixtureManifestRequestMapping, FixtureManifestRequestMatch, FixtureManifestRequestMethod,
    FixtureManifestResponse, FIXTURE_MANIFEST_SCHEMA_VERSION,
};
pub use fixture_pack::{
    fixture_pack_root, resolve_fixture_file_reference, resolve_fixture_manifest_reference,
    FixturePathResolution, DEFAULT_FIXTURE_MANIFEST_REFERENCE, SOURCE_PROFILE_FIXTURES_DIR,
};
