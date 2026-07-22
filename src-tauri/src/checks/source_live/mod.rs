mod activation;

use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::checks::prepare_source_behavior_fingerprints;
use crate::profile_dsl::compiler::{CompileSourceOutcome, CompiledSource};
use crate::profile_dsl::diagnostics::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
};
use crate::profile_dsl::documents::{JsonObject, PhaseLimits};
use crate::profile_dsl::runtime::{
    execute_discovery, DetailField, PhaseOutcome, PolicyOutcome, PostingOccurrence,
    ProfileBrowserClient, ProfileDslSourceDetailExecution, ProfileHttpClient,
    RequestedDetailFields, RequestedFieldDisposition, ReqwestProfileHttpClient,
    RuntimeExecutionContext, SourceDetailExecution, SourceDetailOutcome, SourceDetailRequest,
    SourceDetailResult, UnavailableProfileBrowserClient,
};
use crate::source::documents::SelectedAccessPath;
use crate::source_profile::registry::{RegistrySource, SourceProfileRegistrySnapshot};

use super::persistence::validate_source_live_check_report_key;
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

pub const SOURCE_LIVE_CHECK_LOGIC_VERSION: &str = "source-live-check/v2";
pub(crate) const SOURCE_LIVE_CHECK_MAX_DISCOVERY_REQUESTS: u64 = 1;

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
        &ReqwestProfileHttpClient::new(),
        &ReqwestProfileHttpClient::new(),
        &UnavailableProfileBrowserClient,
    )
}

pub fn check_source_with_fetcher<F>(
    app_data_dir: impl AsRef<Path>,
    source_key: impl AsRef<str>,
    fetcher: &F,
) -> Result<CheckReport, String>
where
    F: ProfileHttpClient + Sync + ?Sized,
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
    D: ProfileHttpClient + Sync + ?Sized,
    T: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let app_data_dir = app_data_dir.as_ref();
    let source_key = source_key.as_ref();
    validate_source_live_check_report_key(source_key).map_err(|error| error.to_string())?;
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
    validate_source_live_check_report_key(source_key).map_err(|error| error.to_string())?;
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
    let current_fingerprints = prepare_source_live_check(&snapshot, source_key)?.fingerprints;
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
    D: ProfileHttpClient + Sync + ?Sized,
    T: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    let prepared = prepare_source_live_check(snapshot, source_key)?;
    let document = &prepared.source.document;
    let mut diagnostics = prepared.source.validation_state.diagnostics.clone();
    let fingerprints = prepared.fingerprints.clone();
    let mut details = source_live_check_details_placeholders();
    details.insert(
        "sourceStatusAtCheck".to_string(),
        serde_json::to_value(document.status).map_err(|error| {
            format!("Source Status could not be serialized for Source Live Check: {error}")
        })?,
    );
    let live_check_subject = SourceLiveCheckSubject::from_selected_access_path(
        source_key,
        &document.selected_access_path,
    );
    details.insert(
        "accessPathKey".to_string(),
        serde_json::Value::String(live_check_subject.access_path_key.clone()),
    );

    if let Some(compiled) = prepared.compiled() {
        let execution_plan = &compiled.execution_plan;
        let discovery_context = RuntimeExecutionContext::uncancellable().with_limits(PhaseLimits {
            max_requests: SOURCE_LIVE_CHECK_MAX_DISCOVERY_REQUESTS,
            ..execution_plan.discovery.limits
        });
        let discovery_result = tauri::async_runtime::block_on(execute_discovery(
            execution_plan,
            &document.source_config,
            discovery_fetcher,
            browser,
            discovery_context,
        ));
        let (discovery_candidates, discovery_report, discovery_diagnostics) = match discovery_result
        {
            Ok(PhaseOutcome::Completed {
                policy_outcome: PolicyOutcome::Accepted { reduced_payload },
                complete_budget_report,
                diagnostics,
            }) => (
                reduced_payload.candidates,
                Some(complete_budget_report),
                diagnostics,
            ),
            Ok(outcome) => (
                Vec::new(),
                Some(outcome.complete_budget_report().clone()),
                outcome.diagnostics().clone(),
            ),
            Err(crate::profile_dsl::runtime::PhaseRunError::Cancelled(cancelled)) => (
                Vec::new(),
                Some(cancelled.complete_budget_report),
                cancelled.diagnostics,
            ),
            Err(crate::profile_dsl::runtime::PhaseRunError::NotStarted { diagnostics, .. }) => {
                (Vec::new(), None, diagnostics)
            }
        };
        let candidate_count = discovery_candidates.len();
        let first_acceptable_candidate = discovery_candidates
            .iter()
            .find(|candidate| is_acceptable_live_candidate(candidate));
        let acceptable_candidate_count = discovery_candidates
            .iter()
            .filter(|candidate| is_acceptable_live_candidate(candidate))
            .count();
        details.insert(
            "candidateCount".to_string(),
            serde_json::json!(candidate_count),
        );
        if let Some(report) = &discovery_report {
            details.insert(
                "discoveryExecutionReport".to_string(),
                serde_json::to_value(report).map_err(|error| {
                    format!("Discovery report could not be serialized: {error}")
                })?,
            );
        }
        diagnostics.extend(discovery_diagnostics);

        if acceptable_candidate_count == 0 {
            diagnostics.push(no_candidates_diagnostic(
                Some(&live_check_subject),
                candidate_count,
                acceptable_candidate_count,
            ));
        } else if execution_plan.detail.is_some() {
            if let Some(candidate) = first_acceptable_candidate {
                details.insert("detailChecked".to_string(), serde_json::Value::Bool(true));
                let detail_execution =
                    ProfileDslSourceDetailExecution::new(detail_fetcher, browser);
                let detail_result =
                    tauri::async_runtime::block_on(detail_execution.execute(SourceDetailRequest {
                        compiled_source: compiled,
                        occurrence: candidate,
                        requested_fields: RequestedDetailFields::description_text(),
                        context: RuntimeExecutionContext::uncancellable(),
                    }));
                let detail_report = match &detail_result {
                    Ok(outcome) => outcome.complete_budget_report(),
                    Err(cancelled) => Some(&cancelled.complete_budget_report),
                };
                if let Some(report) = detail_report {
                    details.insert(
                        "detailExecutionReport".to_string(),
                        serde_json::to_value(report).map_err(|error| {
                            format!("Detail report could not be serialized: {error}")
                        })?,
                    );
                }
                let detail_passed = is_acceptable_detail_result(&detail_result);
                details.insert(
                    "detailPassed".to_string(),
                    serde_json::Value::Bool(detail_passed),
                );
                let detail_failure_cause = if detail_passed {
                    let outcome = detail_result
                        .as_ref()
                        .expect("passing Detail result is a normal outcome");
                    diagnostics.extend(non_error_diagnostics(
                        outcome.diagnostics().cloned().unwrap_or_default(),
                    ));
                    None
                } else {
                    diagnostics.extend(source_detail_diagnostics(&detail_result));
                    Some(detail_failure_cause(&detail_result))
                };
                if let Some(cause) = detail_failure_cause {
                    diagnostics.push(detail_failed_diagnostic(
                        Some(&live_check_subject),
                        &candidate.reference.provider_url,
                        &cause,
                    ));
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

struct PreparedSourceLiveCheck<'a> {
    source: &'a RegistrySource,
    outcome: &'a CompileSourceOutcome,
    fingerprints: Vec<CheckFingerprint>,
}

impl PreparedSourceLiveCheck<'_> {
    fn compiled(&self) -> Option<&CompiledSource> {
        match self.outcome {
            CompileSourceOutcome::Compiled { source, .. } => Some(source),
            CompileSourceOutcome::Rejected { .. } => None,
        }
    }
}

fn prepare_source_live_check<'a>(
    snapshot: &'a SourceProfileRegistrySnapshot,
    source_key: &str,
) -> Result<PreparedSourceLiveCheck<'a>, String> {
    let source = snapshot
        .source(source_key)
        .ok_or_else(|| format!("Source `{source_key}` was not found in the registry snapshot"))?;
    let outcome = source.compile_outcome.as_ref().ok_or_else(|| {
        format!(
            "Source `{source_key}` has no authoritative compiler outcome in the registry snapshot"
        )
    })?;
    let base_profile = match &source.document.selected_access_path {
        SelectedAccessPath::ProfileAccessPath { profile_key, .. } => Some(
            &snapshot
                .profile(profile_key)
                .ok_or_else(|| {
                    format!(
                        "Source `{source_key}` references unresolved Source Profile `{profile_key}`"
                    )
                })?
                .document,
        ),
        SelectedAccessPath::SourceOwnedAccessPath { .. } => None,
    };
    let fingerprints =
        prepare_source_behavior_fingerprints(&source.document, base_profile, outcome)
            .map_err(|error| error.to_string())?;

    Ok(PreparedSourceLiveCheck {
        source,
        outcome,
        fingerprints,
    })
}

fn source_live_check_details_placeholders() -> JsonObject {
    let mut details = JsonObject::new();
    details.insert("sourceStatusAtCheck".to_string(), serde_json::Value::Null);
    details.insert("liveCheckState".to_string(), serde_json::Value::Null);
    details.insert("accessPathKey".to_string(), serde_json::Value::Null);
    details.insert("candidateCount".to_string(), serde_json::Value::Null);
    details.insert(
        "discoveryMode".to_string(),
        serde_json::Value::String("bounded_smoke".to_string()),
    );
    details.insert(
        "maxDiscoveryRequests".to_string(),
        serde_json::json!(SOURCE_LIVE_CHECK_MAX_DISCOVERY_REQUESTS),
    );
    details.insert("detailChecked".to_string(), serde_json::Value::Bool(false));
    details.insert("detailPassed".to_string(), serde_json::Value::Null);
    details
}

fn is_acceptable_live_candidate(candidate: &PostingOccurrence) -> bool {
    candidate
        .provider_values
        .title
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && candidate
            .provider_values
            .company
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

fn is_acceptable_detail_result(result: &SourceDetailResult) -> bool {
    matches!(
        result,
        Ok(SourceDetailOutcome::Completed {
            fields,
            dispositions,
            ..
        }) if fields
            .description_text
            .as_ref()
            .is_some_and(|description_text| !description_text.trim().is_empty())
            && dispositions.iter().any(|disposition| matches!(
                disposition,
                RequestedFieldDisposition::Reused {
                    field: DetailField::DescriptionText
                } | RequestedFieldDisposition::Produced {
                    field: DetailField::DescriptionText
                }
            ))
    )
}

fn source_detail_diagnostics(result: &SourceDetailResult) -> Diagnostics {
    match result {
        Ok(outcome) => outcome.diagnostics().cloned().unwrap_or_default(),
        Err(cancelled) => cancelled.diagnostics.clone(),
    }
}

fn non_error_diagnostics(diagnostics: Diagnostics) -> Diagnostics {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
        .collect()
}

fn detail_failure_cause(result: &SourceDetailResult) -> String {
    match result {
        Ok(SourceDetailOutcome::Completed { dispositions, .. }) => dispositions
            .iter()
            .find_map(|disposition| match disposition {
                RequestedFieldDisposition::Unsupported {
                    field: DetailField::DescriptionText,
                } => Some("detail_description_text_unsupported"),
                RequestedFieldDisposition::Unavailable {
                    field: DetailField::DescriptionText,
                } => Some("detail_description_text_unavailable"),
                RequestedFieldDisposition::Conflicted {
                    field: DetailField::DescriptionText,
                } => Some("detail_description_text_conflicted"),
                _ => None,
            })
            .unwrap_or("detail_description_text_missing"),
        Ok(SourceDetailOutcome::BudgetExhausted { .. }) => "detail_budget_exhausted",
        Ok(SourceDetailOutcome::CandidateExecutionFailed { .. }) => {
            "detail_candidate_execution_failed"
        }
        Ok(SourceDetailOutcome::SourceExecutionFailed { .. }) => "detail_source_execution_failed",
        Ok(SourceDetailOutcome::SourceMismatch) => "detail_source_mismatch",
        Err(_) => "detail_cancelled",
    }
    .to_string()
}

#[cfg(test)]
mod source_detail_typed_control_tests {
    use super::*;
    use crate::profile_dsl::runtime::{
        DetailPatch, PhaseCompletion, PhaseExecutionReport, PhaseUsage, SourceDetailPhaseEvidence,
    };

    #[test]
    fn changed_runtime_diagnostic_text_does_not_change_live_check_detail_state() {
        let result_with_message = |message: &str| {
            Ok(SourceDetailOutcome::Completed {
                fields: DetailPatch::default(),
                dispositions: vec![RequestedFieldDisposition::Unavailable {
                    field: DetailField::DescriptionText,
                }],
                phase_evidence: Some(SourceDetailPhaseEvidence {
                    complete_budget_report: PhaseExecutionReport {
                        usage: PhaseUsage::default(),
                        completion: PhaseCompletion::PolicyUnsatisfied,
                    },
                    diagnostics: vec![Diagnostic {
                        category: DiagnosticCategory::Runtime,
                        code: "arbitrary_runtime_code".to_string(),
                        message: message.to_string(),
                        severity: DiagnosticSeverity::Error,
                        path: "/detail".to_string(),
                        strategy_key: None,
                        details: None,
                    }],
                }),
            })
        };
        let first: SourceDetailResult = result_with_message("first wording");
        let second: SourceDetailResult = result_with_message("completely changed wording");

        assert!(!is_acceptable_detail_result(&first));
        assert!(!is_acceptable_detail_result(&second));
        assert_eq!(detail_failure_cause(&first), detail_failure_cause(&second));
        assert_eq!(
            detail_failure_cause(&first),
            "detail_description_text_unavailable"
        );
    }
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
        path: "/discovery".to_string(),
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
        message: "Source Live Check Detail failed for the selected candidate".to_string(),
        severity: DiagnosticSeverity::Error,
        path: "/detail".to_string(),
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
