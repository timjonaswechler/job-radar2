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

/// Detection's only HTTP allowance. It deliberately owns no posting-phase limits or report.
pub(crate) struct DetectionHttpAllowance {
    response_bytes: Mutex<u64>,
    exhausted: Mutex<Option<AllowanceExhaustion>>,
}

impl DetectionHttpAllowance {
    pub(crate) const LIMIT: u64 = 67_108_864;

    pub(crate) fn new() -> Self {
        Self {
            response_bytes: Mutex::new(0),
            exhausted: Mutex::new(None),
        }
    }

    pub(crate) fn response_bytes(&self) -> u64 {
        *self
            .response_bytes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn remaining_response_bytes(&self) -> u64 {
        Self::LIMIT.saturating_sub(self.response_bytes())
    }

    pub(crate) fn exhaustion(&self) -> Option<AllowanceExhaustion> {
        self.exhausted
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub(crate) fn commit_response_bytes(&self, admitted: u64, exceeded: Option<u64>) {
        let mut used = self
            .response_bytes
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let remaining_before = Self::LIMIT.saturating_sub(*used);
        *used = used.saturating_add(admitted.min(remaining_before));
        let remaining_after = Self::LIMIT.saturating_sub(*used);
        drop(used);
        if let Some(requested) = exceeded {
            let mut exhaustion = self.exhausted.lock().unwrap_or_else(|p| p.into_inner());
            if exhaustion.is_none() {
                *exhaustion = Some(AllowanceExhaustion {
                    dimension: AllowanceDimension::ResponseBytes,
                    requested,
                    remaining: remaining_after,
                    limit_sources: vec![AllowanceLimitSource::Backend],
                });
            }
        }
    }
}

pub(crate) struct InvocationAllowance {
    limits: EffectiveLimits,
    started_at: Instant,
    deadline: Instant,
    state: Mutex<LedgerState>,
    stop: Mutex<Option<AllowanceStop>>,
}

impl InvocationAllowance {
    pub(crate) fn new(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
    ) -> Self {
        let values = caller.map_or(compiled, |caller| compiled.minimum(caller));
        let started_at = Instant::now();
        Self {
            limits: EffectiveLimits {
                values,
                compiled,
                compiled_authored,
                caller,
            },
            started_at,
            deadline: started_at + std::time::Duration::from_millis(values.max_duration_ms),
            state: Mutex::new(LedgerState::default()),
            stop: Mutex::new(None),
        }
    }

    pub(crate) fn effective_limits(&self) -> PhaseLimits {
        self.limits.values
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn browser_work_deadline(&self) -> Instant {
        self.deadline
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_TEARDOWN_RESERVE_MS,
            ))
            .expect("browser-capable phase duration is compiler/caller validated")
    }

    pub(crate) fn browser_graceful_deadline(&self) -> Instant {
        self.deadline
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_FORCE_TERMINATE_REAP_MS
                    + BROWSER_HANDLER_COMPLETION_MS
                    + BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable phase duration is compiler/caller validated")
    }

    pub(crate) fn browser_force_deadline(&self) -> Instant {
        self.deadline
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_HANDLER_COMPLETION_MS + BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable phase duration is compiler/caller validated")
    }

    pub(crate) fn browser_handler_deadline(&self) -> Instant {
        self.deadline
            .checked_sub(std::time::Duration::from_millis(
                BROWSER_SESSION_FINALIZATION_MS,
            ))
            .expect("browser-capable phase duration is compiler/caller validated")
    }

    pub(crate) fn debit(&self, charge: AllowanceCharge) -> Result<(), AllowanceStop> {
        self.debit_with_pagination_limit(charge, None)
    }

    pub(crate) fn debit_with_pagination_limit(
        &self,
        charge: AllowanceCharge,
        pagination_max_requests: Option<u64>,
    ) -> Result<(), AllowanceStop> {
        if let Some(stop) = self.stop() {
            return Err(stop);
        }
        let elapsed = self.elapsed_ms();
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.usage.duration_ms = elapsed;
        let before_duration = [
            (
                AllowanceDimension::StrategyAttempts,
                state.usage.strategy_attempts,
                charge.strategy_attempts,
                self.limits.values.max_strategy_attempts,
            ),
            (
                AllowanceDimension::Requests,
                state.usage.requests,
                charge.requests,
                pagination_max_requests.map_or(self.limits.values.max_requests, |limit| {
                    self.limits.values.max_requests.min(limit)
                }),
            ),
            (
                AllowanceDimension::ProducedItems,
                state.usage.produced_items,
                charge.produced_items,
                self.limits.values.max_produced_items,
            ),
            (
                AllowanceDimension::ResponseBytes,
                state.usage.response_bytes,
                charge.response_bytes,
                self.limits.values.max_response_bytes,
            ),
        ];
        for (dimension, used, requested, limit) in before_duration {
            let Some(next) = used.checked_add(requested) else {
                drop(state);
                return Err(self.fail_internal());
            };
            if next > limit {
                drop(state);
                return Err(self.exhaust_at_limit(
                    dimension,
                    requested,
                    limit.saturating_sub(used),
                    pagination_max_requests,
                    limit,
                ));
            }
        }
        if Instant::now() > self.deadline {
            drop(state);
            return Err(self.exhaust(AllowanceDimension::Duration, 1, 0));
        }
        let after_duration = [
            (
                AllowanceDimension::Pages,
                state.usage.pages,
                charge.pages,
                pagination_max_requests.map_or(self.limits.values.max_pages, |limit| {
                    self.limits.values.max_pages.min(limit)
                }),
            ),
            (
                AllowanceDimension::BrowserActions,
                state.usage.browser_actions,
                charge.browser_actions,
                self.limits.values.max_browser_actions,
            ),
            (
                AllowanceDimension::FanOut,
                state.usage.fan_out,
                charge.fan_out,
                self.limits.values.max_fan_out,
            ),
        ];
        for (dimension, used, requested, limit) in after_duration {
            let Some(next) = used.checked_add(requested) else {
                drop(state);
                return Err(self.fail_internal());
            };
            if next > limit {
                drop(state);
                return Err(self.exhaust_at_limit(
                    dimension,
                    requested,
                    limit.saturating_sub(used),
                    pagination_max_requests,
                    limit,
                ));
            }
        }
        state.usage.strategy_attempts += charge.strategy_attempts;
        state.usage.requests += charge.requests;
        state.usage.produced_items += charge.produced_items;
        state.usage.pages += charge.pages;
        state.usage.browser_actions += charge.browser_actions;
        state.usage.fan_out += charge.fan_out;
        state.usage.response_bytes += charge.response_bytes;
        Ok(())
    }

    pub(crate) fn admit_browser_rendered_bytes(&self, observed: u64) -> Result<(), AllowanceStop> {
        let mut stop = self.stop.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(stop) = stop.clone() {
            return Err(stop);
        }
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let remaining = self
            .limits
            .values
            .max_browser_rendered_bytes
            .saturating_sub(state.usage.browser_rendered_bytes);
        let admitted = observed.min(remaining);
        let Some(next) = state.usage.browser_rendered_bytes.checked_add(admitted) else {
            drop(state);
            let internal = AllowanceStop::Internal;
            *stop = Some(internal.clone());
            return Err(internal);
        };
        state.usage.browser_rendered_bytes = next;
        if observed <= remaining {
            return Ok(());
        }
        drop(state);
        let exhaustion = AllowanceExhaustion {
            dimension: AllowanceDimension::BrowserRenderedBytes,
            requested: observed - remaining,
            remaining: 0,
            limit_sources: self.sources(AllowanceDimension::BrowserRenderedBytes),
        };
        let exhausted = AllowanceStop::Exhausted(exhaustion);
        *stop = Some(exhausted.clone());
        Err(exhausted)
    }

    pub(crate) fn remaining_browser_rendered_bytes(&self) -> u64 {
        let used = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .usage
            .browser_rendered_bytes;
        self.limits
            .values
            .max_browser_rendered_bytes
            .saturating_sub(used)
    }

    pub(crate) fn remaining_response_bytes(&self) -> u64 {
        let used = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .usage
            .response_bytes;
        self.limits.values.max_response_bytes.saturating_sub(used)
    }

    /// Commits bytes already admitted by the HTTP collector exactly once.
    ///
    /// Unlike generic debit this deliberately ignores an elapsed deadline and an
    /// already-recorded stop: transport bytes that crossed H01 remain observable
    /// even when cancellation or another terminal condition wins afterward.
    pub(crate) fn commit_response_bytes(&self, admitted: u64, exceeded: Option<u64>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let remaining = match state.usage.response_bytes.checked_add(admitted) {
            Some(next) if next <= self.limits.values.max_response_bytes => {
                state.usage.response_bytes = next;
                self.limits.values.max_response_bytes - next
            }
            _ => {
                drop(state);
                let _ = self.fail_internal();
                return;
            }
        };
        drop(state);
        if let Some(requested) = exceeded {
            let _ = self.exhaust(AllowanceDimension::ResponseBytes, requested, remaining);
        }
    }

    pub(crate) fn mark_deadline(&self) {
        let _ = self.exhaust(AllowanceDimension::Duration, 1, 0);
    }

    pub(crate) fn mark_deadline_if_expired(&self) {
        if Instant::now() > self.deadline {
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
        let mut usage = self.state.lock().unwrap_or_else(|p| p.into_inner()).usage;
        usage.duration_ms = self.elapsed_ms();
        PhaseExecutionReport { usage, completion }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
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

    fn exhaust(
        &self,
        dimension: AllowanceDimension,
        requested: u64,
        remaining: u64,
    ) -> AllowanceStop {
        let sources = self.sources(dimension);
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

    fn exhaust_at_limit(
        &self,
        dimension: AllowanceDimension,
        requested: u64,
        remaining: u64,
        pagination_max_requests: Option<u64>,
        effective_limit: u64,
    ) -> AllowanceStop {
        let pagination_applies = matches!(
            dimension,
            AllowanceDimension::Requests | AllowanceDimension::Pages
        ) && pagination_max_requests
            .is_some_and(|limit| limit == effective_limit);
        if !pagination_applies {
            return self.exhaust(dimension, requested, remaining);
        }
        let mut sources = self.sources(dimension);
        if !sources.contains(&AllowanceLimitSource::Compiled) {
            let index = usize::from(sources.first() == Some(&AllowanceLimitSource::Backend));
            sources.insert(index, AllowanceLimitSource::Compiled);
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

    fn sources(&self, dimension: AllowanceDimension) -> Vec<AllowanceLimitSource> {
        let value = dimension_value(self.limits.values, dimension);
        let mut sources = Vec::new();
        if dimension_value(PhaseLimits::BACKEND, dimension) == value {
            sources.push(AllowanceLimitSource::Backend);
        }
        if self.limits.compiled_authored
            && dimension_value(self.limits.compiled, dimension) == value
        {
            sources.push(AllowanceLimitSource::Compiled);
        }
        if self
            .limits
            .caller
            .is_some_and(|limits| dimension_value(limits, dimension) == value)
        {
            sources.push(AllowanceLimitSource::Caller);
        }
        sources
    }
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

    #[test]
    fn checked_arithmetic_overflow_is_an_internal_stop() {
        let allowance = InvocationAllowance::new(PhaseLimits::BACKEND, false, None);
        allowance.state.lock().unwrap().usage.requests = u64::MAX;
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
        allowance.state.lock().unwrap().usage.browser_actions =
            PhaseLimits::BACKEND.max_browser_actions;
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
