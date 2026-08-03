//! Source-scoped Candidate Resolution and finalized-only Search Run boundary.

use std::collections::BTreeMap;

use geo::{
    LocationFilterError, LocationFilterMatchReport, LocationFilterNotAppliedReason,
    LocationResolutionAmbiguity,
};
use serde::Serialize;
use url::Url;

use source_engine::{
    definition::{
        CompiledSource, Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics,
        PhaseLimits,
    },
    execution::{
        discover, BoxedBrowserAcquisitionFuture, BrowserAcquisition, BrowserAcquisitionRequest,
        DetailField, DetailPatch, PhaseCompletion, PhaseExecutionReport, PhaseOutcome,
        PhaseRunError, PhaseUsage, PolicyOutcome, PostingOccurrence, PostingOccurrenceIdentity,
        ProfileHttpClient, RequestedDetailFields, RuntimeCancellation, RuntimeExecutionContext,
        SourceDetailExecution, SourceDetailOutcome, SourceDetailRequest,
    },
};

use crate::{
    normalization::{collapse_whitespace, normalize_locations},
    requirements::Requirements,
};

const DIAGNOSTIC_SAMPLE_LIMIT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionLimitDimension {
    DetailCandidates,
    StrategyAttempts,
    Requests,
    ProducedItems,
    Duration,
    Pages,
    BrowserActions,
    FanOut,
    ResponseBytes,
    BrowserRenderedBytes,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    source_key: String,
    identity: PostingOccurrenceIdentity,
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
}

impl Candidate {
    pub fn source_key(&self) -> &str {
        &self.source_key
    }
    pub fn identity(&self) -> &PostingOccurrenceIdentity {
        &self.identity
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn company(&self) -> &str {
        &self.company
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn locations(&self) -> &[String] {
        &self.locations
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ResolutionCompletion {
    Complete,
    Partial {
        limit_reached: ResolutionLimitDimension,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionCounts {
    pub discovered: u64,
    pub processed: u64,
    pub finalized: u64,
    pub rejected: u64,
    pub unresolved: u64,
    pub failed: u64,
    pub budget_skipped: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDiagnosticSummary {
    pub counts_by_code: BTreeMap<String, u64>,
    pub samples: Diagnostics,
    pub sample_limit: u64,
    pub candidate_diagnostics_omitted: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub source_key: String,
    pub finalized: Vec<Candidate>,
    pub completion: ResolutionCompletion,
    pub counts: ResolutionCounts,
    pub usage: PhaseUsage,
    pub diagnostics: Diagnostics,
    pub candidate_diagnostics: CandidateDiagnosticSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFailure {
    DiscoveryExecution,
    SourceDetailExecution,
    GeoResolution,
    SourceMismatch,
    ProtocolInvariant,
    ArithmeticInvariant,
    ReportAboveAllowance,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolutionError {
    Failed {
        failure: ResolutionFailure,
        diagnostics: Diagnostics,
    },
    Cancelled,
}

pub struct Resolver<'a> {
    http: &'a (dyn ProfileHttpClient + Sync),
    browser: &'a dyn BrowserAcquisition,
}

impl<'a> Resolver<'a> {
    pub fn new(
        http: &'a (dyn ProfileHttpClient + Sync),
        browser: &'a dyn BrowserAcquisition,
    ) -> Self {
        Self { http, browser }
    }

    pub async fn resolve<'requirements>(
        &self,
        compiled_source: &CompiledSource,
        requirements: &'requirements Requirements<'requirements>,
        cancellation: &dyn RuntimeCancellation,
    ) -> Result<Resolution, ResolutionError> {
        cancelled(cancellation)?;
        if let Some(error) = requirements.geo_failure.clone() {
            return Err(geo_resolution_failed(error));
        }

        let source_key = compiled_source.source_key().to_string();
        let mut state =
            ResolutionState::new(source_key.clone(), PhaseLimits::BACKEND, requirements);
        let parent_limits = match state.parent.remaining_limits()? {
            ParentAdmission::Admitted(limits) => limits,
            ParentAdmission::Exhausted(dimension) => {
                state.partial = Some(dimension);
                return state.finish(cancellation);
            }
        };
        let discovery_limits = intersect_limits(parent_limits, compiled_source.discovery_limits());
        let discovery = discover(
            compiled_source,
            self.http,
            self.browser,
            RuntimeExecutionContext::with_cancellation(cancellation).with_limits(discovery_limits),
        )
        .await;

        let occurrences = match discovery {
            Err(PhaseRunError::Cancelled(cancelled)) => {
                validate_child_report(
                    &cancelled.complete_budget_report,
                    discovery_limits,
                    |completion| matches!(completion, PhaseCompletion::Cancelled { .. }),
                )?;
                state.parent.commit(&cancelled.complete_budget_report)?;
                return Err(ResolutionError::Cancelled);
            }
            Err(PhaseRunError::NotStarted { diagnostics, .. }) => {
                return Err(failed(ResolutionFailure::DiscoveryExecution, diagnostics));
            }
            Ok(PhaseOutcome::Completed {
                policy_outcome: PolicyOutcome::Accepted { reduced_payload },
                complete_budget_report,
                diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, discovery_limits, |completion| {
                    matches!(completion, PhaseCompletion::Accepted)
                })?;
                state.parent.commit(&complete_budget_report)?;
                state.diagnostics.extend(diagnostics);
                reduced_payload.candidates
            }
            Ok(PhaseOutcome::BudgetExhausted {
                complete_budget_report,
                diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, discovery_limits, |completion| {
                    matches!(completion, PhaseCompletion::BudgetExhausted { .. })
                })?;
                state.parent.commit(&complete_budget_report)?;
                state.diagnostics.extend(diagnostics);
                state.partial = dimension_from_completion(&complete_budget_report.completion)
                    .or(Some(ResolutionLimitDimension::Requests));
                return state.finish(cancellation);
            }
            Ok(PhaseOutcome::ExecutionFailed {
                complete_budget_report,
                diagnostics,
                ..
            })
            | Ok(PhaseOutcome::Completed {
                policy_outcome: PolicyOutcome::PolicyUnsatisfied { .. },
                complete_budget_report,
                diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, discovery_limits, |completion| {
                    matches!(
                        completion,
                        PhaseCompletion::ExecutionFailed | PhaseCompletion::PolicyUnsatisfied
                    )
                })?;
                state.parent.commit(&complete_budget_report)?;
                return Err(failed(ResolutionFailure::DiscoveryExecution, diagnostics));
            }
        };

        if occurrences
            .iter()
            .any(|occurrence| occurrence.identity.source_key() != source_key)
        {
            return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
        }

        let browser = BrowserAdapter(self.browser);
        let detail =
            source_engine::execution::SourceBehaviorDetailExecution::new(self.http, &browser);
        let context = ResolveContext {
            compiled_source,
            requirements,
            cancellation,
            detail: &detail,
        };
        state.process_occurrences(&context, &occurrences).await?;
        state.finish(cancellation)
    }
}

struct BrowserAdapter<'a>(&'a dyn BrowserAcquisition);

impl BrowserAcquisition for BrowserAdapter<'_> {
    fn acquire<'a>(
        &'a self,
        request: BrowserAcquisitionRequest<'a>,
    ) -> BoxedBrowserAcquisitionFuture<'a> {
        self.0.acquire(request)
    }
}

struct ResolveContext<'a> {
    compiled_source: &'a CompiledSource,
    requirements: &'a Requirements<'a>,
    cancellation: &'a dyn RuntimeCancellation,
    detail: &'a dyn SourceDetailExecution,
}

fn intersect_limits(left: PhaseLimits, right: PhaseLimits) -> PhaseLimits {
    PhaseLimits {
        max_strategy_attempts: left.max_strategy_attempts.min(right.max_strategy_attempts),
        max_requests: left.max_requests.min(right.max_requests),
        max_produced_items: left.max_produced_items.min(right.max_produced_items),
        max_duration_ms: left.max_duration_ms.min(right.max_duration_ms),
        max_pages: left.max_pages.min(right.max_pages),
        max_browser_actions: left.max_browser_actions.min(right.max_browser_actions),
        max_fan_out: left.max_fan_out.min(right.max_fan_out),
        max_response_bytes: left.max_response_bytes.min(right.max_response_bytes),
        max_browser_rendered_bytes: left
            .max_browser_rendered_bytes
            .min(right.max_browser_rendered_bytes),
    }
}

struct ResolutionState {
    source_key: String,
    finalized: Vec<Candidate>,
    counts: ResolutionCounts,
    parent: ParentAllowance,
    diagnostics: Diagnostics,
    sampler: DiagnosticSampler,
    detail_candidates: u64,
    partial: Option<ResolutionLimitDimension>,
    location_diagnostics: LocationDiagnosticSummary,
}

impl ResolutionState {
    fn new(source_key: String, limits: PhaseLimits, requirements: &Requirements<'_>) -> Self {
        let mut diagnostics = Vec::new();
        if requirements.missing_radius
            || requirements.geo.as_ref().is_some_and(|geo| {
                geo.filter.not_applied_reason()
                    == Some(LocationFilterNotAppliedReason::MissingRadiusKm)
            })
        {
            diagnostics.push(location_filter_missing_radius_diagnostic());
        }
        let mut location_diagnostics = LocationDiagnosticSummary::default();
        if let Some(geo) = &requirements.geo {
            location_diagnostics.observe_request_ambiguities(geo.filter.request_ambiguities());
        }
        Self {
            source_key,
            finalized: Vec::new(),
            counts: ResolutionCounts::default(),
            parent: ParentAllowance::new(limits),
            diagnostics,
            sampler: DiagnosticSampler::new(DIAGNOSTIC_SAMPLE_LIMIT),
            detail_candidates: 0,
            partial: None,
            location_diagnostics,
        }
    }

    async fn process_occurrences(
        &mut self,
        request: &ResolveContext<'_>,
        occurrences: &[PostingOccurrence],
    ) -> Result<(), ResolutionError> {
        self.counts.discovered = checked_add(
            self.counts.discovered,
            u64::try_from(occurrences.len())
                .map_err(|_| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?,
        )?;
        for (index, occurrence) in occurrences.iter().enumerate() {
            cancelled(request.cancellation)?;
            let mut values = CandidateValues::from_occurrence(occurrence);
            if values
                .title
                .as_deref()
                .is_some_and(|title| !request.requirements.matches_title(title))
                || (values.title.is_none() && hint_rejects(occurrence, request.requirements))
            {
                self.counts.rejected = checked_add(self.counts.rejected, 1)?;
                continue;
            }
            if values.is_complete(request.requirements) {
                let matches = values
                    .final_matches(request.requirements)
                    .await
                    .map_err(geo_resolution_failed)?;
                self.location_diagnostics
                    .observe_match_report(matches.1.as_ref());
                if matches.0 {
                    self.finalized
                        .push(values.finalize(&self.source_key, occurrence)?);
                    self.counts.finalized = checked_add(self.counts.finalized, 1)?;
                } else {
                    self.counts.rejected = checked_add(self.counts.rejected, 1)?;
                }
                continue;
            }
            let needed = values.needed(request.requirements);
            if needed.is_empty()
                || needed.iter().any(|field| {
                    !request
                        .compiled_source
                        .detail_capabilities()
                        .contains(*field)
                })
            {
                self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                continue;
            }
            let stop = if self.detail_candidates == PhaseLimits::BACKEND.max_fan_out {
                Some(ResolutionLimitDimension::DetailCandidates)
            } else {
                self.parent.first_exhausted()
            };
            if let Some(dimension) = stop {
                self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                self.counts.budget_skipped = checked_add(
                    self.counts.budget_skipped,
                    u64::try_from(occurrences.len() - index - 1)
                        .map_err(|_| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?,
                )?;
                self.partial = Some(dimension);
                break;
            }

            let requested_fields = RequestedDetailFields::new(needed.iter().copied())
                .map_err(|_| failed(ResolutionFailure::ProtocolInvariant, Vec::new()))?;
            let child_limits = match self.parent.remaining_limits()? {
                ParentAdmission::Admitted(limits) => intersect_limits(
                    limits,
                    request
                        .compiled_source
                        .detail_limits()
                        .ok_or_else(|| failed(ResolutionFailure::ProtocolInvariant, Vec::new()))?,
                ),
                ParentAdmission::Exhausted(dimension) => {
                    self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                    self.counts.budget_skipped = checked_add(
                        self.counts.budget_skipped,
                        u64::try_from(occurrences.len() - index - 1).map_err(|_| {
                            failed(ResolutionFailure::ArithmeticInvariant, Vec::new())
                        })?,
                    )?;
                    self.partial = Some(dimension);
                    break;
                }
            };
            self.detail_candidates = checked_add(self.detail_candidates, 1)?;
            let context = RuntimeExecutionContext::with_cancellation(request.cancellation)
                .with_limits(child_limits);
            let outcome = request
                .detail
                .execute(SourceDetailRequest {
                    compiled_source: request.compiled_source,
                    occurrence,
                    requested_fields: requested_fields.clone(),
                    context,
                })
                .await
                .map_err(|cancelled| {
                    validate_child_report(
                        &cancelled.complete_budget_report,
                        child_limits,
                        |completion| matches!(completion, PhaseCompletion::Cancelled { .. }),
                    )
                    .and_then(|()| self.parent.commit(&cancelled.complete_budget_report))
                    .map_or_else(|error| error, |()| ResolutionError::Cancelled)
                })?;
            if !valid_detail_report(&outcome) {
                return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
            }
            if let Some(report) = outcome.complete_budget_report() {
                validate_child_report(report, child_limits, |_| true)?;
                self.parent.commit(report)?;
            }
            cancelled(request.cancellation)?;
            match outcome {
                SourceDetailOutcome::Completed {
                    fields,
                    dispositions,
                    phase_evidence,
                } => {
                    if let Some(evidence) = phase_evidence {
                        self.diagnostics.extend(evidence.diagnostics);
                    }
                    if !valid_dispositions(&requested_fields, &dispositions)
                        || !patch_is_requested(&requested_fields, &fields)
                    {
                        return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
                    }
                    if dispositions.iter().any(|d| {
                        matches!(
                            d,
                            source_engine::execution::RequestedFieldDisposition::Unavailable { .. }
                                | source_engine::execution::RequestedFieldDisposition::Conflicted { .. }
                                | source_engine::execution::RequestedFieldDisposition::Unsupported { .. }
                        )
                    }) || !values.apply(fields, &needed)
                        || !values.is_complete(request.requirements)
                    {
                        self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                    } else {
                        let matches = values
                            .final_matches(request.requirements)
                            .await
                            .map_err(geo_resolution_failed)?;
                        self.location_diagnostics
                            .observe_match_report(matches.1.as_ref());
                        if matches.0 {
                            self.finalized
                                .push(values.finalize(&self.source_key, occurrence)?);
                            self.counts.finalized = checked_add(self.counts.finalized, 1)?;
                        } else {
                            self.counts.rejected = checked_add(self.counts.rejected, 1)?;
                        }
                    }
                }
                SourceDetailOutcome::CandidateExecutionFailed { .. } => {
                    self.counts.failed = checked_add(self.counts.failed, 1)?;
                    self.sampler.observe("candidate_detail_execution_failed")?;
                }
                SourceDetailOutcome::BudgetExhausted {
                    complete_budget_report,
                    diagnostics,
                } => {
                    self.diagnostics.extend(diagnostics);
                    self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                    self.counts.budget_skipped = checked_add(
                        self.counts.budget_skipped,
                        u64::try_from(occurrences.len() - index - 1).map_err(|_| {
                            failed(ResolutionFailure::ArithmeticInvariant, Vec::new())
                        })?,
                    )?;
                    self.partial = dimension_from_completion(&complete_budget_report.completion)
                        .or(Some(ResolutionLimitDimension::Requests));
                    break;
                }
                SourceDetailOutcome::SourceExecutionFailed { diagnostics, .. } => {
                    self.diagnostics.extend(diagnostics);
                    return Err(failed(
                        ResolutionFailure::SourceDetailExecution,
                        std::mem::take(&mut self.diagnostics),
                    ));
                }
                SourceDetailOutcome::SourceMismatch => {
                    return Err(failed(
                        ResolutionFailure::SourceMismatch,
                        std::mem::take(&mut self.diagnostics),
                    ));
                }
            }
        }
        Ok(())
    }

    fn finish(
        mut self,
        cancellation: &dyn RuntimeCancellation,
    ) -> Result<Resolution, ResolutionError> {
        // Final commit boundary: cancellation releases no counts, completion, or finalized values.
        cancelled(cancellation)?;
        self.diagnostics
            .extend(self.location_diagnostics.into_diagnostics());
        cancelled_counts(&mut self.counts)?;
        validate_counts(&self.counts, self.finalized.len())?;
        Ok(Resolution {
            source_key: self.source_key,
            finalized: self.finalized,
            completion: self
                .partial
                .map(|limit_reached| ResolutionCompletion::Partial { limit_reached })
                .unwrap_or(ResolutionCompletion::Complete),
            counts: self.counts,
            usage: self.parent.usage,
            diagnostics: self.diagnostics,
            candidate_diagnostics: self.sampler.finish(),
        })
    }
}

fn cancelled_counts(counts: &mut ResolutionCounts) -> Result<(), ResolutionError> {
    counts.processed = counts
        .finalized
        .checked_add(counts.rejected)
        .and_then(|value| value.checked_add(counts.unresolved))
        .and_then(|value| value.checked_add(counts.failed))
        .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
    Ok(())
}

fn validate_child_report(
    report: &PhaseExecutionReport,
    limits: PhaseLimits,
    valid_completion: impl FnOnce(&PhaseCompletion) -> bool,
) -> Result<(), ResolutionError> {
    let usage = report.usage;
    if !valid_completion(&report.completion)
        || usage.strategy_attempts > limits.max_strategy_attempts
        || usage.requests > limits.max_requests
        || usage.produced_items > limits.max_produced_items
        || usage.duration_ms > limits.max_duration_ms
        || usage.pages > limits.max_pages
        || usage.browser_actions > limits.max_browser_actions
        || usage.fan_out > limits.max_fan_out
        || usage.response_bytes > limits.max_response_bytes
        || usage.browser_rendered_bytes > limits.max_browser_rendered_bytes
    {
        return Err(failed(ResolutionFailure::ReportAboveAllowance, Vec::new()));
    }
    Ok(())
}

fn valid_detail_report(outcome: &SourceDetailOutcome) -> bool {
    match outcome {
        SourceDetailOutcome::Completed {
            phase_evidence: Some(evidence),
            ..
        } => matches!(
            evidence.complete_budget_report.completion,
            PhaseCompletion::Accepted | PhaseCompletion::PolicyUnsatisfied
        ),
        SourceDetailOutcome::Completed {
            phase_evidence: None,
            ..
        }
        | SourceDetailOutcome::SourceMismatch => true,
        SourceDetailOutcome::BudgetExhausted {
            complete_budget_report,
            ..
        } => matches!(
            complete_budget_report.completion,
            PhaseCompletion::BudgetExhausted { .. }
        ),
        SourceDetailOutcome::CandidateExecutionFailed {
            complete_budget_report,
            ..
        } => matches!(
            complete_budget_report.completion,
            PhaseCompletion::PolicyUnsatisfied
        ),
        SourceDetailOutcome::SourceExecutionFailed {
            typed_failure: source_engine::execution::SourceDetailFailure::PhaseExecution { .. },
            complete_budget_report: Some(report),
            ..
        } => matches!(report.completion, PhaseCompletion::ExecutionFailed),
        SourceDetailOutcome::SourceExecutionFailed {
            typed_failure: source_engine::execution::SourceDetailFailure::PhasePreStart { .. },
            complete_budget_report: None,
            ..
        } => true,
        SourceDetailOutcome::SourceExecutionFailed { .. } => false,
    }
}

fn patch_is_requested(requested: &RequestedDetailFields, patch: &DetailPatch) -> bool {
    (patch.title.is_none() || requested.contains(DetailField::Title))
        && (patch.company.is_none() || requested.contains(DetailField::Company))
        && (patch.locations.is_none() || requested.contains(DetailField::Locations))
        && (patch.description_text.is_none() || requested.contains(DetailField::DescriptionText))
}

fn valid_dispositions(
    requested: &RequestedDetailFields,
    dispositions: &[source_engine::execution::RequestedFieldDisposition],
) -> bool {
    let fields = dispositions.iter().map(|d| d.field()).collect::<Vec<_>>();
    fields.len() == requested.iter().count()
        && requested
            .iter()
            .all(|field| fields.iter().filter(|f| **f == field).count() == 1)
}

struct CandidateValues {
    title: Option<String>,
    company: Option<String>,
    locations: Vec<String>,
    url: Option<String>,
}
impl CandidateValues {
    fn from_occurrence(o: &PostingOccurrence) -> Self {
        Self {
            title: o
                .provider_values
                .title
                .as_deref()
                .map(collapse_whitespace)
                .filter(|v| !v.is_empty()),
            company: o
                .provider_values
                .company
                .as_deref()
                .map(collapse_whitespace)
                .filter(|v| !v.is_empty()),
            locations: normalize_locations(o.provider_values.locations.clone()),
            url: absolute_url(&o.reference.provider_url),
        }
    }
    fn needed(&self, requirements: &Requirements<'_>) -> Vec<DetailField> {
        let mut out = Vec::new();
        if self.title.is_none() {
            out.push(DetailField::Title);
        }
        if self.company.is_none() {
            out.push(DetailField::Company);
        }
        if requirements.requires_locations() && self.locations.is_empty() {
            out.push(DetailField::Locations);
        }
        out
    }
    fn is_complete(&self, requirements: &Requirements<'_>) -> bool {
        self.title.is_some()
            && self.company.is_some()
            && self.url.is_some()
            && (!requirements.requires_locations() || !self.locations.is_empty())
    }
    async fn final_matches(
        &self,
        requirements: &Requirements<'_>,
    ) -> Result<(bool, Option<LocationFilterMatchReport>), LocationFilterError> {
        let title_matches = self
            .title
            .as_deref()
            .is_some_and(|title| requirements.matches_title(title));
        if !title_matches || self.company.is_none() || self.url.is_none() {
            return Ok((false, None));
        }
        requirements.matches_locations(&self.locations).await
    }
    fn apply(&mut self, patch: DetailPatch, requested: &[DetailField]) -> bool {
        let mut progress = false;
        macro_rules! scalar {
            ($field:ident, $value:expr) => {
                if let Some(value) = $value {
                    let value = collapse_whitespace(&value);
                    if value.is_empty() {
                        return false;
                    }
                    match &self.$field {
                        Some(old) if old != &value => return false,
                        None => {
                            self.$field = Some(value);
                            progress = true;
                        }
                        _ => {}
                    }
                }
            };
        }
        if requested.contains(&DetailField::Title) {
            scalar!(title, patch.title);
        }
        if requested.contains(&DetailField::Company) {
            scalar!(company, patch.company);
        }
        if requested.contains(&DetailField::Locations) {
            if let Some(values) = patch.locations {
                let values = normalize_locations(values);
                if values.is_empty() {
                    return false;
                }
                if self.locations.is_empty() {
                    self.locations = values;
                    progress = true;
                } else if self.locations != values {
                    return false;
                }
            }
        }
        progress
    }
    fn finalize(
        self,
        source_key: &str,
        occurrence: &PostingOccurrence,
    ) -> Result<Candidate, ResolutionError> {
        Ok(Candidate {
            source_key: source_key.to_string(),
            identity: occurrence.identity.clone(),
            title: self
                .title
                .ok_or_else(|| failed(ResolutionFailure::ProtocolInvariant, Vec::new()))?,
            company: self
                .company
                .ok_or_else(|| failed(ResolutionFailure::ProtocolInvariant, Vec::new()))?,
            url: self
                .url
                .ok_or_else(|| failed(ResolutionFailure::ProtocolInvariant, Vec::new()))?,
            locations: self.locations,
        })
    }
}

fn absolute_url(value: &str) -> Option<String> {
    Url::parse(value.trim())
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
        .map(Into::into)
}
fn hint_rejects(o: &PostingOccurrence, requirements: &Requirements<'_>) -> bool {
    o.hints
        .get("title")
        .filter(|h| h.hint_use == Some(source_engine::execution::HintUse::SearchPrefilter))
        .is_some_and(|h| !requirements.matches_title(&collapse_whitespace(&h.value)))
}

enum ParentAdmission {
    Admitted(PhaseLimits),
    Exhausted(ResolutionLimitDimension),
}

struct ParentAllowance {
    limits: PhaseLimits,
    usage: PhaseUsage,
    started: std::time::Instant,
}
impl ParentAllowance {
    fn new(limits: PhaseLimits) -> Self {
        Self {
            limits,
            usage: PhaseUsage::default(),
            started: std::time::Instant::now(),
        }
    }
    fn first_exhausted(&self) -> Option<ResolutionLimitDimension> {
        [
            (
                self.usage.strategy_attempts == self.limits.max_strategy_attempts,
                ResolutionLimitDimension::StrategyAttempts,
            ),
            (
                self.usage.requests == self.limits.max_requests,
                ResolutionLimitDimension::Requests,
            ),
            (
                self.usage.produced_items == self.limits.max_produced_items,
                ResolutionLimitDimension::ProducedItems,
            ),
            (
                self.usage.response_bytes == self.limits.max_response_bytes,
                ResolutionLimitDimension::ResponseBytes,
            ),
            (
                self.elapsed_ms() >= self.limits.max_duration_ms,
                ResolutionLimitDimension::Duration,
            ),
            (
                self.usage.pages == self.limits.max_pages,
                ResolutionLimitDimension::Pages,
            ),
            (
                self.usage.browser_actions == self.limits.max_browser_actions,
                ResolutionLimitDimension::BrowserActions,
            ),
            (
                self.usage.fan_out == self.limits.max_fan_out,
                ResolutionLimitDimension::FanOut,
            ),
            (
                self.usage.browser_rendered_bytes == self.limits.max_browser_rendered_bytes,
                ResolutionLimitDimension::BrowserRenderedBytes,
            ),
        ]
        .into_iter()
        .find_map(|(exhausted, dimension)| exhausted.then_some(dimension))
    }
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    fn remaining_limits(&self) -> Result<ParentAdmission, ResolutionError> {
        let elapsed_ms = self.elapsed_ms();
        if elapsed_ms >= self.limits.max_duration_ms {
            return Ok(ParentAdmission::Exhausted(
                ResolutionLimitDimension::Duration,
            ));
        }
        macro_rules! rem {
            ($limit:ident, $usage:ident, $dimension:expr) => {{
                let remaining = self
                    .limits
                    .$limit
                    .checked_sub(self.usage.$usage)
                    .ok_or_else(|| failed(ResolutionFailure::ReportAboveAllowance, Vec::new()))?;
                if remaining == 0 {
                    return Ok(ParentAdmission::Exhausted($dimension));
                }
                remaining
            }};
        }
        Ok(ParentAdmission::Admitted(PhaseLimits {
            max_strategy_attempts: rem!(
                max_strategy_attempts,
                strategy_attempts,
                ResolutionLimitDimension::StrategyAttempts
            ),
            max_requests: rem!(max_requests, requests, ResolutionLimitDimension::Requests),
            max_produced_items: rem!(
                max_produced_items,
                produced_items,
                ResolutionLimitDimension::ProducedItems
            ),
            // This value and the exhaustion decision derive from the same elapsed snapshot.
            max_duration_ms: self.limits.max_duration_ms - elapsed_ms,
            max_pages: rem!(max_pages, pages, ResolutionLimitDimension::Pages),
            max_browser_actions: rem!(
                max_browser_actions,
                browser_actions,
                ResolutionLimitDimension::BrowserActions
            ),
            max_fan_out: rem!(max_fan_out, fan_out, ResolutionLimitDimension::FanOut),
            max_response_bytes: rem!(
                max_response_bytes,
                response_bytes,
                ResolutionLimitDimension::ResponseBytes
            ),
            max_browser_rendered_bytes: rem!(
                max_browser_rendered_bytes,
                browser_rendered_bytes,
                ResolutionLimitDimension::BrowserRenderedBytes
            ),
        }))
    }
    fn commit(&mut self, report: &PhaseExecutionReport) -> Result<(), ResolutionError> {
        macro_rules! add {
            ($field:ident, $limit:ident) => {{
                self.usage.$field = self
                    .usage
                    .$field
                    .checked_add(report.usage.$field)
                    .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
                if self.usage.$field > self.limits.$limit {
                    return Err(failed(ResolutionFailure::ReportAboveAllowance, Vec::new()));
                }
            }};
        }
        add!(strategy_attempts, max_strategy_attempts);
        add!(requests, max_requests);
        add!(produced_items, max_produced_items);
        self.usage.duration_ms = self
            .usage
            .duration_ms
            .checked_add(report.usage.duration_ms)
            .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
        // Duration admission uses one monotonic deadline, while the result still reports the
        // exact sum of sequential child durations. A contradictory child sequence above the
        // parent ceiling is an invariant failure rather than a committed over-limit report.
        if self.usage.duration_ms > self.limits.max_duration_ms {
            return Err(failed(ResolutionFailure::ReportAboveAllowance, Vec::new()));
        }
        add!(pages, max_pages);
        add!(browser_actions, max_browser_actions);
        add!(fan_out, max_fan_out);
        add!(response_bytes, max_response_bytes);
        add!(browser_rendered_bytes, max_browser_rendered_bytes);
        Ok(())
    }
}

struct DiagnosticSampler {
    limit: usize,
    totals: BTreeMap<String, u64>,
    samples: Diagnostics,
    omitted: u64,
}
impl DiagnosticSampler {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            totals: BTreeMap::new(),
            samples: Vec::new(),
            omitted: 0,
        }
    }
    fn observe(&mut self, code: &'static str) -> Result<(), ResolutionError> {
        let count = self.totals.entry(code.to_string()).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
        if self.samples.len() < self.limit {
            self.samples.push(sanitized_diagnostic(
                code,
                "Candidate Detail execution failed",
                DiagnosticSeverity::Warning,
            ));
        } else {
            self.omitted = self
                .omitted
                .checked_add(1)
                .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
        }
        Ok(())
    }
    fn finish(self) -> CandidateDiagnosticSummary {
        CandidateDiagnosticSummary {
            counts_by_code: self.totals,
            samples: self.samples,
            sample_limit: self.limit as u64,
            candidate_diagnostics_omitted: self.omitted,
        }
    }
}
#[derive(Default)]
struct LocationDiagnosticSummary {
    unresolved_location_count: u64,
    affected_candidate_count: u64,
    unresolved_samples: Vec<String>,
    request_ambiguity_count: u64,
    request_ambiguities: Vec<LocationResolutionAmbiguity>,
    candidate_ambiguity_count: u64,
    candidate_ambiguity_samples: Vec<LocationResolutionAmbiguity>,
}

impl LocationDiagnosticSummary {
    fn observe_request_ambiguities(&mut self, values: &[LocationResolutionAmbiguity]) {
        self.request_ambiguity_count = u64::try_from(values.len()).unwrap_or(u64::MAX);
        self.request_ambiguities = values.iter().take(5).map(sanitize_ambiguity).collect();
    }

    fn observe_match_report(&mut self, report: Option<&LocationFilterMatchReport>) {
        let Some(report) = report else { return };
        if !report.unresolved_candidate_locations.is_empty() {
            self.affected_candidate_count = self.affected_candidate_count.saturating_add(1);
            self.unresolved_location_count = self.unresolved_location_count.saturating_add(
                u64::try_from(report.unresolved_candidate_locations.len()).unwrap_or(u64::MAX),
            );
            for value in &report.unresolved_candidate_locations {
                let value = sanitize_geo_value(value);
                if self.unresolved_samples.len() < 5 && !self.unresolved_samples.contains(&value) {
                    self.unresolved_samples.push(value);
                }
            }
        }
        self.candidate_ambiguity_count = self
            .candidate_ambiguity_count
            .saturating_add(u64::try_from(report.candidate_ambiguities.len()).unwrap_or(u64::MAX));
        for ambiguity in &report.candidate_ambiguities {
            if self.candidate_ambiguity_samples.len() < 5 {
                self.candidate_ambiguity_samples
                    .push(sanitize_ambiguity(ambiguity));
            }
        }
    }

    fn into_diagnostics(self) -> Diagnostics {
        let mut diagnostics = Vec::new();
        if self.unresolved_location_count > 0 {
            diagnostics.push(Diagnostic {
                category: DiagnosticCategory::Runtime,
                code: "location_filter_candidate_locations_unresolved".to_string(),
                message: "Some candidate location values could not be resolved and did not contribute to active location filter matches.".to_string(),
                severity: DiagnosticSeverity::Warning,
                path: "/candidates/*/locations".to_string(),
                strategy_key: None,
                details: Some(serde_json::json!({
                    "unresolvedLocationCount": self.unresolved_location_count,
                    "affectedCandidateCount": self.affected_candidate_count,
                    "samples": self.unresolved_samples,
                    "sampleLimit": 5
                })),
            });
        }
        if self.request_ambiguity_count > 0 || self.candidate_ambiguity_count > 0 {
            diagnostics.push(Diagnostic {
                category: DiagnosticCategory::Runtime,
                code: "location_filter_ambiguous_locations".to_string(),
                message: "Some locations resolved to multiple geo points; location filtering considered all resolved locations.".to_string(),
                severity: DiagnosticSeverity::Info,
                path: "/locations".to_string(),
                strategy_key: None,
                details: Some(serde_json::json!({
                    "requestLocationAmbiguityCount": self.request_ambiguity_count,
                    "candidateLocationAmbiguityCount": self.candidate_ambiguity_count,
                    "requestSamples": ambiguity_json(&self.request_ambiguities),
                    "candidateSamples": ambiguity_json(&self.candidate_ambiguity_samples),
                    "sampleLimit": 5
                })),
            });
        }
        diagnostics
    }
}

fn sanitize_geo_value(value: &str) -> String {
    collapse_whitespace(value).chars().take(120).collect()
}
fn sanitize_ambiguity(value: &LocationResolutionAmbiguity) -> LocationResolutionAmbiguity {
    LocationResolutionAmbiguity {
        input: sanitize_geo_value(&value.input),
        resolved_labels: value
            .resolved_labels
            .iter()
            .take(5)
            .map(|v| sanitize_geo_value(v))
            .collect(),
    }
}
fn ambiguity_json(values: &[LocationResolutionAmbiguity]) -> Vec<serde_json::Value> {
    values
        .iter()
        .take(5)
        .map(|value| {
            serde_json::json!({
                "input": value.input,
                "resolvedLabels": value.resolved_labels,
            })
        })
        .collect()
}
fn location_filter_missing_radius_diagnostic() -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: "location_filter_not_applied_missing_radius_km".to_string(),
        message: "Search Request locations were configured, but radiusKm is missing; location filtering was not applied.".to_string(),
        severity: DiagnosticSeverity::Warning,
        path: "/radiusKm".to_string(),
        strategy_key: None,
        details: None,
    }
}
fn geo_resolution_failed(_error: LocationFilterError) -> ResolutionError {
    failed(
        ResolutionFailure::GeoResolution,
        vec![Diagnostic {
            category: DiagnosticCategory::Runtime,
            code: "location_filter_geo_resolution_failed".to_string(),
            message: "Candidate location resolution failed at runtime".to_string(),
            severity: DiagnosticSeverity::Error,
            path: "/candidates/*/locations".to_string(),
            strategy_key: None,
            details: None,
        }],
    )
}

fn sanitized_diagnostic(code: &str, message: &str, severity: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.to_string(),
        message: message.to_string(),
        severity,
        path: "/candidates".to_string(),
        strategy_key: None,
        details: None,
    }
}
fn cancelled(c: &dyn RuntimeCancellation) -> Result<(), ResolutionError> {
    if c.is_cancelled() {
        Err(ResolutionError::Cancelled)
    } else {
        Ok(())
    }
}
fn failed(failure: ResolutionFailure, diagnostics: Diagnostics) -> ResolutionError {
    ResolutionError::Failed {
        failure,
        diagnostics,
    }
}
fn checked_add(a: u64, b: u64) -> Result<u64, ResolutionError> {
    a.checked_add(b)
        .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))
}
fn validate_counts(c: &ResolutionCounts, finalized_len: usize) -> Result<(), ResolutionError> {
    let processed = c
        .finalized
        .checked_add(c.rejected)
        .and_then(|v| v.checked_add(c.unresolved))
        .and_then(|v| v.checked_add(c.failed))
        .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
    let discovered = processed
        .checked_add(c.budget_skipped)
        .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
    if processed != c.processed
        || discovered != c.discovered
        || u64::try_from(finalized_len).ok() != Some(c.finalized)
    {
        return Err(failed(ResolutionFailure::ArithmeticInvariant, Vec::new()));
    }
    Ok(())
}
fn dimension_from_completion(c: &PhaseCompletion) -> Option<ResolutionLimitDimension> {
    let PhaseCompletion::BudgetExhausted { exhaustion } = c else {
        return None;
    };
    use source_engine::execution::AllowanceDimension::*;
    Some(match exhaustion.dimension {
        StrategyAttempts => ResolutionLimitDimension::StrategyAttempts,
        Requests => ResolutionLimitDimension::Requests,
        ProducedItems => ResolutionLimitDimension::ProducedItems,
        Duration => ResolutionLimitDimension::Duration,
        Pages => ResolutionLimitDimension::Pages,
        BrowserActions => ResolutionLimitDimension::BrowserActions,
        FanOut => ResolutionLimitDimension::FanOut,
        ResponseBytes => ResolutionLimitDimension::ResponseBytes,
        BrowserRenderedBytes => ResolutionLimitDimension::BrowserRenderedBytes,
        LogicalWaits => ResolutionLimitDimension::Duration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepted(usage: PhaseUsage) -> PhaseExecutionReport {
        PhaseExecutionReport {
            usage,
            completion: PhaseCompletion::Accepted,
        }
    }

    #[test]
    fn limit_intersection_uses_the_tightest_value_for_every_dimension() {
        let left = PhaseLimits::BACKEND;
        let right = PhaseLimits {
            max_strategy_attempts: left.max_strategy_attempts - 1,
            max_requests: left.max_requests - 1,
            max_produced_items: left.max_produced_items - 1,
            max_duration_ms: left.max_duration_ms - 1,
            max_pages: left.max_pages - 1,
            max_browser_actions: left.max_browser_actions - 1,
            max_fan_out: left.max_fan_out - 1,
            max_response_bytes: left.max_response_bytes - 1,
            max_browser_rendered_bytes: left.max_browser_rendered_bytes - 1,
        };

        assert_eq!(intersect_limits(left, right), right);
        assert_eq!(intersect_limits(right, left), right);
    }

    #[test]
    fn cumulative_child_reports_reach_the_exact_parent_limit() {
        let limits = PhaseLimits {
            max_requests: 3,
            max_duration_ms: 100,
            ..PhaseLimits::BACKEND
        };
        let mut parent = ParentAllowance::new(limits);
        parent
            .commit(&accepted(PhaseUsage {
                requests: 2,
                duration_ms: 40,
                ..PhaseUsage::default()
            }))
            .unwrap();
        let ParentAdmission::Admitted(remaining) = parent.remaining_limits().unwrap() else {
            panic!("one request must remain");
        };
        assert_eq!(remaining.max_requests, 1);

        parent
            .commit(&accepted(PhaseUsage {
                requests: 1,
                duration_ms: 60,
                ..PhaseUsage::default()
            }))
            .unwrap();
        assert_eq!(parent.usage.requests, 3);
        assert_eq!(parent.usage.duration_ms, 100);
        assert!(matches!(
            parent.remaining_limits().unwrap(),
            ParentAdmission::Exhausted(ResolutionLimitDimension::Requests)
        ));
    }

    #[test]
    fn child_reports_above_their_exact_allowance_fail_closed() {
        let limits = PhaseLimits {
            max_requests: 1,
            ..PhaseLimits::BACKEND
        };
        let report = accepted(PhaseUsage {
            requests: 2,
            ..PhaseUsage::default()
        });

        assert!(matches!(
            validate_child_report(&report, limits, |completion| {
                matches!(completion, PhaseCompletion::Accepted)
            }),
            Err(ResolutionError::Failed {
                failure: ResolutionFailure::ReportAboveAllowance,
                ..
            })
        ));
    }
}
