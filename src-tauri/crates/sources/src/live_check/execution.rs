use super::fingerprints::prepare_source_behavior_fingerprints;
use crate::installed::{PreparedSource, Snapshot};
use source_engine::definition::CompiledSource;
use source_engine::definition::SelectedAccessPath;
use source_engine::definition::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics};
use source_engine::definition::{JsonObject, PhaseLimits};
use source_engine::execution::{
    discover, BrowserAcquisition, DetailField, PhaseOutcome, PolicyOutcome, PostingOccurrence,
    ProfileHttpClient, RequestedDetailFields, RequestedFieldDisposition, RuntimeExecutionContext,
    SourceBehaviorDetailExecution, SourceDetailExecution, SourceDetailOutcome, SourceDetailRequest,
    SourceDetailResult,
};

use super::{
    fingerprint::CheckFingerprint,
    report::{CheckReport, CheckReportKind, CheckReportResult, CheckReportSubject},
};

pub(super) const MAX_DISCOVERY_REQUESTS: u64 = 1;

#[derive(Clone)]
pub(super) struct ExecutionContext<'a> {
    pub checked_at: String,
    pub cancellation: Option<&'a dyn source_engine::execution::RuntimeCancellation>,
}

impl<'a> ExecutionContext<'a> {
    fn runtime_context(&self) -> RuntimeExecutionContext<'a> {
        self.cancellation.map_or_else(
            RuntimeExecutionContext::uncancellable,
            RuntimeExecutionContext::with_cancellation,
        )
    }
}

pub(super) async fn build_report<D, T, A>(
    snapshot: &Snapshot,
    source_key: &str,
    discovery_fetcher: &D,
    detail_fetcher: &T,
    acquisition: &A,
    context: &ExecutionContext<'_>,
) -> Result<CheckReport, String>
where
    D: ProfileHttpClient + Sync,
    T: ProfileHttpClient + Sync + ?Sized,
    A: BrowserAcquisition + Sync,
{
    let prepared = prepare(snapshot, source_key)?;
    let document = prepared.source.document();
    let mut diagnostics = prepared.source.validation().diagnostics.clone();
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
        let discovery_context = context.runtime_context().with_limits(PhaseLimits {
            max_requests: MAX_DISCOVERY_REQUESTS,
            ..compiled.discovery_limits()
        });
        let discovery_result =
            discover(compiled, discovery_fetcher, acquisition, discovery_context).await;
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
            Err(source_engine::execution::PhaseRunError::Cancelled(cancelled)) => (
                Vec::new(),
                Some(cancelled.complete_budget_report),
                cancelled.diagnostics,
            ),
            Err(source_engine::execution::PhaseRunError::NotStarted { diagnostics, .. }) => {
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
        } else if compiled.supports_detail() {
            if let Some(candidate) = first_acceptable_candidate {
                details.insert("detailChecked".to_string(), serde_json::Value::Bool(true));
                let detail_execution =
                    SourceBehaviorDetailExecution::new(detail_fetcher, acquisition);
                let detail_result = detail_execution
                    .execute(SourceDetailRequest {
                        compiled_source: compiled,
                        occurrence: candidate,
                        requested_fields: RequestedDetailFields::description_text(),
                        context: context.runtime_context(),
                    })
                    .await;
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
        context.checked_at.clone(),
        super::LOGIC_VERSION,
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

pub(super) struct PreparedSourceLiveCheck<'a> {
    source: &'a PreparedSource,
    pub fingerprints: Vec<CheckFingerprint>,
}

impl PreparedSourceLiveCheck<'_> {
    fn compiled(&self) -> Option<&CompiledSource> {
        self.source.compiled()
    }
}

pub(super) fn prepare<'a>(
    snapshot: &'a Snapshot,
    source_key: &str,
) -> Result<PreparedSourceLiveCheck<'a>, String> {
    let source = snapshot
        .source(source_key)
        .ok_or_else(|| format!("Source `{source_key}` was not found in the registry snapshot"))?;
    let outcome = source.compiler_outcome();
    let base_profile = match &source.document().selected_access_path {
        SelectedAccessPath::ProfileAccessPath { profile_key, .. } => Some(
            snapshot
                .profile_for_live_check(profile_key)
                .ok_or_else(|| {
                    format!(
                    "Source `{source_key}` references unresolved Source Profile `{profile_key}`"
                )
                })?,
        ),
        SelectedAccessPath::SourceOwnedAccessPath { .. } => None,
    };
    let fingerprints =
        prepare_source_behavior_fingerprints(source.document(), base_profile, outcome)
            .map_err(|error| error.to_string())?;

    Ok(PreparedSourceLiveCheck {
        source,
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
        serde_json::json!(MAX_DISCOVERY_REQUESTS),
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
    use source_engine::execution::{
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
