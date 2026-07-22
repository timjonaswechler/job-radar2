use std::{collections::VecDeque, future::Future, pin::Pin, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::profile_dsl::{
    compiler::CompiledSource,
    diagnostics::Diagnostics,
    occurrence::{
        DetailField, DetailPatch, PostingOccurrence, PostingOccurrenceIdentity,
        RequestedDetailFields,
    },
};

use super::{
    allowance::PhaseExecutionReport,
    browser::ProfileBrowserClient,
    cancellation::RuntimeExecutionContext,
    detail::execute_detail,
    http::ProfileHttpClient,
    outcome::{
        DetailPhasePayload, PhaseCancelled, PhaseExecutionFailure, PhaseOutcome,
        PhasePreStartFailure, PhaseRunError, PolicyOutcome, PolicyUnsatisfiedCause,
    },
};

/// One checked, candidate-scoped Source Detail request.
///
/// `compiled_source` owns the authoritative executable plan and its validated,
/// immutable runtime binding input.
pub struct SourceDetailRequest<'a> {
    pub compiled_source: &'a CompiledSource,
    pub occurrence: &'a PostingOccurrence,
    pub requested_fields: RequestedDetailFields,
    pub context: RuntimeExecutionContext<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDetailRequestSnapshot {
    pub source_key: String,
    pub occurrence_identity: PostingOccurrenceIdentity,
    pub requested_fields: RequestedDetailFields,
}

impl SourceDetailRequestSnapshot {
    pub fn new(
        source_key: impl Into<String>,
        occurrence_identity: PostingOccurrenceIdentity,
        requested_fields: RequestedDetailFields,
    ) -> Self {
        Self {
            source_key: source_key.into(),
            occurrence_identity,
            requested_fields,
        }
    }

    fn from_request(request: &SourceDetailRequest<'_>) -> Self {
        Self {
            source_key: request.compiled_source.execution_plan.source.key.clone(),
            occurrence_identity: request.occurrence.identity.clone(),
            requested_fields: request.requested_fields.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RequestedFieldDisposition {
    Reused { field: DetailField },
    Produced { field: DetailField },
    Unsupported { field: DetailField },
    Unavailable { field: DetailField },
    Conflicted { field: DetailField },
}

impl RequestedFieldDisposition {
    pub fn field(self) -> DetailField {
        match self {
            Self::Reused { field }
            | Self::Produced { field }
            | Self::Unsupported { field }
            | Self::Unavailable { field }
            | Self::Conflicted { field } => field,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetailPhaseEvidence {
    pub complete_budget_report: PhaseExecutionReport,
    pub diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDetailFailure {
    IncludesExecutionFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceDetailFailure {
    PhaseExecution { failure: PhaseExecutionFailure },
    PhasePreStart { failure: PhasePreStartFailure },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceDetailOutcome {
    Completed {
        fields: DetailPatch,
        dispositions: Vec<RequestedFieldDisposition>,
        #[serde(skip_serializing_if = "Option::is_none")]
        phase_evidence: Option<SourceDetailPhaseEvidence>,
    },
    BudgetExhausted {
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    CandidateExecutionFailed {
        typed_failure: CandidateDetailFailure,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    SourceExecutionFailed {
        typed_failure: SourceDetailFailure,
        /// Exact phase report after phase work starts; absent only when the
        /// lower phase rejects the request before starting and supplies no report.
        #[serde(skip_serializing_if = "Option::is_none")]
        complete_budget_report: Option<PhaseExecutionReport>,
        diagnostics: Diagnostics,
    },
    SourceMismatch,
}

impl SourceDetailOutcome {
    pub fn diagnostics(&self) -> Option<&Diagnostics> {
        match self {
            Self::Completed {
                phase_evidence: Some(evidence),
                ..
            } => Some(&evidence.diagnostics),
            Self::BudgetExhausted { diagnostics, .. }
            | Self::CandidateExecutionFailed { diagnostics, .. }
            | Self::SourceExecutionFailed { diagnostics, .. } => Some(diagnostics),
            Self::Completed {
                phase_evidence: None,
                ..
            }
            | Self::SourceMismatch => None,
        }
    }

    pub fn complete_budget_report(&self) -> Option<&PhaseExecutionReport> {
        match self {
            Self::Completed {
                phase_evidence: Some(evidence),
                ..
            } => Some(&evidence.complete_budget_report),
            Self::BudgetExhausted {
                complete_budget_report,
                ..
            }
            | Self::CandidateExecutionFailed {
                complete_budget_report,
                ..
            } => Some(complete_budget_report),
            Self::SourceExecutionFailed {
                complete_budget_report,
                ..
            } => complete_budget_report.as_ref(),
            Self::Completed {
                phase_evidence: None,
                ..
            }
            | Self::SourceMismatch => None,
        }
    }
}

/// Exact shared phase cancellation object; Source Detail does not wrap or rebuild it.
pub type DetailCancelled = PhaseCancelled;

pub type SourceDetailResult = Result<SourceDetailOutcome, DetailCancelled>;

/// Domain-owned candidate-scoped Source Detail execution seam.
pub trait SourceDetailExecution: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: SourceDetailRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = SourceDetailResult> + Send + 'a>>;
}

pub struct ProfileDslSourceDetailExecution<'a, F: ?Sized, B: ?Sized> {
    fetcher: &'a F,
    browser: &'a B,
}

impl<'a, F: ?Sized, B: ?Sized> ProfileDslSourceDetailExecution<'a, F, B> {
    pub fn new(fetcher: &'a F, browser: &'a B) -> Self {
        Self { fetcher, browser }
    }
}

impl<F, B> SourceDetailExecution for ProfileDslSourceDetailExecution<'_, F, B>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    fn execute<'a>(
        &'a self,
        request: SourceDetailRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = SourceDetailResult> + Send + 'a>> {
        Box::pin(async move { self.execute_request(request).await })
    }
}

impl<F, B> ProfileDslSourceDetailExecution<'_, F, B>
where
    F: ProfileHttpClient + Sync + ?Sized,
    B: ProfileBrowserClient + Sync + ?Sized,
{
    async fn execute_request(&self, request: SourceDetailRequest<'_>) -> SourceDetailResult {
        let source_key = &request.compiled_source.execution_plan.source.key;
        if request.occurrence.identity.source_key() != source_key {
            return Ok(SourceDetailOutcome::SourceMismatch);
        }

        let capabilities = request.compiled_source.detail_capabilities();
        let mut fields = DetailPatch::default();
        let mut dispositions = Vec::new();
        let mut phase_fields = Vec::new();

        for field in request.requested_fields.iter() {
            if reuse_occurrence_field(&mut fields, request.occurrence, field) {
                dispositions.push(RequestedFieldDisposition::Reused { field });
            } else if !capabilities.contains(field) {
                dispositions.push(RequestedFieldDisposition::Unsupported { field });
            } else {
                phase_fields.push(field);
            }
        }

        if phase_fields.is_empty() {
            return Ok(SourceDetailOutcome::Completed {
                fields,
                dispositions,
                phase_evidence: None,
            });
        }

        let phase_request = RequestedDetailFields::new(phase_fields.iter().copied())
            .expect("supported missing Detail fields are non-empty");
        let phase_result = execute_detail(
            &request.compiled_source.execution_plan,
            &request.compiled_source.source_config,
            request.occurrence,
            phase_request,
            self.fetcher,
            self.browser,
            request.context,
        )
        .await;

        match phase_result {
            Ok(PhaseOutcome::Completed {
                policy_outcome,
                complete_budget_report,
                diagnostics,
            }) => match policy_outcome {
                PolicyOutcome::Accepted { reduced_payload } => {
                    project_completed_fields(
                        &mut fields,
                        &mut dispositions,
                        &phase_fields,
                        &reduced_payload,
                    );
                    canonicalize_dispositions(&mut dispositions);
                    Ok(SourceDetailOutcome::Completed {
                        fields,
                        dispositions,
                        phase_evidence: Some(SourceDetailPhaseEvidence {
                            complete_budget_report,
                            diagnostics,
                        }),
                    })
                }
                PolicyOutcome::PolicyUnsatisfied {
                    cause: PolicyUnsatisfiedCause::RejectedOnly,
                } => {
                    dispositions.extend(
                        phase_fields
                            .iter()
                            .copied()
                            .map(|field| RequestedFieldDisposition::Unavailable { field }),
                    );
                    canonicalize_dispositions(&mut dispositions);
                    Ok(SourceDetailOutcome::Completed {
                        fields,
                        dispositions,
                        phase_evidence: Some(SourceDetailPhaseEvidence {
                            complete_budget_report,
                            diagnostics,
                        }),
                    })
                }
                PolicyOutcome::PolicyUnsatisfied {
                    cause: PolicyUnsatisfiedCause::IncludesExecutionFailure,
                } => Ok(SourceDetailOutcome::CandidateExecutionFailed {
                    typed_failure: CandidateDetailFailure::IncludesExecutionFailure,
                    complete_budget_report,
                    diagnostics,
                }),
            },
            Ok(PhaseOutcome::BudgetExhausted {
                complete_budget_report,
                diagnostics,
            }) => Ok(SourceDetailOutcome::BudgetExhausted {
                complete_budget_report,
                diagnostics,
            }),
            Ok(PhaseOutcome::ExecutionFailed {
                typed_failure,
                complete_budget_report,
                diagnostics,
            }) => Ok(SourceDetailOutcome::SourceExecutionFailed {
                typed_failure: SourceDetailFailure::PhaseExecution {
                    failure: typed_failure,
                },
                complete_budget_report: Some(complete_budget_report),
                diagnostics,
            }),
            Err(PhaseRunError::Cancelled(cancelled)) => Err(cancelled),
            Err(PhaseRunError::NotStarted {
                failure,
                diagnostics,
            }) => Ok(SourceDetailOutcome::SourceExecutionFailed {
                typed_failure: SourceDetailFailure::PhasePreStart { failure },
                complete_budget_report: None,
                diagnostics,
            }),
        }
    }
}

fn reuse_occurrence_field(
    fields: &mut DetailPatch,
    occurrence: &PostingOccurrence,
    field: DetailField,
) -> bool {
    match field {
        DetailField::Title => occurrence
            .provider_values
            .title
            .as_ref()
            .filter(|value| !value.is_empty())
            .is_some_and(|value| {
                fields.title = Some(value.clone());
                true
            }),
        DetailField::Company => occurrence
            .provider_values
            .company
            .as_ref()
            .filter(|value| !value.is_empty())
            .is_some_and(|value| {
                fields.company = Some(value.clone());
                true
            }),
        DetailField::Locations => {
            if occurrence.provider_values.locations.is_empty()
                || occurrence
                    .provider_values
                    .locations
                    .iter()
                    .any(String::is_empty)
            {
                false
            } else {
                fields.locations = Some(occurrence.provider_values.locations.clone());
                true
            }
        }
        DetailField::DescriptionText => occurrence
            .provider_values
            .description_text
            .as_ref()
            .filter(|value| !value.is_empty())
            .is_some_and(|value| {
                fields.description_text = Some(value.clone());
                true
            }),
    }
}

fn project_completed_fields(
    fields: &mut DetailPatch,
    dispositions: &mut Vec<RequestedFieldDisposition>,
    phase_fields: &[DetailField],
    payload: &DetailPhasePayload,
) {
    for field in phase_fields.iter().copied() {
        if payload
            .conflicts
            .iter()
            .any(|conflict| conflict.field == field)
        {
            dispositions.push(RequestedFieldDisposition::Conflicted { field });
        } else if copy_patch_field(fields, &payload.patch, field) {
            dispositions.push(RequestedFieldDisposition::Produced { field });
        } else {
            dispositions.push(RequestedFieldDisposition::Unavailable { field });
        }
    }
}

fn copy_patch_field(target: &mut DetailPatch, source: &DetailPatch, field: DetailField) -> bool {
    match field {
        DetailField::Title => source.title.as_ref().is_some_and(|value| {
            target.title = Some(value.clone());
            true
        }),
        DetailField::Company => source.company.as_ref().is_some_and(|value| {
            target.company = Some(value.clone());
            true
        }),
        DetailField::Locations => source.locations.as_ref().is_some_and(|value| {
            target.locations = Some(value.clone());
            true
        }),
        DetailField::DescriptionText => source.description_text.as_ref().is_some_and(|value| {
            target.description_text = Some(value.clone());
            true
        }),
    }
}

fn canonicalize_dispositions(dispositions: &mut [RequestedFieldDisposition]) {
    dispositions.sort_by_key(|disposition| disposition.field());
}

/// Strict scripted seam implementation. It records immutable call snapshots and
/// returns only pre-scripted closed results; it never interprets a plan or writes.
pub struct ScriptedSourceDetailExecution {
    script: Mutex<VecDeque<(SourceDetailRequestSnapshot, SourceDetailResult)>>,
    recorded: Mutex<Vec<SourceDetailRequestSnapshot>>,
}

impl ScriptedSourceDetailExecution {
    pub fn new(
        script: impl IntoIterator<Item = (SourceDetailRequestSnapshot, SourceDetailResult)>,
    ) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            recorded: Mutex::new(Vec::new()),
        }
    }

    pub fn recorded_calls(&self) -> Vec<SourceDetailRequestSnapshot> {
        self.recorded
            .lock()
            .expect("Source Detail recording mutex poisoned")
            .clone()
    }

    pub fn assert_finished(&self) {
        let remaining = self
            .script
            .lock()
            .expect("Source Detail script mutex poisoned")
            .len();
        assert_eq!(
            remaining, 0,
            "{remaining} scripted Source Detail calls remain"
        );
    }
}

impl SourceDetailExecution for ScriptedSourceDetailExecution {
    fn execute<'a>(
        &'a self,
        request: SourceDetailRequest<'a>,
    ) -> Pin<Box<dyn Future<Output = SourceDetailResult> + Send + 'a>> {
        let actual = SourceDetailRequestSnapshot::from_request(&request);
        let scripted = self
            .script
            .lock()
            .expect("Source Detail script mutex poisoned")
            .pop_front()
            .expect("unexpected Source Detail call: script is exhausted");
        assert_eq!(actual, scripted.0, "unexpected Source Detail call");
        self.recorded
            .lock()
            .expect("Source Detail recording mutex poisoned")
            .push(actual);
        Box::pin(std::future::ready(scripted.1))
    }
}
