use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::profile_dsl::{
    diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticSeverity},
    documents::PhaseLimits,
    execution_plan::capabilities::ExecutionPlanFetch,
};

pub(crate) const BROWSER_GRACEFUL_CLOSE_MS: u64 = 500;
pub(crate) const BROWSER_FORCE_TERMINATE_REAP_MS: u64 = 1_000;
pub(crate) const BROWSER_HANDLER_COMPLETION_MS: u64 = 250;
pub(crate) const BROWSER_SESSION_FINALIZATION_MS: u64 = 250;
pub(crate) const BROWSER_TEARDOWN_RESERVE_MS: u64 = BROWSER_GRACEFUL_CLOSE_MS
    + BROWSER_FORCE_TERMINATE_REAP_MS
    + BROWSER_HANDLER_COMPLETION_MS
    + BROWSER_SESSION_FINALIZATION_MS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowanceDimension {
    StrategyAttempts,
    Requests,
    ProducedItems,
    Duration,
    Pages,
    BrowserActions,
    FanOut,
    ResponseBytes,
    BrowserRenderedBytes,
    LogicalWaits,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AllowanceLimitSource {
    Backend,
    Compiled,
    Caller,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowanceExhaustion {
    pub dimension: AllowanceDimension,
    pub requested: u64,
    pub remaining: u64,
    pub limit_sources: Vec<AllowanceLimitSource>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseCancellationReason {
    UserCancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PhaseCompletion {
    Accepted,
    PolicyUnsatisfied,
    BudgetExhausted { exhaustion: AllowanceExhaustion },
    ExecutionFailed,
    Cancelled { reason: PhaseCancellationReason },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseUsage {
    pub strategy_attempts: u64,
    pub requests: u64,
    pub produced_items: u64,
    pub duration_ms: u64,
    pub pages: u64,
    pub browser_actions: u64,
    pub fan_out: u64,
    pub response_bytes: u64,
    pub browser_rendered_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseExecutionReport {
    pub usage: PhaseUsage,
    pub completion: PhaseCompletion,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AllowanceCharge {
    pub(crate) strategy_attempts: u64,
    pub(crate) requests: u64,
    pub(crate) produced_items: u64,
    pub(crate) pages: u64,
    pub(crate) browser_actions: u64,
    pub(crate) fan_out: u64,
    pub(crate) response_bytes: u64,
    pub(crate) logical_waits: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum AllowanceStop {
    Exhausted(AllowanceExhaustion),
    Internal,
}

#[derive(Clone, Copy)]
struct EffectiveLimits {
    values: PhaseLimits,
    compiled: PhaseLimits,
    compiled_authored: bool,
    caller: Option<PhaseLimits>,
}

#[derive(Clone, Copy, Default)]
struct LedgerState {
    usage: PhaseUsage,
}

struct AllowanceScopeState {
    parent: Option<usize>,
    limits: EffectiveLimits,
    started_at: Option<Instant>,
    deadline: Option<Instant>,
    state: LedgerState,
    logical_waits: u64,
    max_logical_waits: Option<u64>,
}

/// One private invocation-owned allowance root. Child scopes are views over this
/// root: every debit locks the root once, checks the complete ancestor chain,
/// and commits all affected scope usage atomically.
pub(crate) struct InvocationAllowance {
    scopes: Mutex<Vec<AllowanceScopeState>>,
    stop: Mutex<Option<AllowanceStop>>,
}

impl InvocationAllowance {
    pub(crate) const ROOT_SCOPE: usize = 0;

    pub(crate) fn new(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
    ) -> Self {
        Self::new_with_logical_wait_limit(compiled, compiled_authored, caller, None)
    }

    pub(crate) fn new_with_logical_wait_limit(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
        max_logical_waits: Option<u64>,
    ) -> Self {
        Self::new_scope_root(compiled, compiled_authored, caller, max_logical_waits, true)
    }

    pub(crate) fn new_inactive_with_logical_wait_limit(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
        max_logical_waits: Option<u64>,
    ) -> Self {
        Self::new_scope_root(
            compiled,
            compiled_authored,
            caller,
            max_logical_waits,
            false,
        )
    }

    fn new_scope_root(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
        max_logical_waits: Option<u64>,
        active: bool,
    ) -> Self {
        let values = caller.map_or(compiled, |caller| compiled.minimum(caller));
        let started_at = active.then(Instant::now);
        Self {
            scopes: Mutex::new(vec![AllowanceScopeState {
                parent: None,
                limits: EffectiveLimits {
                    values,
                    compiled,
                    compiled_authored,
                    caller,
                },
                started_at,
                deadline: started_at.map(|started| {
                    started + std::time::Duration::from_millis(values.max_duration_ms)
                }),
                state: LedgerState::default(),
                logical_waits: 0,
                max_logical_waits,
            }]),
            stop: Mutex::new(None),
        }
    }

    pub(crate) fn child_scope(
        &self,
        parent: usize,
        limits: PhaseLimits,
        max_logical_waits: Option<u64>,
    ) -> Result<usize, AllowanceStop> {
        if self.stop().is_some() {
            return Err(self.stop().expect("stop was observed"));
        }
        let started_at = Instant::now();
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        if parent >= scopes.len() {
            drop(scopes);
            return Err(self.fail_internal());
        }
        let id = scopes.len();
        scopes.push(AllowanceScopeState {
            parent: Some(parent),
            limits: EffectiveLimits {
                values: limits,
                compiled: limits,
                compiled_authored: false,
                caller: None,
            },
            started_at: Some(started_at),
            deadline: Some(started_at + std::time::Duration::from_millis(limits.max_duration_ms)),
            state: LedgerState::default(),
            logical_waits: 0,
            max_logical_waits,
        });
        Ok(id)
    }

    pub(crate) fn inactive_child_scope(
        &self,
        parent: usize,
        limits: PhaseLimits,
        max_logical_waits: Option<u64>,
    ) -> Result<usize, AllowanceStop> {
        if let Some(stop) = self.stop() {
            return Err(stop);
        }
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        if parent >= scopes.len() {
            drop(scopes);
            return Err(self.fail_internal());
        }
        let id = scopes.len();
        scopes.push(AllowanceScopeState {
            parent: Some(parent),
            limits: EffectiveLimits {
                values: limits,
                compiled: limits,
                compiled_authored: false,
                caller: None,
            },
            started_at: None,
            deadline: None,
            state: LedgerState::default(),
            logical_waits: 0,
            max_logical_waits,
        });
        Ok(id)
    }

    /// Activates an inactive scope and every inactive ancestor in one root lock.
    /// Detection uses this immediately before its first Browser effect so prior
    /// URL/HTTP work cannot consume Browser-only duration.
    pub(crate) fn activate_scope_chain(&self, scope: usize) -> Result<(), AllowanceStop> {
        if let Some(stop) = self.stop() {
            return Err(stop);
        }
        let now = Instant::now();
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let chain = scope_chain(&scopes, scope);
        if chain.is_empty() {
            drop(scopes);
            return Err(self.fail_internal());
        }
        for id in chain.into_iter().rev() {
            let current = &mut scopes[id];
            if current.started_at.is_none() {
                current.started_at = Some(now);
                current.deadline = Some(
                    now + std::time::Duration::from_millis(current.limits.values.max_duration_ms),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn effective_limits(&self) -> PhaseLimits {
        self.effective_limits_for(Self::ROOT_SCOPE)
    }

    pub(crate) fn effective_limits_for(&self, scope: usize) -> PhaseLimits {
        let scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let mut chain = scope_chain(&scopes, scope);
        let first = chain.pop().unwrap_or(Self::ROOT_SCOPE);
        chain
            .into_iter()
            .fold(scopes[first].limits.values, |limits, id| {
                limits.minimum(scopes[id].limits.values)
            })
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline_for(Self::ROOT_SCOPE)
    }

    pub(crate) fn deadline_for(&self, scope: usize) -> Instant {
        let scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        scope_chain(&scopes, scope)
            .into_iter()
            .filter_map(|id| scopes[id].deadline)
            .min()
            .expect("Browser allowance scope chain is active before acquisition")
    }

    pub(crate) fn browser_work_deadline_for(&self, scope: usize) -> Instant {
        self.deadline_for(scope)
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_TEARDOWN_RESERVE_MS,
            ))
            .expect("browser-capable scope duration is validated")
    }

    pub(crate) fn browser_graceful_deadline_for(&self, scope: usize) -> Instant {
        self.deadline_for(scope)
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_FORCE_TERMINATE_REAP_MS
                    + BROWSER_HANDLER_COMPLETION_MS
                    + BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable scope duration is validated")
    }

    pub(crate) fn browser_force_deadline_for(&self, scope: usize) -> Instant {
        self.deadline_for(scope)
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_HANDLER_COMPLETION_MS + BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable scope duration is validated")
    }

    pub(crate) fn browser_handler_deadline_for(&self, scope: usize) -> Instant {
        self.deadline_for(scope)
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable scope duration is validated")
    }

    pub(crate) fn browser_work_deadline(&self) -> Instant {
        self.browser_work_deadline_for(Self::ROOT_SCOPE)
    }
    pub(crate) fn browser_graceful_deadline(&self) -> Instant {
        self.browser_graceful_deadline_for(Self::ROOT_SCOPE)
    }
    pub(crate) fn browser_force_deadline(&self) -> Instant {
        self.browser_force_deadline_for(Self::ROOT_SCOPE)
    }
    pub(crate) fn browser_handler_deadline(&self) -> Instant {
        self.browser_handler_deadline_for(Self::ROOT_SCOPE)
    }

    pub(crate) fn debit(&self, charge: AllowanceCharge) -> Result<(), AllowanceStop> {
        self.debit_for(Self::ROOT_SCOPE, charge, None)
    }

    pub(crate) fn debit_with_pagination_limit(
        &self,
        charge: AllowanceCharge,
        pagination_max_requests: Option<u64>,
    ) -> Result<(), AllowanceStop> {
        self.debit_for(Self::ROOT_SCOPE, charge, pagination_max_requests)
    }

    pub(crate) fn debit_for(
        &self,
        scope: usize,
        charge: AllowanceCharge,
        pagination_max_requests: Option<u64>,
    ) -> Result<(), AllowanceStop> {
        if let Some(stop) = self.stop() {
            return Err(stop);
        }
        let now = Instant::now();
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let chain = scope_chain(&scopes, scope);
        if chain.is_empty() {
            drop(scopes);
            return Err(self.fail_internal());
        }
        for id in &chain {
            let current = &mut scopes[*id];
            current.state.usage.duration_ms = current.started_at.map_or(0, elapsed_ms);
            let limit_requests = pagination_max_requests
                .map_or(current.limits.values.max_requests, |limit| {
                    current.limits.values.max_requests.min(limit)
                });
            let limit_pages = pagination_max_requests
                .map_or(current.limits.values.max_pages, |limit| {
                    current.limits.values.max_pages.min(limit)
                });
            let checks = [
                (
                    AllowanceDimension::StrategyAttempts,
                    current.state.usage.strategy_attempts,
                    charge.strategy_attempts,
                    current.limits.values.max_strategy_attempts,
                ),
                (
                    AllowanceDimension::Requests,
                    current.state.usage.requests,
                    charge.requests,
                    limit_requests,
                ),
                (
                    AllowanceDimension::ProducedItems,
                    current.state.usage.produced_items,
                    charge.produced_items,
                    current.limits.values.max_produced_items,
                ),
                (
                    AllowanceDimension::ResponseBytes,
                    current.state.usage.response_bytes,
                    charge.response_bytes,
                    current.limits.values.max_response_bytes,
                ),
            ];
            for (dimension, used, requested, limit) in checks {
                let Some(next) = used.checked_add(requested) else {
                    drop(scopes);
                    return Err(self.fail_internal());
                };
                if next > limit {
                    let sources = debit_limit_sources(
                        current.limits,
                        dimension,
                        pagination_max_requests,
                        limit,
                    );
                    drop(scopes);
                    return Err(self.exhaust_with_sources(
                        dimension,
                        requested,
                        limit.saturating_sub(used),
                        sources,
                    ));
                }
            }
            if current.deadline.is_some_and(|deadline| now > deadline) {
                let sources = limit_sources(current.limits, AllowanceDimension::Duration);
                drop(scopes);
                return Err(self.exhaust_with_sources(AllowanceDimension::Duration, 1, 0, sources));
            }
            let after = [
                (
                    AllowanceDimension::Pages,
                    current.state.usage.pages,
                    charge.pages,
                    limit_pages,
                ),
                (
                    AllowanceDimension::BrowserActions,
                    current.state.usage.browser_actions,
                    charge.browser_actions,
                    current.limits.values.max_browser_actions,
                ),
                (
                    AllowanceDimension::FanOut,
                    current.state.usage.fan_out,
                    charge.fan_out,
                    current.limits.values.max_fan_out,
                ),
            ];
            for (dimension, used, requested, limit) in after {
                let Some(next) = used.checked_add(requested) else {
                    drop(scopes);
                    return Err(self.fail_internal());
                };
                if next > limit {
                    let sources = debit_limit_sources(
                        current.limits,
                        dimension,
                        pagination_max_requests,
                        limit,
                    );
                    drop(scopes);
                    return Err(self.exhaust_with_sources(
                        dimension,
                        requested,
                        limit.saturating_sub(used),
                        sources,
                    ));
                }
            }
            let Some(next_waits) = current.logical_waits.checked_add(charge.logical_waits) else {
                drop(scopes);
                return Err(self.fail_internal());
            };
            if current
                .max_logical_waits
                .is_some_and(|wait_limit| next_waits > wait_limit)
            {
                let wait_limit = current.max_logical_waits.expect("checked as present");
                let remaining = wait_limit.saturating_sub(current.logical_waits);
                drop(scopes);
                return Err(self.exhaust_with_sources(
                    AllowanceDimension::LogicalWaits,
                    charge.logical_waits,
                    remaining,
                    vec![AllowanceLimitSource::Backend],
                ));
            }
        }
        for id in chain {
            let current = &mut scopes[id];
            current.state.usage.strategy_attempts += charge.strategy_attempts;
            current.state.usage.requests += charge.requests;
            current.state.usage.produced_items += charge.produced_items;
            current.state.usage.pages += charge.pages;
            current.state.usage.browser_actions += charge.browser_actions;
            current.state.usage.fan_out += charge.fan_out;
            current.state.usage.response_bytes += charge.response_bytes;
            current.logical_waits += charge.logical_waits;
        }
        Ok(())
    }

    pub(crate) fn admit_browser_rendered_bytes(&self, observed: u64) -> Result<(), AllowanceStop> {
        self.admit_browser_rendered_bytes_for(Self::ROOT_SCOPE, observed)
    }

    pub(crate) fn admit_browser_rendered_bytes_for(
        &self,
        scope: usize,
        observed: u64,
    ) -> Result<(), AllowanceStop> {
        if let Some(stop) = self.stop() {
            return Err(stop);
        }
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let chain = scope_chain(&scopes, scope);
        if chain.is_empty() {
            drop(scopes);
            return Err(self.fail_internal());
        }
        let mut tightest_remaining = u64::MAX;
        let mut tightest_sources = Vec::new();
        for id in &chain {
            let current = &scopes[*id];
            let remaining = current
                .limits
                .values
                .max_browser_rendered_bytes
                .saturating_sub(current.state.usage.browser_rendered_bytes);
            if remaining < tightest_remaining {
                tightest_remaining = remaining;
                tightest_sources =
                    limit_sources(current.limits, AllowanceDimension::BrowserRenderedBytes);
            }
            if current
                .state
                .usage
                .browser_rendered_bytes
                .checked_add(observed)
                .is_none()
            {
                drop(scopes);
                return Err(self.fail_internal());
            }
        }
        if observed <= tightest_remaining {
            for id in chain {
                scopes[id].state.usage.browser_rendered_bytes += observed;
            }
            return Ok(());
        }
        // Observed content cannot be exposed. Consume every applicable remaining
        // scope so no child view can accidentally reuse capacity after the stop.
        for id in chain {
            let current = &mut scopes[id];
            current.state.usage.browser_rendered_bytes =
                current.limits.values.max_browser_rendered_bytes;
        }
        drop(scopes);
        Err(self.exhaust_with_sources(
            AllowanceDimension::BrowserRenderedBytes,
            observed.saturating_sub(tightest_remaining),
            0,
            tightest_sources,
        ))
    }

    pub(crate) fn remaining_browser_rendered_bytes(&self) -> u64 {
        self.remaining_browser_rendered_bytes_for(Self::ROOT_SCOPE)
    }

    pub(crate) fn remaining_browser_rendered_bytes_for(&self, scope: usize) -> u64 {
        let scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        scope_chain(&scopes, scope)
            .into_iter()
            .map(|id| {
                scopes[id]
                    .limits
                    .values
                    .max_browser_rendered_bytes
                    .saturating_sub(scopes[id].state.usage.browser_rendered_bytes)
            })
            .min()
            .unwrap_or(0)
    }

    pub(crate) fn remaining_response_bytes(&self) -> u64 {
        let scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        scopes[Self::ROOT_SCOPE]
            .limits
            .values
            .max_response_bytes
            .saturating_sub(scopes[Self::ROOT_SCOPE].state.usage.response_bytes)
    }

    pub(crate) fn commit_response_bytes(&self, admitted: u64, exceeded: Option<u64>) {
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let root = &mut scopes[Self::ROOT_SCOPE];
        let remaining = match root.state.usage.response_bytes.checked_add(admitted) {
            Some(next) if next <= root.limits.values.max_response_bytes => {
                root.state.usage.response_bytes = next;
                root.limits.values.max_response_bytes - next
            }
            _ => {
                drop(scopes);
                let _ = self.fail_internal();
                return;
            }
        };
        drop(scopes);
        if let Some(requested) = exceeded {
            let _ = self.exhaust_root(AllowanceDimension::ResponseBytes, requested, remaining);
        }
    }

    pub(crate) fn mark_deadline(&self) {
        let _ = self.exhaust_root(AllowanceDimension::Duration, 1, 0);
    }
    pub(crate) fn mark_deadline_if_expired(&self) {
        let deadline =
            self.scopes.lock().unwrap_or_else(|p| p.into_inner())[Self::ROOT_SCOPE].deadline;
        if deadline.is_some_and(|deadline| Instant::now() > deadline) {
            self.mark_deadline();
        }
    }
    pub(crate) fn stop(&self) -> Option<AllowanceStop> {
        self.stop.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub(crate) fn prestart_failure_report() -> PhaseExecutionReport {
        PhaseExecutionReport {
            usage: PhaseUsage::default(),
            completion: PhaseCompletion::ExecutionFailed,
        }
    }
    pub(crate) fn report(&self, completion: PhaseCompletion) -> PhaseExecutionReport {
        let mut scopes = self.scopes.lock().unwrap_or_else(|p| p.into_inner());
        let root = &mut scopes[Self::ROOT_SCOPE];
        root.state.usage.duration_ms = root.started_at.map_or(0, elapsed_ms);
        PhaseExecutionReport {
            usage: root.state.usage,
            completion,
        }
    }
    pub(crate) fn mark_internal_failure(&self) {
        let _ = self.fail_internal();
    }
    fn fail_internal(&self) -> AllowanceStop {
        let stop = AllowanceStop::Internal;
        let mut current = self.stop.lock().unwrap_or_else(|p| p.into_inner());
        if current.is_none() {
            *current = Some(stop.clone());
        }
        current.clone().expect("stop was set")
    }
    fn exhaust_root(
        &self,
        dimension: AllowanceDimension,
        requested: u64,
        remaining: u64,
    ) -> AllowanceStop {
        let limits = self.scopes.lock().unwrap_or_else(|p| p.into_inner())[Self::ROOT_SCOPE].limits;
        self.exhaust_with_sources(
            dimension,
            requested,
            remaining,
            limit_sources(limits, dimension),
        )
    }
    fn exhaust_with_sources(
        &self,
        dimension: AllowanceDimension,
        requested: u64,
        remaining: u64,
        mut sources: Vec<AllowanceLimitSource>,
    ) -> AllowanceStop {
        if sources.is_empty() {
            sources.push(AllowanceLimitSource::Backend);
        }
        let stop = AllowanceStop::Exhausted(AllowanceExhaustion {
            dimension,
            requested,
            remaining,
            limit_sources: sources,
        });
        let mut current = self.stop.lock().unwrap_or_else(|p| p.into_inner());
        if current.is_none() {
            *current = Some(stop.clone());
        }
        current.clone().expect("stop was set")
    }
}

fn scope_chain(scopes: &[AllowanceScopeState], mut scope: usize) -> Vec<usize> {
    if scope >= scopes.len() {
        return Vec::new();
    }
    let mut chain = Vec::new();
    loop {
        chain.push(scope);
        match scopes[scope].parent {
            Some(parent) => scope = parent,
            None => break,
        }
    }
    chain
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn debit_limit_sources(
    limits: EffectiveLimits,
    dimension: AllowanceDimension,
    pagination_max_requests: Option<u64>,
    effective_limit: u64,
) -> Vec<AllowanceLimitSource> {
    let mut sources = limit_sources(limits, dimension);
    let pagination_applies = matches!(
        dimension,
        AllowanceDimension::Requests | AllowanceDimension::Pages
    ) && pagination_max_requests
        .is_some_and(|limit| limit == effective_limit);
    if pagination_applies && !sources.contains(&AllowanceLimitSource::Compiled) {
        let index = usize::from(sources.first() == Some(&AllowanceLimitSource::Backend));
        sources.insert(index, AllowanceLimitSource::Compiled);
    }
    sources
}

fn limit_sources(
    limits: EffectiveLimits,
    dimension: AllowanceDimension,
) -> Vec<AllowanceLimitSource> {
    let value = dimension_value(limits.values, dimension);
    let mut sources = Vec::new();
    if dimension_value(PhaseLimits::BACKEND, dimension) == value {
        sources.push(AllowanceLimitSource::Backend);
    }
    if limits.compiled_authored && dimension_value(limits.compiled, dimension) == value {
        sources.push(AllowanceLimitSource::Compiled);
    }
    if limits
        .caller
        .is_some_and(|caller| dimension_value(caller, dimension) == value)
    {
        sources.push(AllowanceLimitSource::Caller);
    }
    sources
}

pub(crate) fn uses_browser(fetch: &ExecutionPlanFetch) -> bool {
    matches!(fetch, ExecutionPlanFetch::Browser { .. })
}

pub(crate) fn completion_for_stop(stop: AllowanceStop) -> PhaseCompletion {
    match stop {
        AllowanceStop::Exhausted(exhaustion) => PhaseCompletion::BudgetExhausted { exhaustion },
        AllowanceStop::Internal => PhaseCompletion::ExecutionFailed,
    }
}

pub(crate) fn diagnostic_for_stop(stop: &AllowanceStop, path: &str) -> Diagnostic {
    let (code, message, details) = match stop {
        AllowanceStop::Exhausted(exhaustion) => (
            "phase_allowance_exhausted",
            "Cumulative phase allowance exhausted",
            serde_json::to_value(exhaustion).unwrap_or_else(|_| serde_json::json!({})),
        ),
        AllowanceStop::Internal => (
            "phase_allowance_internal_failure",
            "Cumulative phase allowance accounting failed",
            serde_json::json!({}),
        ),
    };
    Diagnostic {
        category: DiagnosticCategory::Runtime,
        code: code.to_string(),
        message: message.to_string(),
        severity: DiagnosticSeverity::Error,
        path: path.to_string(),
        strategy_key: None,
        details: Some(details),
    }
}

fn dimension_value(limits: PhaseLimits, dimension: AllowanceDimension) -> u64 {
    match dimension {
        AllowanceDimension::StrategyAttempts => limits.max_strategy_attempts,
        AllowanceDimension::Requests => limits.max_requests,
        AllowanceDimension::ProducedItems => limits.max_produced_items,
        AllowanceDimension::Duration => limits.max_duration_ms,
        AllowanceDimension::Pages => limits.max_pages,
        AllowanceDimension::BrowserActions => limits.max_browser_actions,
        AllowanceDimension::FanOut => limits.max_fan_out,
        AllowanceDimension::ResponseBytes => limits.max_response_bytes,
        AllowanceDimension::BrowserRenderedBytes => limits.max_browser_rendered_bytes,
        AllowanceDimension::LogicalWaits => u64::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_succeeds_and_atomic_denial_does_not_charge_other_dimensions() {
        let limits = PhaseLimits {
            max_strategy_attempts: 2,
            max_requests: 1,
            max_produced_items: 2,
            max_duration_ms: 120_000,
            max_pages: 1,
            max_browser_actions: 1,
            max_fan_out: 1,
            max_response_bytes: 67_108_864,
            max_browser_rendered_bytes: 67_108_864,
        };
        let allowance = InvocationAllowance::new(limits, true, None);
        allowance
            .debit(AllowanceCharge {
                requests: 1,
                pages: 1,
                ..AllowanceCharge::default()
            })
            .unwrap();
        let stop = allowance
            .debit(AllowanceCharge {
                strategy_attempts: 1,
                requests: 1,
                pages: 1,
                ..AllowanceCharge::default()
            })
            .unwrap_err();
        let AllowanceStop::Exhausted(exhaustion) = stop else {
            panic!("expected exhaustion")
        };
        assert_eq!(exhaustion.dimension, AllowanceDimension::Requests);
        let report = allowance.report(PhaseCompletion::PolicyUnsatisfied);
        assert_eq!(report.usage.strategy_attempts, 0);
        assert_eq!(report.usage.requests, 1);
        assert_eq!(report.usage.pages, 1);
    }

    #[test]
    fn equality_and_one_over_are_exact_for_every_counter_dimension() {
        let limits = PhaseLimits {
            max_strategy_attempts: 1,
            max_requests: 1,
            max_produced_items: 1,
            max_duration_ms: 120_000,
            max_pages: 1,
            max_browser_actions: 1,
            max_fan_out: 1,
            max_response_bytes: 67_108_864,
            max_browser_rendered_bytes: 67_108_864,
        };
        let cases = [
            (
                AllowanceDimension::StrategyAttempts,
                AllowanceCharge {
                    strategy_attempts: 1,
                    ..AllowanceCharge::default()
                },
            ),
            (
                AllowanceDimension::Requests,
                AllowanceCharge {
                    requests: 1,
                    ..AllowanceCharge::default()
                },
            ),
            (
                AllowanceDimension::ProducedItems,
                AllowanceCharge {
                    produced_items: 1,
                    ..AllowanceCharge::default()
                },
            ),
            (
                AllowanceDimension::Pages,
                AllowanceCharge {
                    pages: 1,
                    ..AllowanceCharge::default()
                },
            ),
            (
                AllowanceDimension::BrowserActions,
                AllowanceCharge {
                    browser_actions: 1,
                    ..AllowanceCharge::default()
                },
            ),
            (
                AllowanceDimension::FanOut,
                AllowanceCharge {
                    fan_out: 1,
                    ..AllowanceCharge::default()
                },
            ),
        ];
        for (dimension, charge) in cases {
            let allowance = InvocationAllowance::new(limits, true, None);
            allowance.debit(charge).expect("equality succeeds");
            let AllowanceStop::Exhausted(exhaustion) = allowance.debit(charge).unwrap_err() else {
                panic!("expected one-over exhaustion")
            };
            assert_eq!(exhaustion.dimension, dimension);
            assert_eq!(exhaustion.requested, 1);
            assert_eq!(exhaustion.remaining, 0);
        }
    }

    #[test]
    fn response_byte_prefix_is_committed_to_the_single_root_before_exhaustion() {
        let limits = PhaseLimits {
            max_response_bytes: 3,
            ..PhaseLimits::BACKEND
        };
        let allowance = InvocationAllowance::new(limits, true, None);
        allowance.commit_response_bytes(3, Some(1));
        let stop = allowance
            .stop()
            .expect("one proven excess byte exhausts response capacity");
        let AllowanceStop::Exhausted(exhaustion) = stop else {
            panic!("expected byte exhaustion")
        };
        assert_eq!(exhaustion.dimension, AllowanceDimension::ResponseBytes);
        assert_eq!(exhaustion.remaining, 0);
        let report = allowance.report(PhaseCompletion::BudgetExhausted { exhaustion });
        assert_eq!(report.usage.response_bytes, 3);
    }

    #[test]
    fn browser_teardown_deadline_partition_is_exact_and_carries_unused_time_forward() {
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        assert_eq!(
            allowance.deadline() - allowance.browser_work_deadline(),
            std::time::Duration::from_millis(2_000)
        );
        assert_eq!(
            allowance.browser_graceful_deadline() - allowance.browser_work_deadline(),
            std::time::Duration::from_millis(500)
        );
        assert_eq!(
            allowance.browser_force_deadline() - allowance.browser_graceful_deadline(),
            std::time::Duration::from_millis(1_000)
        );
        assert_eq!(
            allowance.browser_handler_deadline() - allowance.browser_force_deadline(),
            std::time::Duration::from_millis(250)
        );
        assert_eq!(
            allowance.deadline() - allowance.browser_handler_deadline(),
            std::time::Duration::from_millis(250)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn inactive_nested_duration_scopes_start_together_and_are_exact() {
        for (operation_ms, profile_ms, strategy_ms, boundary_ms) in [
            (60_000, 30_000, 20_000, 20_000),
            (60_000, 30_000, 60_000, 30_000),
            (60_000, 120_000, 120_000, 60_000),
        ] {
            let limits = |duration| PhaseLimits {
                max_duration_ms: duration,
                ..PhaseLimits::BACKEND
            };
            let allowance =
                InvocationAllowance::new_scope_root(limits(operation_ms), false, None, None, false);
            // Time before activation is intentionally outside Browser duration.
            tokio::time::advance(std::time::Duration::from_secs(70)).await;
            let profile = allowance
                .inactive_child_scope(InvocationAllowance::ROOT_SCOPE, limits(profile_ms), None)
                .unwrap();
            allowance.activate_scope_chain(profile).unwrap();
            let strategy = allowance
                .child_scope(profile, limits(strategy_ms), None)
                .unwrap();
            assert_eq!(
                allowance.deadline_for(strategy) - allowance.browser_work_deadline_for(strategy),
                std::time::Duration::from_millis(BROWSER_TEARDOWN_RESERVE_MS),
                "every tightest 20/30/60 scope retains the teardown reserve",
            );
            tokio::time::advance(std::time::Duration::from_millis(boundary_ms)).await;
            allowance
                .debit_for(strategy, AllowanceCharge::default(), None)
                .expect("exact Browser duration boundary succeeds");
            tokio::time::advance(std::time::Duration::from_millis(1)).await;
            let AllowanceStop::Exhausted(exhaustion) = allowance
                .debit_for(strategy, AllowanceCharge::default(), None)
                .expect_err("one millisecond over must exhaust the tightest scope")
            else {
                panic!("expected duration exhaustion")
            };
            assert_eq!(exhaustion.dimension, AllowanceDimension::Duration);
        }
    }

    #[test]
    fn nested_scope_debits_are_atomic_and_logical_waits_remain_private() {
        let operation = PhaseLimits {
            max_requests: 8,
            max_duration_ms: 60_000,
            max_browser_actions: 32,
            max_browser_rendered_bytes: 16,
            ..PhaseLimits::BACKEND
        };
        let profile = PhaseLimits {
            max_requests: 2,
            max_duration_ms: 30_000,
            max_browser_actions: 10,
            max_browser_rendered_bytes: 4,
            ..PhaseLimits::BACKEND
        };
        let strategy = PhaseLimits {
            max_requests: 1,
            max_duration_ms: 20_000,
            max_browser_actions: 32,
            max_browser_rendered_bytes: 2,
            ..PhaseLimits::BACKEND
        };
        let allowance =
            InvocationAllowance::new_with_logical_wait_limit(operation, false, None, Some(32));
        let profile_scope = allowance
            .child_scope(InvocationAllowance::ROOT_SCOPE, profile, Some(8))
            .unwrap();
        let strategy_scope = allowance
            .child_scope(profile_scope, strategy, Some(4))
            .unwrap();
        allowance
            .debit_for(
                strategy_scope,
                AllowanceCharge {
                    requests: 1,
                    browser_actions: 5,
                    logical_waits: 4,
                    ..AllowanceCharge::default()
                },
                None,
            )
            .unwrap();
        let stop = allowance
            .debit_for(
                strategy_scope,
                AllowanceCharge {
                    logical_waits: 1,
                    ..AllowanceCharge::default()
                },
                None,
            )
            .unwrap_err();
        let AllowanceStop::Exhausted(exhaustion) = stop else {
            panic!("expected nested exhaustion")
        };
        assert_eq!(exhaustion.dimension, AllowanceDimension::LogicalWaits);
        let scopes = allowance.scopes.lock().unwrap();
        assert_eq!(
            scopes[InvocationAllowance::ROOT_SCOPE].state.usage.requests,
            1
        );
        assert_eq!(scopes[profile_scope].state.usage.requests, 1);
        assert_eq!(scopes[strategy_scope].state.usage.requests, 1);
        assert_eq!(scopes[strategy_scope].logical_waits, 4);
        assert_eq!(scopes[InvocationAllowance::ROOT_SCOPE].state.usage.pages, 0);
    }

    #[test]
    fn nested_observed_byte_excess_consumes_every_applicable_remainder() {
        let operation = PhaseLimits {
            max_browser_rendered_bytes: 16,
            ..PhaseLimits::BACKEND
        };
        let profile = PhaseLimits {
            max_browser_rendered_bytes: 4,
            ..PhaseLimits::BACKEND
        };
        let strategy = PhaseLimits {
            max_browser_rendered_bytes: 2,
            ..PhaseLimits::BACKEND
        };
        let allowance = InvocationAllowance::new(operation, false, None);
        let profile_scope = allowance
            .child_scope(InvocationAllowance::ROOT_SCOPE, profile, None)
            .unwrap();
        let strategy_scope = allowance
            .child_scope(profile_scope, strategy, None)
            .unwrap();
        let stop = allowance
            .admit_browser_rendered_bytes_for(strategy_scope, 3)
            .unwrap_err();
        let AllowanceStop::Exhausted(exhaustion) = stop else {
            panic!("expected Browser byte exhaustion")
        };
        assert_eq!(
            exhaustion.dimension,
            AllowanceDimension::BrowserRenderedBytes
        );
        let scopes = allowance.scopes.lock().unwrap();
        assert_eq!(
            scopes[InvocationAllowance::ROOT_SCOPE]
                .state
                .usage
                .browser_rendered_bytes,
            16
        );
        assert_eq!(scopes[profile_scope].state.usage.browser_rendered_bytes, 4);
        assert_eq!(scopes[strategy_scope].state.usage.browser_rendered_bytes, 2);
    }

    #[test]
    fn checked_arithmetic_overflow_is_an_internal_stop() {
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        allowance.scopes.lock().unwrap()[InvocationAllowance::ROOT_SCOPE]
            .state
            .usage
            .requests = u64::MAX;
        assert!(matches!(
            allowance.debit(AllowanceCharge {
                requests: 1,
                ..AllowanceCharge::default()
            }),
            Err(AllowanceStop::Internal)
        ));
    }

    #[test]
    fn tied_limit_sources_are_reported_in_backend_compiled_caller_order() {
        let allowance =
            InvocationAllowance::new(PhaseLimits::BACKEND, true, Some(PhaseLimits::BACKEND));
        allowance.scopes.lock().unwrap()[InvocationAllowance::ROOT_SCOPE]
            .state
            .usage
            .browser_actions = PhaseLimits::BACKEND.max_browser_actions;
        let AllowanceStop::Exhausted(exhaustion) = allowance
            .debit(AllowanceCharge {
                browser_actions: 1,
                ..AllowanceCharge::default()
            })
            .unwrap_err()
        else {
            panic!("expected exhaustion")
        };
        assert_eq!(
            exhaustion.limit_sources,
            vec![
                AllowanceLimitSource::Backend,
                AllowanceLimitSource::Compiled,
                AllowanceLimitSource::Caller
            ]
        );
    }
}
