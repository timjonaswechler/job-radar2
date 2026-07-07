use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::{
    JsonObject, SupportEvidence, SupportEvidenceKind, SupportLevel,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument, SourceStatus};
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
    let report = build_source_profile_verification_report(app_data_dir, &snapshot, profile_key)?;
    persist_latest_check_report(app_data_dir, &report).map_err(|error| error.to_string())?;
    Ok(report)
}

pub(crate) fn build_source_profile_verification_report(
    app_data_dir: &Path,
    snapshot: &SourceProfileRegistrySnapshot,
    profile_key: &str,
) -> Result<CheckReport, String> {
    let mut diagnostics = profile_registry_diagnostics(snapshot, profile_key);
    diagnostics.extend(invalid_support_evidence_kind_diagnostics(
        app_data_dir,
        profile_key,
    ));
    let mut fingerprints = vec![verification_logic_fingerprint()];
    let mut details = JsonObject::new();
    let mut fixture_checks = Vec::new();
    details.insert(
        "effectiveVerificationState".to_string(),
        serde_json::json!("unknown"),
    );

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

        let fixture_evidence = fixture_support_evidence(&profile.document.support.evidence);
        if profile.document.support.level == SupportLevel::Verified && fixture_evidence.is_empty() {
            diagnostics.push(verified_support_missing_fixture_evidence_diagnostic(
                profile_key,
            ));
        }

        for evidence in fixture_evidence {
            let verification = verify_fixture_manifest_evidence(
                app_data_dir,
                &profile.document,
                evidence,
                &mut fingerprints,
            )?;
            diagnostics.extend(verification.diagnostics);
            fixture_checks.push(verification.fixture_check);
        }
    } else {
        diagnostics.push(unknown_source_profile_diagnostic(profile_key));
    }

    details.insert(
        "fixtureChecks".to_string(),
        serde_json::json!(fixture_checks),
    );

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

struct FixtureEvidenceVerification {
    diagnostics: Diagnostics,
    fixture_check: serde_json::Value,
}

fn fixture_support_evidence(evidence: &Option<Vec<SupportEvidence>>) -> Vec<&SupportEvidence> {
    evidence
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry.kind == SupportEvidenceKind::Fixture)
        .collect()
}

fn verify_fixture_manifest_evidence(
    app_data_dir: &Path,
    profile: &crate::source_profile::documents::SourceProfileDocument,
    evidence: &SupportEvidence,
    fingerprints: &mut Vec<CheckFingerprint>,
) -> Result<FixtureEvidenceVerification, String> {
    let reference = evidence.reference.as_str();
    let mut diagnostics = Vec::new();

    let resolution = resolve_fixture_manifest_reference(app_data_dir, &profile.key, reference);
    diagnostics.extend(resolution.diagnostics);
    let Some(manifest_path) = resolution.resolved_path else {
        return Ok(fixture_evidence_verification(reference, None, diagnostics));
    };
    if has_error_diagnostics(&diagnostics) {
        return Ok(fixture_evidence_verification(reference, None, diagnostics));
    }

    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            diagnostics.push(manifest_invalid_json_diagnostic(
                &profile.key,
                reference,
                error.to_string(),
            ));
            return Ok(fixture_evidence_verification(reference, None, diagnostics));
        }
    };
    fingerprints.push(CheckFingerprint::with_reference(
        "fixture_manifest",
        reference,
        sha256_hex(&manifest_bytes),
    ));

    let manifest: FixtureManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            diagnostics.push(manifest_invalid_json_diagnostic(
                &profile.key,
                reference,
                error.to_string(),
            ));
            return Ok(fixture_evidence_verification(reference, None, diagnostics));
        }
    };
    let access_path_key = manifest.access_path_key.as_str();

    if manifest.profile_key != profile.key {
        diagnostics.push(profile_key_mismatch_diagnostic(
            &profile.key,
            &manifest.profile_key,
            reference,
        ));
    }

    if !profile
        .access_paths
        .iter()
        .any(|access_path| access_path.key == manifest.access_path_key)
    {
        diagnostics.push(access_path_missing_diagnostic(
            &profile.key,
            &manifest.access_path_key,
            reference,
        ));
    } else if let Some(diagnostic) =
        source_config_invalid_diagnostic(profile, &manifest, reference)?
    {
        diagnostics.push(diagnostic);
    }

    Ok(fixture_evidence_verification(
        reference,
        Some(access_path_key),
        diagnostics,
    ))
}

fn source_config_invalid_diagnostic(
    profile: &crate::source_profile::documents::SourceProfileDocument,
    manifest: &FixtureManifest,
    reference: &str,
) -> Result<Option<Diagnostic>, String> {
    let source_key = format!("{}_fixture_verification", profile.key);
    let source = SourceDocument {
        schema_version: 2,
        key: source_key.clone(),
        name: format!("{} fixture verification", profile.name),
        status: SourceStatus::Active,
        source_config: manifest.source_config.clone(),
        selected_access_path: SelectedAccessPath::ProfileAccessPath {
            profile_key: profile.key.clone(),
            path_key: manifest.access_path_key.clone(),
        },
        source_overrides: None,
        source_support: None,
        diagnostics: None,
    };
    let snapshot = crate::profile_dsl::compiler::ProfileCompilerSnapshot {
        profiles: vec![profile.clone()],
        sources: vec![source],
    };
    let result =
        crate::profile_dsl::compiler::compile_source_execution_plan(&snapshot, &source_key);
    let source_config_diagnostics = result
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.path.starts_with("/sourceConfig"))
        .collect::<Diagnostics>();

    if source_config_diagnostics.is_empty() {
        return Ok(None);
    }

    Ok(Some(Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.source_config_invalid".to_string(),
        message: format!(
            "Fixture Manifest `{reference}` Source Config is invalid for Source Profile `{}` Access Path `{}`",
            profile.key, manifest.access_path_key
        ),
        severity: DiagnosticSeverity::Error,
        path: "/sourceConfig".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "profileKey": profile.key,
            "accessPathKey": manifest.access_path_key,
            "reference": reference,
            "diagnostics": source_config_diagnostics,
        })),
    }))
}

fn fixture_evidence_verification(
    reference: &str,
    access_path_key: Option<&str>,
    diagnostics: Diagnostics,
) -> FixtureEvidenceVerification {
    let mut fixture_check = serde_json::json!({
        "reference": reference,
        "result": if has_error_diagnostics(&diagnostics) { "failed" } else { "passed" },
        "coverage": {
            "postingDiscovery": false,
            "postingDetailDescriptionText": false,
        }
    });
    if let Some(access_path_key) = access_path_key {
        fixture_check["accessPathKey"] = serde_json::json!(access_path_key);
    }

    FixtureEvidenceVerification {
        diagnostics,
        fixture_check,
    }
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

fn invalid_support_evidence_kind_diagnostics(
    app_data_dir: &Path,
    profile_key: &str,
) -> Diagnostics {
    let profile_path = app_data_dir
        .join("source-profiles")
        .join(format!("{profile_key}.json"));
    let Ok(contents) = fs::read_to_string(profile_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return Vec::new();
    };

    value
        .pointer("/support/evidence")
        .and_then(|evidence| evidence.as_array())
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, evidence)| {
            let kind = evidence.get("kind").and_then(|kind| kind.as_str())?;
            if matches!(kind, "fixture" | "smoke" | "manual_review" | "schema_check") {
                return None;
            }
            Some(Diagnostic {
                category: DiagnosticCategory::Verification,
                code: "verification.invalid_support_evidence_kind".to_string(),
                message: format!(
                    "Support evidence kind `{kind}` is not supported for Source Profile verification"
                ),
                severity: DiagnosticSeverity::Error,
                path: format!("/support/evidence/{index}/kind"),
                strategy_key: None,
                details: Some(serde_json::json!({
                    "profileKey": profile_key,
                    "kind": kind,
                    "allowedKinds": ["fixture", "smoke", "manual_review", "schema_check"],
                    "hint": "`url` is valid only for detect.evidence.kind; use `manual_review` or `fixture` for support evidence."
                })),
            })
        })
        .collect()
}

fn manifest_invalid_json_diagnostic(
    profile_key: &str,
    reference: &str,
    parse_error: String,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.manifest_invalid_json".to_string(),
        message: format!(
            "Fixture Manifest `{reference}` for Source Profile `{profile_key}` is invalid JSON or does not match the manifest type contract"
        ),
        severity: DiagnosticSeverity::Error,
        path: "".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "profileKey": profile_key,
            "reference": reference,
            "parseError": parse_error,
        })),
    }
}

fn profile_key_mismatch_diagnostic(
    expected_profile_key: &str,
    actual_profile_key: &str,
    reference: &str,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.profile_key_mismatch".to_string(),
        message: format!(
            "Fixture Manifest `{reference}` profileKey `{actual_profile_key}` does not match checked Source Profile `{expected_profile_key}`"
        ),
        severity: DiagnosticSeverity::Error,
        path: "/profileKey".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "expectedProfileKey": expected_profile_key,
            "actualProfileKey": actual_profile_key,
            "reference": reference,
        })),
    }
}

fn access_path_missing_diagnostic(
    profile_key: &str,
    access_path_key: &str,
    reference: &str,
) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Fixture,
        code: "fixture.access_path_missing".to_string(),
        message: format!(
            "Fixture Manifest `{reference}` references missing Access Path `{access_path_key}` on Source Profile `{profile_key}`"
        ),
        severity: DiagnosticSeverity::Error,
        path: "/accessPathKey".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "profileKey": profile_key,
            "accessPathKey": access_path_key,
            "reference": reference,
        })),
    }
}

fn verified_support_missing_fixture_evidence_diagnostic(profile_key: &str) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Verification,
        code: "verification.verified_support_missing_fixture_evidence".to_string(),
        message: format!(
            "Source Profile `{profile_key}` declares verified support but has no fixture evidence"
        ),
        severity: DiagnosticSeverity::Error,
        path: "/support/evidence".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "profileKey": profile_key,
            "supportLevel": "verified",
        })),
    }
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
