use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::profile_dsl::compiler::ProfileCompilerSnapshot;
use crate::profile_dsl::diagnostics::{DiagnosticSeverity, Diagnostics};
use crate::profile_dsl::documents::JsonObject;
use crate::source::documents::{SelectedAccessPath, SourceDocument};
use crate::source::validation::derive_source_validation_state;
use crate::source_profile::documents::SourceProfileDocument;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

use super::{
    persist_latest_check_report, CheckFingerprint, CheckReport, CheckReportKind, CheckReportResult,
    CheckReportSubject,
};

pub const SOURCE_LIVE_CHECK_LOGIC_VERSION: &str = "source-live-check/v1";

pub fn check_source(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> Result<CheckReport, String> {
    let app_data_dir = app_data_dir.as_ref();
    let source_key = source_key.as_ref();
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    let report = build_source_live_check_report(&snapshot, source_key)?;
    persist_latest_check_report(app_data_dir, &report).map_err(|error| error.to_string())?;
    Ok(report)
}

pub(crate) fn build_source_live_check_report(
    snapshot: &SourceProfileRegistrySnapshot,
    source_key: &str,
) -> Result<CheckReport, String> {
    let compiler_snapshot = compiler_snapshot_from_registry(snapshot);
    let validation_state = derive_source_validation_state(&compiler_snapshot, source_key);
    let diagnostics = validation_state.diagnostics;
    let mut fingerprints = vec![live_check_logic_fingerprint()];
    let mut details = source_live_check_details_placeholders();

    if let Some(source) = snapshot.source(source_key) {
        let source_document = &source.document;
        details.insert(
            "sourceStatusAtCheck".to_string(),
            serde_json::to_value(source_document.status).map_err(|error| {
                format!("Source Status could not be serialized for Source Live Check: {error}")
            })?,
        );
        details.insert(
            "accessPathKey".to_string(),
            serde_json::Value::String(selected_access_path_key(
                &source_document.selected_access_path,
            )),
        );

        fingerprints.push(source_document_fingerprint(source_document)?);
        fingerprints.push(json_fingerprint(
            "source_config",
            &source_document.source_config,
        )?);
        if let Some(source_overrides) = &source_document.source_overrides {
            fingerprints.push(json_fingerprint("source_overrides", source_overrides)?);
        }
        if let SelectedAccessPath::ProfileAccessPath { profile_key, .. } =
            &source_document.selected_access_path
        {
            if let Some(profile) = snapshot.profile(profile_key) {
                fingerprints.push(source_profile_document_fingerprint(&profile.document)?);
            }
        }
    }

    let result = if has_error_diagnostics(&diagnostics) {
        CheckReportResult::Failed
    } else {
        CheckReportResult::Passed
    };
    details.insert(
        "liveCheckState".to_string(),
        serde_json::Value::String(match result {
            CheckReportResult::Passed => "live_check_passed".to_string(),
            CheckReportResult::Failed => "live_check_failed".to_string(),
        }),
    );

    let mut report = CheckReport::new(
        CheckReportKind::SourceLiveCheck,
        CheckReportSubject::source(source_key),
        current_utc_timestamp(),
        SOURCE_LIVE_CHECK_LOGIC_VERSION,
        result,
    );
    report.fingerprints = fingerprints;
    report.diagnostics = diagnostics;
    report.details = details;
    Ok(report)
}

fn compiler_snapshot_from_registry(
    snapshot: &SourceProfileRegistrySnapshot,
) -> ProfileCompilerSnapshot {
    ProfileCompilerSnapshot {
        profiles: snapshot
            .profiles
            .iter()
            .map(|profile| profile.document.clone())
            .collect(),
        sources: snapshot
            .sources
            .iter()
            .map(|source| source.document.clone())
            .collect(),
    }
}

fn source_live_check_details_placeholders() -> JsonObject {
    let mut details = JsonObject::new();
    details.insert("sourceStatusAtCheck".to_string(), serde_json::Value::Null);
    details.insert("liveCheckState".to_string(), serde_json::Value::Null);
    details.insert("accessPathKey".to_string(), serde_json::Value::Null);
    details.insert("candidateCount".to_string(), serde_json::Value::Null);
    details.insert("detailChecked".to_string(), serde_json::Value::Bool(false));
    details.insert("detailPassed".to_string(), serde_json::Value::Null);
    details
}

fn selected_access_path_key(selected_access_path: &SelectedAccessPath) -> String {
    match selected_access_path {
        SelectedAccessPath::ProfileAccessPath { path_key, .. } => path_key.clone(),
        SelectedAccessPath::SourceOwnedAccessPath { key, .. } => key.clone(),
    }
}

fn has_error_diagnostics(diagnostics: &Diagnostics) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn source_document_fingerprint(source: &SourceDocument) -> Result<CheckFingerprint, String> {
    json_fingerprint("source_document", source)
}

fn source_profile_document_fingerprint(
    profile: &SourceProfileDocument,
) -> Result<CheckFingerprint, String> {
    json_fingerprint("source_profile_document", profile)
}

fn live_check_logic_fingerprint() -> CheckFingerprint {
    CheckFingerprint::new(
        "live_check_logic",
        sha256_hex(SOURCE_LIVE_CHECK_LOGIC_VERSION.as_bytes()),
    )
}

fn json_fingerprint<T>(kind: &str, value: &T) -> Result<CheckFingerprint, String>
where
    T: serde::Serialize,
{
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("{kind} could not be fingerprinted: {error}"))?;
    Ok(CheckFingerprint::new(kind, sha256_hex(&bytes)))
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
    let month_parameter = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_parameter + 2) / 5 + 1;
    let month = month_parameter + if month_parameter < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
