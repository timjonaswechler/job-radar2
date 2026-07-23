//! Source-scoped Candidate Resolution and finalized-only Search Run boundary.

use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::Mutex,
};

use regex::{Regex, RegexBuilder};
use serde::Serialize;
use url::Url;

use crate::{
    geo::{
        GeoResolver, LocationFilterMatchReport, LocationFilterNotAppliedReason,
        LocationMatchOutcome, LocationResolutionAmbiguity, PreparedLocationFilter,
    },
    profile_dsl::{
        compiler::CompiledSource,
        diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Diagnostics},
        documents::PhaseLimits,
        occurrence::{
            DetailField, DetailPatch, PostingOccurrence, PostingOccurrenceIdentity,
            RequestedDetailFields,
        },
        runtime::{
            execute_discovery, BrowserAcquisition, DiscoveryBrowserAdapter, PhaseBrowser,
            PhaseCompletion, PhaseExecutionReport, PhaseOutcome, PhaseRunError, PhaseUsage,
            PolicyOutcome, ProfileHttpClient, RuntimeCancellation, RuntimeExecutionContext,
            SourceDetailExecution, SourceDetailOutcome, SourceDetailRequest,
        },
    },
    search::{
        normalization::{collapse_whitespace, normalize_locations},
        request::{SearchRule, SearchRuleKind, SearchRuleTarget},
    },
};

pub const CANDIDATE_DIAGNOSTIC_SAMPLE_LIMIT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionLimitDimension {
    DiscoveryBatches,
    DiscoveredItems,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionCeilings {
    pub max_batch_size: u64,
    pub max_discovery_batches: u64,
    pub max_discovered_items: u64,
    pub max_detail_candidates: u64,
    pub phase: PhaseLimits,
}

impl ResolutionCeilings {
    pub fn validate(self) -> Result<Self, ResolutionFailure> {
        let backend = PhaseLimits::BACKEND;
        if self.max_batch_size == 0
            || self.max_discovery_batches == 0
            || self.max_discovered_items == 0
            || self.max_detail_candidates == 0
            || !self.phase.all_positive()
            || !self.phase.within(backend)
            // These operation-level ceilings are derived only from existing immutable
            // backend dimensions; Candidate Resolution introduces no product numbers.
            || self.max_batch_size > backend.max_produced_items
            || self.max_discovery_batches > backend.max_pages
            || self.max_discovered_items > backend.max_produced_items
            || self.max_detail_candidates > backend.max_fan_out
        {
            return Err(ResolutionFailure::InvalidInput);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct CompiledSearchRequirements<'a> {
    include: Vec<CompiledRule>,
    exclude: Vec<CompiledRule>,
    geo: Option<GeoRequirements<'a>>,
    missing_radius: bool,
    geo_runtime_failure: bool,
}

#[derive(Clone)]
struct GeoRequirements<'a> {
    filter: PreparedLocationFilter,
    resolver: &'a dyn GeoResolver,
}

impl std::fmt::Debug for GeoRequirements<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeoRequirements")
            .field("filter", &self.filter)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct CompiledRule {
    matcher: CompiledRuleMatcher,
}

#[derive(Clone, Debug)]
enum CompiledRuleMatcher {
    Text(String),
    Regex(Regex),
}

impl<'a> CompiledSearchRequirements<'a> {
    /// Compiles matching when no radius is configured. Configured locations without a radius
    /// deliberately do not apply a location filter, preserving established Search Run semantics.
    pub fn compile(
        include: &[SearchRule],
        exclude: &[SearchRule],
        locations: &[String],
        radius_km: Option<i64>,
    ) -> Result<Self, RequirementsCompilationFailure> {
        if radius_km.is_some() {
            return Err(RequirementsCompilationFailure::RadiusRequiresGeoResolver);
        }
        Ok(Self {
            include: compile_rules(include, false)?,
            exclude: compile_rules(exclude, true)?,
            geo: None,
            missing_radius: !locations.is_empty(),
            geo_runtime_failure: false,
        })
    }

    /// Compiles radius matching using the same prepared filter and resolver as the existing
    /// Search Request geo semantics. Geo matching remains inside final Candidate Resolution.
    pub async fn compile_with_geo(
        include: &[SearchRule],
        exclude: &[SearchRule],
        locations: &[String],
        radius_km: Option<i64>,
        resolver: &'a dyn GeoResolver,
    ) -> Result<Self, String> {
        let (geo, geo_runtime_failure) =
            match crate::geo::prepare_location_filter(resolver, locations, radius_km).await {
                Ok(filter) => (Some(GeoRequirements { filter, resolver }), false),
                Err(error)
                    if error.starts_with("Search Request location could not be resolved:") =>
                {
                    return Err(error);
                }
                Err(_) => (None, true),
            };
        Ok(Self {
            include: compile_rules(include, false).map_err(requirements_error)?,
            exclude: compile_rules(exclude, true).map_err(requirements_error)?,
            geo,
            missing_radius: false,
            geo_runtime_failure,
        })
    }

    fn matches_title(&self, title: &str) -> bool {
        let included = self.include.iter().any(|rule| rule.matches(title));
        included && !self.exclude.iter().any(|rule| rule.matches(title))
    }

    async fn matches_locations(
        &self,
        locations: &[String],
    ) -> Result<(bool, Option<LocationFilterMatchReport>), String> {
        if let Some(geo) = &self.geo {
            let report = geo
                .filter
                .matches_candidate_with_report(geo.resolver, locations)
                .await?;
            let matched = matches!(
                report.outcome,
                LocationMatchOutcome::Applied { matched: true }
                    | LocationMatchOutcome::NotApplied { .. }
            );
            return Ok((matched, Some(report)));
        }
        Ok((true, None))
    }

    fn requires_locations(&self) -> bool {
        self.geo
            .as_ref()
            .is_some_and(|geo| geo.filter.not_applied_reason().is_none())
    }
}

fn requirements_error(failure: RequirementsCompilationFailure) -> String {
    format!("Search Request matching requirements are invalid: {failure:?}")
}

impl CompiledRule {
    fn matches(&self, value: &str) -> bool {
        match &self.matcher {
            CompiledRuleMatcher::Text(needle) => value.to_lowercase().contains(needle),
            CompiledRuleMatcher::Regex(regex) => regex.is_match(value),
        }
    }
}

fn compile_rules(
    rules: &[SearchRule],
    case_insensitive_regex: bool,
) -> Result<Vec<CompiledRule>, RequirementsCompilationFailure> {
    rules
        .iter()
        .map(|rule| {
            if rule.target != SearchRuleTarget::Title {
                return Err(RequirementsCompilationFailure::UnsupportedRuleTarget);
            }
            let matcher = match rule.kind {
                SearchRuleKind::Text => CompiledRuleMatcher::Text(rule.value.to_lowercase()),
                SearchRuleKind::Regex => CompiledRuleMatcher::Regex(
                    RegexBuilder::new(&rule.value)
                        .case_insensitive(case_insensitive_regex)
                        .build()
                        .map_err(|_| RequirementsCompilationFailure::InvalidRegex)?,
                ),
            };
            Ok(CompiledRule { matcher })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequirementsCompilationFailure {
    InvalidRegex,
    UnsupportedRuleTarget,
    RadiusRequiresGeoResolver,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedCandidate {
    source_key: String,
    identity: PostingOccurrenceIdentity,
    title: String,
    company: String,
    url: String,
    locations: Vec<String>,
}

impl FinalizedCandidate {
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionReport {
    pub usage: PhaseUsage,
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
pub struct SourceResolution {
    pub source_key: String,
    pub finalized: Vec<FinalizedCandidate>,
    pub completion: ResolutionCompletion,
    pub counts: ResolutionCounts,
    pub remaining: Option<u64>,
    pub report: ResolutionReport,
    pub diagnostics: Diagnostics,
    pub candidate_diagnostics: CandidateDiagnosticSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFailure {
    InvalidInput,
    DiscoveryExecution,
    SourceDetailExecution,
    GeoResolution,
    SourceMismatch,
    ProtocolInvariant,
    ArithmeticInvariant,
    ReportAboveAllowance,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceResolutionError {
    Failed {
        failure: ResolutionFailure,
        diagnostics: Diagnostics,
    },
    Cancelled,
}

pub struct SourceResolutionRequest<'a> {
    pub compiled_source: &'a CompiledSource,
    pub requirements: &'a CompiledSearchRequirements<'a>,
    pub ceilings: ResolutionCeilings,
    pub cancellation: &'a dyn RuntimeCancellation,
    pub discovery: SourceDiscovery<'a>,
    pub detail: &'a dyn SourceDetailExecution,
}

/// Operation-owned Discovery configuration. The batch protocol remains an implementation detail
/// of Candidate Resolution.
pub struct SourceDiscovery<'a> {
    execution: SourceDiscoveryKind<'a>,
}

enum SourceDiscoveryKind<'a> {
    ProfileDsl {
        fetcher: &'a (dyn ProfileHttpClient + Sync),
        acquisition: &'a dyn BrowserAcquisition,
    },
    Scripted(&'a ScriptedSourceDiscoveryExecution),
}

impl<'a> SourceDiscovery<'a> {
    pub fn profile_dsl(
        fetcher: &'a (dyn ProfileHttpClient + Sync),
        acquisition: &'a dyn BrowserAcquisition,
    ) -> Self {
        Self {
            execution: SourceDiscoveryKind::ProfileDsl {
                fetcher,
                acquisition,
            },
        }
    }

    #[doc(hidden)]
    pub fn scripted(execution: &'a ScriptedSourceDiscoveryExecution) -> Self {
        Self {
            execution: SourceDiscoveryKind::Scripted(execution),
        }
    }

    async fn execute_batch(&self, request: DiscoveryBatchRequest<'_>) -> DiscoveryBatchResult {
        match self.execution {
            SourceDiscoveryKind::ProfileDsl {
                fetcher,
                acquisition,
            } => execute_profile_dsl_batch(fetcher, acquisition, request).await,
            SourceDiscoveryKind::Scripted(execution) => execution.execute_batch(request).await,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DiscoveryContinuation {
    source_key: String,
    value: String,
}

#[derive(Clone)]
struct DiscoveryBatchRequest<'a> {
    pub compiled_source: &'a CompiledSource,
    pub maximum: u64,
    pub limits: PhaseLimits,
    pub context: RuntimeExecutionContext<'a>,
    continuation: Option<&'a DiscoveryContinuation>,
}

#[derive(Clone, Debug)]
struct DiscoveryBatch {
    pub occurrences: Vec<PostingOccurrence>,
    pub exhausted: bool,
    pub remaining: Option<u64>,
    pub complete_budget_report: PhaseExecutionReport,
    pub diagnostics: Diagnostics,
    continuation: Option<DiscoveryContinuation>,
}

#[derive(Clone, Debug)]
enum DiscoveryBatchFailure {
    NotStarted,
    BudgetExhausted {
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    Cancelled {
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    Execution {
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    UnboundedMaterializedOutput,
}

type DiscoveryBatchResult = Result<DiscoveryBatch, DiscoveryBatchFailure>;

/// Strict external integration-test fixture for the sealed Discovery seam.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct ScriptedDiscoveryBatch {
    pub expected_continuation: Option<String>,
    pub expected_maximum: u64,
    pub expected_limits: PhaseLimits,
    pub occurrences: Vec<PostingOccurrence>,
    pub exhausted: bool,
    pub remaining: Option<u64>,
    pub continuation: Option<String>,
    pub continuation_source_key: Option<String>,
    pub complete_budget_report: PhaseExecutionReport,
    pub diagnostics: Diagnostics,
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub enum ScriptedDiscoveryOutcome {
    Batch(ScriptedDiscoveryBatch),
    BudgetExhausted {
        expected_continuation: Option<String>,
        expected_maximum: u64,
        expected_limits: PhaseLimits,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    Cancelled {
        expected_continuation: Option<String>,
        expected_maximum: u64,
        expected_limits: PhaseLimits,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
    ExecutionFailed {
        expected_continuation: Option<String>,
        expected_maximum: u64,
        expected_limits: PhaseLimits,
        complete_budget_report: PhaseExecutionReport,
        diagnostics: Diagnostics,
    },
}

impl From<ScriptedDiscoveryBatch> for ScriptedDiscoveryOutcome {
    fn from(value: ScriptedDiscoveryBatch) -> Self {
        Self::Batch(value)
    }
}

#[doc(hidden)]
pub struct ScriptedSourceDiscoveryExecution {
    source_key: String,
    script: Mutex<VecDeque<ScriptedDiscoveryOutcome>>,
    calls: Mutex<Vec<Option<String>>>,
    last_duration_limit: Mutex<Option<u64>>,
}

impl ScriptedSourceDiscoveryExecution {
    pub fn new(
        source_key: impl Into<String>,
        batches: impl IntoIterator<Item = ScriptedDiscoveryBatch>,
    ) -> Self {
        Self::new_outcomes(source_key, batches.into_iter().map(Into::into))
    }
    pub fn new_outcomes(
        source_key: impl Into<String>,
        outcomes: impl IntoIterator<Item = ScriptedDiscoveryOutcome>,
    ) -> Self {
        Self {
            source_key: source_key.into(),
            script: Mutex::new(outcomes.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
            last_duration_limit: Mutex::new(None),
        }
    }
    pub fn recorded_continuations(&self) -> Vec<Option<String>> {
        self.calls.lock().unwrap().clone()
    }
    pub fn assert_finished(&self) {
        assert!(
            self.script.lock().unwrap().is_empty(),
            "scripted Discovery outcomes remain"
        )
    }
}

impl ScriptedSourceDiscoveryExecution {
    async fn execute_batch(&self, request: DiscoveryBatchRequest<'_>) -> DiscoveryBatchResult {
        let actual_continuation = request.continuation.map(|value| value.value.clone());
        self.calls.lock().unwrap().push(actual_continuation.clone());
        if request.compiled_source.execution_plan.source.key != self.source_key {
            return Err(DiscoveryBatchFailure::NotStarted);
        }
        let scripted = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(DiscoveryBatchFailure::NotStarted)?;
        let check = |expected_continuation: &Option<String>,
                     expected_maximum: u64,
                     expected_limits: PhaseLimits| {
            assert_eq!(
                &actual_continuation, expected_continuation,
                "unexpected Discovery continuation"
            );
            assert_eq!(
                request.maximum, expected_maximum,
                "unexpected Discovery maximum"
            );
            let mut expected_without_duration = expected_limits;
            expected_without_duration.max_duration_ms = request.limits.max_duration_ms;
            assert_eq!(
                request.limits, expected_without_duration,
                "unexpected Discovery limits other than duration"
            );
            assert!(
                request.limits.max_duration_ms > 0
                    && request.limits.max_duration_ms <= expected_limits.max_duration_ms,
                "Discovery duration must be positive and no greater than its expected upper bound"
            );
            let mut previous = self.last_duration_limit.lock().unwrap();
            assert!(
                previous.is_none_or(|prior| request.limits.max_duration_ms <= prior),
                "Discovery duration limits must tighten monotonically"
            );
            *previous = Some(request.limits.max_duration_ms);
        };
        match scripted {
            ScriptedDiscoveryOutcome::Batch(scripted) => {
                check(
                    &scripted.expected_continuation,
                    scripted.expected_maximum,
                    scripted.expected_limits,
                );
                let continuation = scripted.continuation.map(|value| DiscoveryContinuation {
                    source_key: scripted
                        .continuation_source_key
                        .unwrap_or_else(|| self.source_key.clone()),
                    value,
                });
                Ok(DiscoveryBatch {
                    occurrences: scripted.occurrences,
                    exhausted: scripted.exhausted,
                    remaining: scripted.remaining,
                    complete_budget_report: scripted.complete_budget_report,
                    diagnostics: scripted.diagnostics,
                    continuation,
                })
            }
            ScriptedDiscoveryOutcome::BudgetExhausted {
                expected_continuation,
                expected_maximum,
                expected_limits,
                complete_budget_report,
                diagnostics,
            } => {
                check(&expected_continuation, expected_maximum, expected_limits);
                Err(DiscoveryBatchFailure::BudgetExhausted {
                    complete_budget_report,
                    diagnostics,
                })
            }
            ScriptedDiscoveryOutcome::Cancelled {
                expected_continuation,
                expected_maximum,
                expected_limits,
                complete_budget_report,
                diagnostics,
            } => {
                check(&expected_continuation, expected_maximum, expected_limits);
                Err(DiscoveryBatchFailure::Cancelled {
                    complete_budget_report,
                    diagnostics,
                })
            }
            ScriptedDiscoveryOutcome::ExecutionFailed {
                expected_continuation,
                expected_maximum,
                expected_limits,
                complete_budget_report,
                diagnostics,
            } => {
                check(&expected_continuation, expected_maximum, expected_limits);
                Err(DiscoveryBatchFailure::Execution {
                    complete_budget_report,
                    diagnostics,
                })
            }
        }
    }
}

/// Truthful one-shot adapter over the current Profile-DSL Discovery phase. The phase is tightened
/// by the supplied maximum. It accepts only a complete materialized vector already within that
/// bound and never slices it or invents continuation.
async fn execute_profile_dsl_batch(
    fetcher: &(dyn ProfileHttpClient + Sync),
    acquisition: &dyn BrowserAcquisition,
    request: DiscoveryBatchRequest<'_>,
) -> DiscoveryBatchResult {
    if request.continuation.is_some() {
        return Err(DiscoveryBatchFailure::UnboundedMaterializedOutput);
    }
    let plan = &request.compiled_source.execution_plan;
    // The landed phase envelope has no natural-pagination exhaustion fact. A
    // paginated accepted vector therefore cannot be truthfully projected as an
    // exhausted batch, even when it happens to fit. Refuse it rather than slice,
    // continue, or invent exhaustion.
    if plan
        .discovery
        .strategies
        .iter()
        .any(|strategy| strategy.pagination.is_some())
    {
        return Err(DiscoveryBatchFailure::UnboundedMaterializedOutput);
    }
    let browser = if plan.discovery.strategies.iter().any(|strategy| {
        matches!(
            strategy.fetch,
            crate::profile_dsl::execution_plan::capabilities::ExecutionPlanFetch::Browser { .. }
        )
    }) {
        PhaseBrowser::Browser(DiscoveryBrowserAdapter::new(acquisition))
    } else {
        PhaseBrowser::BrowserFree
    };
    let result = execute_discovery(
        plan,
        &request.compiled_source.source_config,
        fetcher,
        browser,
        request.context,
    )
    .await;
    match result {
        Err(PhaseRunError::Cancelled(cancelled)) => Err(DiscoveryBatchFailure::Cancelled {
            complete_budget_report: cancelled.complete_budget_report,
            diagnostics: cancelled.diagnostics,
        }),
        Err(PhaseRunError::NotStarted { .. }) => Err(DiscoveryBatchFailure::NotStarted),
        Ok(PhaseOutcome::Completed {
            policy_outcome: PolicyOutcome::Accepted { reduced_payload },
            complete_budget_report,
            diagnostics,
        }) => {
            if u64::try_from(reduced_payload.candidates.len())
                .ok()
                .is_none_or(|len| len > request.maximum)
            {
                return Err(DiscoveryBatchFailure::Execution {
                    complete_budget_report,
                    diagnostics,
                });
            }
            Ok(DiscoveryBatch {
                occurrences: reduced_payload.candidates,
                exhausted: true,
                remaining: Some(0),
                complete_budget_report,
                diagnostics,
                continuation: None,
            })
        }
        Ok(PhaseOutcome::BudgetExhausted {
            complete_budget_report,
            diagnostics,
        }) => Err(DiscoveryBatchFailure::BudgetExhausted {
            complete_budget_report,
            diagnostics,
        }),
        Ok(PhaseOutcome::ExecutionFailed {
            complete_budget_report,
            diagnostics,
            ..
        })
        | Ok(PhaseOutcome::Completed {
            complete_budget_report,
            diagnostics,
            policy_outcome: PolicyOutcome::PolicyUnsatisfied { .. },
        }) => Err(DiscoveryBatchFailure::Execution {
            complete_budget_report,
            diagnostics,
        }),
    }
}

pub async fn resolve_source_candidates(
    request: SourceResolutionRequest<'_>,
) -> Result<SourceResolution, SourceResolutionError> {
    let ceilings = request
        .ceilings
        .validate()
        .map_err(|failure| failed(failure, Vec::new()))?;
    if request.requirements.geo_runtime_failure {
        return Err(geo_resolution_failed(String::new()));
    }
    let source_key = request.compiled_source.execution_plan.source.key.clone();
    let mut state = ResolutionState::new(source_key.clone(), ceilings.phase, request.requirements);
    let mut continuation: Option<DiscoveryContinuation> = None;
    let mut used_tokens = HashSet::new();
    let mut identities = HashSet::new();
    let mut batches = 0u64;
    let mut remaining_exact = true;

    loop {
        cancelled(request.cancellation)?;
        let stop = if batches == ceilings.max_discovery_batches {
            Some(ResolutionLimitDimension::DiscoveryBatches)
        } else if state.counts.discovered == ceilings.max_discovered_items {
            Some(ResolutionLimitDimension::DiscoveredItems)
        } else {
            state.parent.first_exhausted()
        };
        if let Some(dimension) = stop {
            state.partial = Some(dimension);
            return state.finish(request.cancellation);
        }

        let maximum = ceilings
            .max_batch_size
            .min(ceilings.max_discovered_items - state.counts.discovered);
        let mut child_limits = match state.parent.remaining_limits()? {
            ParentAdmission::Admitted(limits) => limits,
            ParentAdmission::Exhausted(dimension) => {
                state.partial = Some(dimension);
                return state.finish(request.cancellation);
            }
        };
        child_limits.max_produced_items = child_limits.max_produced_items.min(maximum);
        let context = RuntimeExecutionContext::with_cancellation(request.cancellation)
            .with_limits(child_limits);
        let outcome = request
            .discovery
            .execute_batch(DiscoveryBatchRequest {
                compiled_source: request.compiled_source,
                maximum,
                limits: child_limits,
                context,
                continuation: continuation.as_ref(),
            })
            .await;

        let batch = match outcome {
            Ok(batch) => batch,
            Err(DiscoveryBatchFailure::BudgetExhausted {
                complete_budget_report,
                diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, child_limits, |completion| {
                    matches!(completion, PhaseCompletion::BudgetExhausted { .. })
                })?;
                state.parent.commit(&complete_budget_report)?;
                state.diagnostics.extend(diagnostics);
                state.partial = dimension_from_completion(&complete_budget_report.completion)
                    .or(Some(ResolutionLimitDimension::Requests));
                return state.finish(request.cancellation);
            }
            Err(DiscoveryBatchFailure::Cancelled {
                complete_budget_report,
                diagnostics: _diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, child_limits, |completion| {
                    matches!(completion, PhaseCompletion::Cancelled { .. })
                })?;
                state.parent.commit(&complete_budget_report)?;
                return Err(SourceResolutionError::Cancelled);
            }
            Err(DiscoveryBatchFailure::Execution {
                complete_budget_report,
                diagnostics,
            }) => {
                validate_child_report(&complete_budget_report, child_limits, |completion| {
                    matches!(
                        completion,
                        PhaseCompletion::Accepted
                            | PhaseCompletion::ExecutionFailed
                            | PhaseCompletion::PolicyUnsatisfied
                    )
                })?;
                state.parent.commit(&complete_budget_report)?;
                return Err(failed(ResolutionFailure::DiscoveryExecution, diagnostics));
            }
            Err(DiscoveryBatchFailure::NotStarted)
            | Err(DiscoveryBatchFailure::UnboundedMaterializedOutput) => {
                return Err(failed(ResolutionFailure::DiscoveryExecution, Vec::new()));
            }
        };

        validate_child_report(&batch.complete_budget_report, child_limits, |completion| {
            matches!(completion, PhaseCompletion::Accepted)
        })?;
        state.parent.commit(&batch.complete_budget_report)?;
        batches = checked_add(batches, 1)?;
        state.diagnostics.extend(batch.diagnostics);

        let batch_len = u64::try_from(batch.occurrences.len())
            .map_err(|_| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?;
        if batch_len > maximum
            || (!batch.exhausted && batch_len == 0)
            || (batch.exhausted != batch.continuation.is_none())
        {
            return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
        }
        if let Some(token) = &batch.continuation {
            if token.source_key != source_key
                || continuation.as_ref().is_some_and(|old| old == token)
                || !used_tokens.insert(token.clone())
            {
                return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
            }
        }
        for occurrence in &batch.occurrences {
            if occurrence.identity.source_key() != source_key
                || !identities.insert(occurrence.identity.clone())
            {
                return Err(failed(ResolutionFailure::ProtocolInvariant, Vec::new()));
            }
        }
        if remaining_exact {
            if let Some(current) = batch.remaining {
                let consistent_with_exhaustion =
                    (batch.exhausted && current == 0) || (!batch.exhausted && current > 0);
                let consistent = consistent_with_exhaustion
                    && state
                        .remaining
                        .map(|previous| previous.checked_sub(batch_len) == Some(current))
                        .unwrap_or(true);
                if consistent {
                    state.remaining = Some(current);
                } else {
                    state.remaining = None;
                    remaining_exact = false;
                    state.diagnostics.push(sanitized_diagnostic(
                        "discovery_remaining_inconsistent",
                        "Discovery remaining became unavailable",
                        DiagnosticSeverity::Warning,
                    ));
                }
            } else {
                state.remaining = None;
                remaining_exact = false;
            }
        }

        state
            .process_occurrences(&request, &batch.occurrences)
            .await?;
        if state.partial.is_some() {
            return state.finish(request.cancellation);
        }
        continuation = batch.continuation;
        if batch.exhausted {
            return state.finish(request.cancellation);
        }
    }
}

struct ResolutionState {
    source_key: String,
    finalized: Vec<FinalizedCandidate>,
    counts: ResolutionCounts,
    remaining: Option<u64>,
    parent: ParentAllowance,
    diagnostics: Diagnostics,
    sampler: DiagnosticSampler,
    detail_candidates: u64,
    partial: Option<ResolutionLimitDimension>,
    location_diagnostics: LocationDiagnosticSummary,
}

impl ResolutionState {
    fn new(
        source_key: String,
        limits: PhaseLimits,
        requirements: &CompiledSearchRequirements<'_>,
    ) -> Self {
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
            remaining: None,
            parent: ParentAllowance::new(limits),
            diagnostics,
            sampler: DiagnosticSampler::new(CANDIDATE_DIAGNOSTIC_SAMPLE_LIMIT),
            detail_candidates: 0,
            partial: None,
            location_diagnostics,
        }
    }

    async fn process_occurrences(
        &mut self,
        request: &SourceResolutionRequest<'_>,
        occurrences: &[PostingOccurrence],
    ) -> Result<(), SourceResolutionError> {
        self.counts.discovered = checked_add(
            self.counts.discovered,
            u64::try_from(occurrences.len())
                .map_err(|_| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))?,
        )?;
        for (index, occurrence) in occurrences.iter().enumerate() {
            cancelled(request.cancellation)?;
            if hint_rejects(occurrence, request.requirements) {
                self.counts.rejected = checked_add(self.counts.rejected, 1)?;
                continue;
            }
            let mut values = CandidateValues::from_occurrence(occurrence);
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
            let stop = if self.detail_candidates == request.ceilings.max_detail_candidates {
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
                ParentAdmission::Admitted(limits) => limits,
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
                    .map_or_else(|error| error, |()| SourceResolutionError::Cancelled)
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
                    if dispositions.iter().any(|d| matches!(d, crate::profile_dsl::runtime::RequestedFieldDisposition::Unavailable { .. } | crate::profile_dsl::runtime::RequestedFieldDisposition::Conflicted { .. } | crate::profile_dsl::runtime::RequestedFieldDisposition::Unsupported { .. }))
                        || !values.apply(fields, &needed)
                        || !values.is_complete(request.requirements)
                    {
                        self.counts.unresolved = checked_add(self.counts.unresolved, 1)?;
                    } else {
                        let matches = values
                            .final_matches(request.requirements)
                            .await
                            .map_err(geo_resolution_failed)?;
                        self.location_diagnostics.observe_match_report(matches.1.as_ref());
                        if matches.0 {
                            self.finalized.push(values.finalize(&self.source_key, occurrence)?);
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
    ) -> Result<SourceResolution, SourceResolutionError> {
        // Final commit boundary: cancellation releases no counts, completion, or finalized values.
        cancelled(cancellation)?;
        self.diagnostics
            .extend(self.location_diagnostics.into_diagnostics());
        cancelled_counts(&mut self.counts)?;
        validate_counts(&self.counts, self.finalized.len())?;
        Ok(SourceResolution {
            source_key: self.source_key,
            finalized: self.finalized,
            completion: self
                .partial
                .map(|limit_reached| ResolutionCompletion::Partial { limit_reached })
                .unwrap_or(ResolutionCompletion::Complete),
            counts: self.counts,
            remaining: self.remaining,
            report: ResolutionReport {
                usage: self.parent.usage,
            },
            diagnostics: self.diagnostics,
            candidate_diagnostics: self.sampler.finish(),
        })
    }
}

fn cancelled_counts(counts: &mut ResolutionCounts) -> Result<(), SourceResolutionError> {
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
) -> Result<(), SourceResolutionError> {
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
            typed_failure: crate::profile_dsl::runtime::SourceDetailFailure::PhaseExecution { .. },
            complete_budget_report: Some(report),
            ..
        } => matches!(report.completion, PhaseCompletion::ExecutionFailed),
        SourceDetailOutcome::SourceExecutionFailed {
            typed_failure: crate::profile_dsl::runtime::SourceDetailFailure::PhasePreStart { .. },
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
    dispositions: &[crate::profile_dsl::runtime::RequestedFieldDisposition],
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
    fn needed(&self, requirements: &CompiledSearchRequirements<'_>) -> Vec<DetailField> {
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
    fn is_complete(&self, requirements: &CompiledSearchRequirements<'_>) -> bool {
        self.title.is_some()
            && self.company.is_some()
            && self.url.is_some()
            && (!requirements.requires_locations() || !self.locations.is_empty())
    }
    async fn final_matches(
        &self,
        requirements: &CompiledSearchRequirements<'_>,
    ) -> Result<(bool, Option<LocationFilterMatchReport>), String> {
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
    ) -> Result<FinalizedCandidate, SourceResolutionError> {
        Ok(FinalizedCandidate {
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
fn hint_rejects(o: &PostingOccurrence, requirements: &CompiledSearchRequirements<'_>) -> bool {
    o.hints
        .get("title")
        .filter(|h| h.hint_use == Some(crate::profile_dsl::runtime::HintUse::SearchPrefilter))
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
    fn remaining_limits(&self) -> Result<ParentAdmission, SourceResolutionError> {
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
    fn commit(&mut self, report: &PhaseExecutionReport) -> Result<(), SourceResolutionError> {
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
    fn observe(&mut self, code: &'static str) -> Result<(), SourceResolutionError> {
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
fn geo_resolution_failed(_error: String) -> SourceResolutionError {
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
fn cancelled(c: &dyn RuntimeCancellation) -> Result<(), SourceResolutionError> {
    if c.is_cancelled() {
        Err(SourceResolutionError::Cancelled)
    } else {
        Ok(())
    }
}
fn failed(failure: ResolutionFailure, diagnostics: Diagnostics) -> SourceResolutionError {
    SourceResolutionError::Failed {
        failure,
        diagnostics,
    }
}
fn checked_add(a: u64, b: u64) -> Result<u64, SourceResolutionError> {
    a.checked_add(b)
        .ok_or_else(|| failed(ResolutionFailure::ArithmeticInvariant, Vec::new()))
}
fn validate_counts(
    c: &ResolutionCounts,
    finalized_len: usize,
) -> Result<(), SourceResolutionError> {
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
    use crate::profile_dsl::runtime::AllowanceDimension::*;
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
