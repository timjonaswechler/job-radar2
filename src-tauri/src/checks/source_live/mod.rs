mod activation;

use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::profile_dsl::compiler::{compile_source_execution_plan, ProfileCompilerSnapshot};
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::JsonObject;
use crate::profile_dsl::execution_plan::SourceExecutionPlan;
use crate::profile_dsl::runtime::{
    execute_posting_detail_with_clients, execute_posting_discovery_with_clients,
    PostingDetailExecutionResult, PostingDetailFetcher, PostingDetailPostingOccurrence,
    PostingDiscoveryCandidate, PostingDiscoveryFetcher, ProfileBrowserClient,
    ReqwestPostingDetailFetcher, ReqwestPostingDiscoveryFetcher, UnavailableProfileBrowserClient,
};
use crate::source::documents::{SelectedAccessPath, SourceDocument};
use crate::source::validation::derive_source_validation_state;
use crate::source_profile::documents::SourceProfileDocument;
use crate::source_profile::registry::SourceProfileRegistrySnapshot;

use super::{
    evaluate_check_report_freshness, persist_latest_check_report, read_latest_check_report,
    source_live_check_report_path, CheckFingerprint, CheckReport, CheckReportFreshness,
    CheckReportFreshnessState, CheckReportKind, CheckReportPersistenceError, CheckReportResult,
    CheckReportSubject,
};

pub use activation::{
    check_and_activate_source, check_and_activate_source_with_clients,
    check_and_activate_source_with_fetcher, check_and_reactivate_source,
    check_and_reactivate_source_with_clients, check_and_reactivate_source_with_fetcher,
};

pub const SOURCE_LIVE_CHECK_LOGIC_VERSION: &str = "source-live-check/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceLiveCheckReportState {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLiveCheckReportStatus {
    pub state: SourceLiveCheckReportState,
    pub report: Option<CheckReport>,
    pub freshness: Option<CheckReportFreshness>,
}

pub fn check_source(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> Result<CheckReport, String> {
    check_source_with_clients(
        app_data_dir,
        source_key,
        &ReqwestPostingDiscoveryFetcher::new(),
        &ReqwestPostingDetailFetcher::new(),
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_source_with_fetcher<F>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    fetcher: &F,
) -> Result<CheckReport, String>
where
    F: PostingDiscoveryFetcher + PostingDetailFetcher + Sync + ?Sized,
{
    check_source_with_clients(
        app_data_dir,
        source_key,
        fetcher,
        fetcher,
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_source_with_clients<D, T, B>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    browser: &B,
) -> Result<CheckReport, String>
where
    D: PostingDiscoveryFetcher + Sync + ?Sized,
    T: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let app_data_dir = app_data_dir.as_ref();
    let source_key = source_key.as_ref();
    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    let report = build_source_live_check_report(
        &snapshot,
        source_key,
        discovery_fetcher,
        detail_fetcher,
        browser,
    )?;
    persist_latest_check_report(app_data_dir, &report).map_err(|error| error.to_string())?;
    Ok(report)
}

pub fn source_live_check_report_status(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
) -> Result<SourceLiveCheckReportStatus, String> {
    let app_data_dir = app_data_dir.as_ref();
    let source_key = source_key.as_ref();
    let report_path = source_live_check_report_path(app_data_dir, source_key);
    let report = match read_latest_check_report(&report_path) {
        Ok(report) => report,
        Err(CheckReportPersistenceError::Io(error)) if error.kind() == ErrorKind::NotFound => {
            return Ok(SourceLiveCheckReportStatus {
                state: SourceLiveCheckReportState::Unknown,
                report: None,
                freshness: None,
            });
        }
        Err(error) => return Err(error.to_string()),
    };

    let snapshot = crate::source_profile::registry::load_snapshot(app_data_dir);
    let current_fingerprints = source_live_check_fingerprints(&snapshot, source_key)?;
    let freshness = evaluate_check_report_freshness(
        &report,
        SOURCE_LIVE_CHECK_LOGIC_VERSION,
        &current_fingerprints,
    );
    let state = match freshness.state {
        CheckReportFreshnessState::Fresh => SourceLiveCheckReportState::Fresh,
        CheckReportFreshnessState::Stale => SourceLiveCheckReportState::Stale,
    };

    Ok(SourceLiveCheckReportStatus {
        state,
        report: Some(report),
        freshness: Some(freshness),
    })
}

pub(crate) fn build_source_live_check_report<D, T, B>(
    snapshot: &SourceProfileRegistrySnapshot,
    source_key: &str,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    browser: &B,
) -> Result<CheckReport, String>
where
    D: PostingDiscoveryFetcher + Sync + ?Sized,
    T: PostingDetailFetcher + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let compiler_snapshot = compiler_snapshot_from_registry(snapshot);
    let validation_state = derive_source_validation_state(&compiler_snapshot, source_key);
    let can_compile = validation_state.can_compile;
    let mut diagnostics = validation_state.diagnostics;
    let fingerprints = source_live_check_fingerprints(snapshot, source_key)?;
    let mut details = source_live_check_details_placeholders();
    let mut live_check_subject = None;

    if let Some(source) = snapshot.source(source_key) {
        let source_document = &source.document;
        details.insert(
            "sourceStatusAtCheck".to_string(),
            serde_json::to_value(source_document.status).map_err(|error| {
                format!("Source Status could not be serialized for Source Live Check: {error}")
            })?,
        );
        let subject = SourceLiveCheckSubject::from_selected_access_path(
            source_key,
            &source_document.selected_access_path,
        );
        details.insert(
            "accessPathKey".to_string(),
            serde_json::Value::String(subject.access_path_key.clone()),
        );
        live_check_subject = Some(subject);
    }

    if can_compile {
        if let Some(execution_plan) =
            compile_live_check_execution_plan(&compiler_snapshot, source_key)
        {
            let discovery_result = tauri::async_runtime::block_on(
                execute_posting_discovery_with_clients(&execution_plan, discovery_fetcher, browser),
            );
            let candidate_count = discovery_result.candidates.len();
            let first_acceptable_candidate = discovery_result
                .candidates
                .iter()
                .find(|candidate| is_acceptable_live_candidate(candidate));
            let acceptable_candidate_count = discovery_result
                .candidates
                .iter()
                .filter(|candidate| is_acceptable_live_candidate(candidate))
                .count();
            details.insert(
                "candidateCount".to_string(),
                serde_json::json!(candidate_count),
            );
            diagnostics.extend(discovery_result.diagnostics);

            if acceptable_candidate_count == 0 {
                diagnostics.push(no_candidates_diagnostic(
                    live_check_subject.as_ref(),
                    candidate_count,
                    acceptable_candidate_count,
                ));
            } else if execution_plan.posting_detail.is_some() {
                if let Some(candidate) = first_acceptable_candidate {
                    details.insert("detailChecked".to_string(), serde_json::Value::Bool(true));
                    let posting = posting_detail_occurrence_from_candidate(candidate);
                    let detail_result =
                        tauri::async_runtime::block_on(execute_posting_detail_with_clients(
                            &execution_plan,
                            &posting,
                            detail_fetcher,
                            browser,
                        ));
                    let detail_passed = is_acceptable_detail_result(&detail_result);
                    details.insert(
                        "detailPassed".to_string(),
                        serde_json::Value::Bool(detail_passed),
                    );
                    let detail_failure_cause = if detail_passed {
                        None
                    } else {
                        Some(detail_failure_cause(&detail_result))
                    };
                    diagnostics.extend(detail_result.diagnostics);
                    if let Some(cause) = detail_failure_cause {
                        diagnostics.push(detail_failed_diagnostic(
                            live_check_subject.as_ref(),
                            &candidate.url,
                            &cause,
                        ));
                    }
                }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLiveCheckSubject {
    source_key: String,
    profile_key: Option<String>,
    access_path_key: String,
}

impl SourceLiveCheckSubject {
    fn from_selected_access_path(
        source_key: &str,
        selected_access_path: &SelectedAccessPath,
    ) -> Self {
        match selected_access_path {
            SelectedAccessPath::ProfileAccessPath {
                profile_key,
                path_key,
            } => Self {
                source_key: source_key.to_string(),
                profile_key: Some(profile_key.clone()),
                access_path_key: path_key.clone(),
            },
            SelectedAccessPath::SourceOwnedAccessPath { key, .. } => Self {
                source_key: source_key.to_string(),
                profile_key: None,
                access_path_key: key.clone(),
            },
        }
    }
}

fn source_live_check_fingerprints(
    snapshot: &SourceProfileRegistrySnapshot,
    source_key: &str,
) -> Result<Vec<CheckFingerprint>, String> {
    let mut fingerprints = vec![live_check_logic_fingerprint()];

    if let Some(source) = snapshot.source(source_key) {
        let source_document = &source.document;
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

    Ok(fingerprints)
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

fn compile_live_check_execution_plan(
    snapshot: &ProfileCompilerSnapshot,
    source_key: &str,
) -> Option<SourceExecutionPlan> {
    let mut live_check_snapshot = snapshot.clone();
    if let Some(source) = live_check_snapshot
        .sources
        .iter_mut()
        .find(|source| source.key == source_key)
    {
        source.status = crate::source::documents::SourceStatus::Active;
    }
    compile_source_execution_plan(&live_check_snapshot, source_key).execution_plan
}

fn is_acceptable_live_candidate(candidate: &PostingDiscoveryCandidate) -> bool {
    !candidate.title.trim().is_empty()
        && !candidate.company.trim().is_empty()
        && !candidate.url.trim().is_empty()
}

fn posting_detail_occurrence_from_candidate(
    candidate: &PostingDiscoveryCandidate,
) -> PostingDetailPostingOccurrence {
    PostingDetailPostingOccurrence {
        url: candidate.url.clone(),
        title: Some(candidate.title.clone()),
        company: Some(candidate.company.clone()),
        locations: candidate.locations.clone(),
        description_text: candidate.description_text.clone(),
        posting_meta: candidate.posting_meta.clone(),
    }
}

fn is_acceptable_detail_result(result: &PostingDetailExecutionResult) -> bool {
    !has_error_diagnostics(&result.diagnostics)
        && result
            .description_text
            .as_ref()
            .is_some_and(|description_text| !description_text.trim().is_empty())
}

fn detail_failure_cause(result: &PostingDetailExecutionResult) -> String {
    result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code.clone())
        .unwrap_or_else(|| "posting_detail_description_text_missing".to_string())
}

fn no_candidates_diagnostic(
    subject: Option<&SourceLiveCheckSubject>,
    candidate_count: usize,
    acceptable_candidate_count: usize,
) -> Diagnostic {
    let (source_key, profile_key, access_path_key) = subject
        .map(|subject| {
            (
                subject.source_key.clone(),
                subject
                    .profile_key
                    .as_ref()
                    .map_or(serde_json::Value::Null, |profile_key| {
                        serde_json::Value::String(profile_key.clone())
                    }),
                serde_json::Value::String(subject.access_path_key.clone()),
            )
        })
        .unwrap_or_else(|| {
            (
                String::new(),
                serde_json::Value::Null,
                serde_json::Value::Null,
            )
        });

    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_live_check.no_candidates".to_string(),
        message: "Source Live Check discovery returned no acceptable posting candidates"
            .to_string(),
        severity: DiagnosticSeverity::Error,
        path: "/postingDiscovery".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "sourceKey": source_key,
            "profileKey": profile_key,
            "accessPathKey": access_path_key,
            "candidateCount": candidate_count,
            "acceptableCandidateCount": acceptable_candidate_count,
            "requiredFields": ["title", "company", "url"]
        })),
    }
}

fn detail_failed_diagnostic(
    subject: Option<&SourceLiveCheckSubject>,
    candidate_url: &str,
    cause: &str,
) -> Diagnostic {
    let (source_key, profile_key, access_path_key) = subject
        .map(|subject| {
            (
                subject.source_key.clone(),
                subject
                    .profile_key
                    .as_ref()
                    .map_or(serde_json::Value::Null, |profile_key| {
                        serde_json::Value::String(profile_key.clone())
                    }),
                serde_json::Value::String(subject.access_path_key.clone()),
            )
        })
        .unwrap_or_else(|| {
            (
                String::new(),
                serde_json::Value::Null,
                serde_json::Value::Null,
            )
        });

    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "source_live_check.detail_failed".to_string(),
        message: "Source Live Check postingDetail failed for the selected candidate".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "/postingDetail".to_string(),
        strategy_key: None,
        details: Some(serde_json::json!({
            "sourceKey": source_key,
            "profileKey": profile_key,
            "accessPathKey": access_path_key,
            "candidateUrl": candidate_url,
            "cause": cause
        })),
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
