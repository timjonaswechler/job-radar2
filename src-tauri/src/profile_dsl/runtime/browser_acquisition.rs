use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::time::Instant;

use crate::profile_dsl::{
    documents::PhaseLimits,
    execution_plan::capabilities::{ExecutionPlanBrowserInteraction, ExecutionPlanBrowserWait},
};

use super::{
    allowance::{
        completion_for_stop, AllowanceCharge, InvocationAllowance, PhaseCompletion,
        PhaseExecutionReport,
    },
    cancellation::{RuntimeCancellation, RuntimeExecutionContext},
};

pub(crate) type BoxedBrowserAcquisitionFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<BrowserRenderedContent, BrowserAcquisitionTerminal>> + Send + 'a,
    >,
>;

/// Phase-neutral, vendor-neutral Browser acquisition input. The control attachment
/// borrows the caller-owned cumulative phase allowance; an acquisition cannot mint
/// or reset capacity.
pub struct BrowserAcquisitionRequest<'a> {
    pub target: String,
    pub waits: Vec<ExecutionPlanBrowserWait>,
    pub interactions: Vec<ExecutionPlanBrowserInteraction>,
    hard_deadline: Instant,
    control: RuntimeExecutionContext<'a>,
}

impl<'a> BrowserAcquisitionRequest<'a> {
    pub(crate) fn new(
        target: String,
        waits: Vec<ExecutionPlanBrowserWait>,
        interactions: Vec<ExecutionPlanBrowserInteraction>,
        control: RuntimeExecutionContext<'a>,
    ) -> Result<Self, BrowserAcquisitionFailure> {
        let hard_deadline = control.deadline().ok_or_else(|| {
            BrowserAcquisitionFailure::new(
                BrowserAcquisitionFailureKind::RuntimeLaunch,
                "Browser acquisition requires caller-owned cumulative control",
            )
        })?;
        Ok(Self {
            target,
            waits,
            interactions,
            hard_deadline,
            control,
        })
    }

    pub fn hard_deadline(&self) -> Instant {
        self.hard_deadline
    }

    pub(crate) fn browser_work_deadline(&self) -> Instant {
        self.control
            .browser_work_deadline()
            .expect("Browser acquisition always has caller-owned control")
    }

    pub(crate) fn browser_graceful_deadline(&self) -> Instant {
        self.control
            .browser_graceful_deadline()
            .expect("Browser acquisition always has caller-owned control")
    }

    pub(crate) fn browser_force_deadline(&self) -> Instant {
        self.control
            .browser_force_deadline()
            .expect("Browser acquisition always has caller-owned control")
    }

    pub(crate) fn browser_handler_deadline(&self) -> Instant {
        self.control
            .browser_handler_deadline()
            .expect("Browser acquisition always has caller-owned control")
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }

    pub(crate) async fn cancelled(&self) {
        self.control.cancelled().await;
    }

    pub(crate) fn mark_deadline(&self) {
        self.control.mark_deadline();
    }

    pub(crate) fn admit_navigation(&self) -> Result<(), BrowserAcquisitionTerminal> {
        self.control
            .debit(AllowanceCharge {
                requests: 1,
                ..AllowanceCharge::default()
            })
            .map_err(|_| BrowserAcquisitionTerminal::AllowanceStopped)
    }

    pub(crate) fn admit_wait(&self) -> Result<(), BrowserAcquisitionTerminal> {
        self.control
            .debit(AllowanceCharge::default())
            .map_err(|_| BrowserAcquisitionTerminal::AllowanceStopped)
    }

    pub(crate) fn admit_interaction(&self) -> Result<(), BrowserAcquisitionTerminal> {
        self.control
            .debit(AllowanceCharge {
                browser_actions: 1,
                ..AllowanceCharge::default()
            })
            .map_err(|_| BrowserAcquisitionTerminal::AllowanceStopped)
    }

    pub(crate) fn admit_rendered_content(
        &self,
        content: String,
    ) -> Result<BrowserRenderedContent, BrowserAcquisitionTerminal> {
        self.control
            .admit_browser_rendered_bytes(content.len() as u64)
            .map_err(|_| BrowserAcquisitionTerminal::AllowanceStopped)?;
        Ok(BrowserRenderedContent(content))
    }

    fn snapshot(&self) -> BrowserAcquisitionRequestSnapshot {
        BrowserAcquisitionRequestSnapshot {
            target: self.target.clone(),
            waits: self.waits.clone(),
            interactions: self.interactions.clone(),
            browser_rendered_bytes_remaining: self.control.remaining_browser_rendered_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserAcquisitionRequestSnapshot {
    pub target: String,
    pub waits: Vec<ExecutionPlanBrowserWait>,
    pub interactions: Vec<ExecutionPlanBrowserInteraction>,
    pub browser_rendered_bytes_remaining: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserRenderedContent(String);

impl BrowserRenderedContent {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn utf8_len(&self) -> u64 {
        self.0.len() as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserAcquisitionFailure {
    pub kind: BrowserAcquisitionFailureKind,
    pub message: String,
}

impl BrowserAcquisitionFailure {
    pub fn new(kind: BrowserAcquisitionFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserAcquisitionFailureKind {
    RuntimeLaunch,
    Navigation,
    Wait { wait_index: usize },
    Interaction { interaction_index: usize },
    ContentRead,
    Deadline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserInfrastructureFailure {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserAcquisitionCancellationReason {
    UserCancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserAcquisitionCancellation {
    pub reason: BrowserAcquisitionCancellationReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserAcquisitionTerminal {
    Failure(BrowserAcquisitionFailure),
    InfrastructureFailure(BrowserInfrastructureFailure),
    AllowanceStopped,
    Cancelled(BrowserAcquisitionCancellation),
}

pub trait BrowserAcquisition: Send + Sync {
    fn acquire<'a>(
        &'a self,
        request: BrowserAcquisitionRequest<'a>,
    ) -> BoxedBrowserAcquisitionFuture<'a>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptedBrowserAcquisitionEvent {
    Gate(String),
    Navigate,
    Wait {
        wait_index: usize,
    },
    Interaction {
        interaction_index: usize,
        attempted_clicks: u64,
    },
    Content(String),
    Failure(BrowserAcquisitionFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptedBrowserFinalization {
    Graceful { gate: Option<String> },
    Forced { gate: Option<String> },
    InfrastructureFailure { message: String },
}

impl Default for ScriptedBrowserFinalization {
    fn default() -> Self {
        Self::Graceful { gate: None }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptedBrowserAcquisitionExpectation {
    pub request: BrowserAcquisitionRequestSnapshot,
    pub events: Vec<ScriptedBrowserAcquisitionEvent>,
    pub finalization: ScriptedBrowserFinalization,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserLifecycleEvent {
    Reserved,
    Navigation,
    Wait { wait_index: usize },
    InteractionAttempt { interaction_index: usize },
    ContentRead,
    PrimarySealed,
    GracefulClose,
    ForceTerminate,
    Reaped,
    ReapFailed,
    HandlerCompleted,
    HandlerAborted,
    ActiveSessionReleased,
    SessionFinalized,
}

pub struct ScriptedBrowserAcquisition {
    expectations: Mutex<VecDeque<ScriptedBrowserAcquisitionExpectation>>,
    requests: Mutex<Vec<BrowserAcquisitionRequestSnapshot>>,
    lifecycle: Mutex<Vec<BrowserLifecycleEvent>>,
    mismatches: Mutex<Vec<String>>,
    gates: Mutex<BTreeMap<String, Arc<tokio::sync::Notify>>>,
}

impl ScriptedBrowserAcquisition {
    pub fn new(
        expectations: impl IntoIterator<Item = ScriptedBrowserAcquisitionExpectation>,
    ) -> Self {
        Self {
            expectations: Mutex::new(expectations.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            lifecycle: Mutex::new(Vec::new()),
            mismatches: Mutex::new(Vec::new()),
            gates: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn requests(&self) -> Vec<BrowserAcquisitionRequestSnapshot> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn lifecycle(&self) -> Vec<BrowserLifecycleEvent> {
        self.lifecycle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn mismatches(&self) -> Vec<String> {
        self.mismatches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn expectations_satisfied(&self) -> bool {
        self.expectations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
            && self.mismatches().is_empty()
    }

    pub fn gate_is_waiting(&self, name: &str) -> bool {
        self.gates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(name)
    }

    pub fn release_gate(&self, name: &str) -> bool {
        let gate = self
            .gates
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(name)
            .cloned();
        if let Some(gate) = gate {
            gate.notify_one();
            true
        } else {
            false
        }
    }

    fn log(&self, event: BrowserLifecycleEvent) {
        self.lifecycle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event);
    }

    fn mismatch(&self, message: impl Into<String>) -> BrowserAcquisitionTerminal {
        self.mismatches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(message.into());
        BrowserAcquisitionTerminal::Failure(BrowserAcquisitionFailure::new(
            BrowserAcquisitionFailureKind::RuntimeLaunch,
            "scripted Browser acquisition contract mismatch",
        ))
    }

    async fn await_gate(
        &self,
        name: String,
        control: RuntimeExecutionContext<'_>,
        deadline: Option<Instant>,
    ) -> GateOutcome {
        let notify = {
            let mut gates = self.gates.lock().unwrap_or_else(|p| p.into_inner());
            gates
                .entry(name)
                .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
                .clone()
        };
        let deadline = async move {
            match deadline {
                Some(deadline) => tokio::time::sleep_until(deadline).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            biased;
            _ = control.cancelled() => GateOutcome::Cancelled,
            _ = deadline => GateOutcome::Deadline,
            _ = notify.notified() => GateOutcome::Released,
        }
    }

    async fn await_cleanup_gate_after_cancellation(
        &self,
        name: String,
        deadline: Option<Instant>,
    ) -> bool {
        let notify = {
            let mut gates = self.gates.lock().unwrap_or_else(|p| p.into_inner());
            gates
                .entry(name)
                .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
                .clone()
        };
        let deadline = async move {
            match deadline {
                Some(deadline) => tokio::time::sleep_until(deadline).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            _ = deadline => false,
            _ = notify.notified() => true,
        }
    }

    async fn finalize(
        &self,
        finalization: ScriptedBrowserFinalization,
        control: RuntimeExecutionContext<'_>,
        primary: &mut BrowserAcquisitionTerminalOrSuccess,
    ) -> Option<BrowserInfrastructureFailure> {
        self.log(BrowserLifecycleEvent::PrimarySealed);
        let cancellation_before_cleanup = control.is_cancelled();
        if cancellation_before_cleanup {
            set_cancellation(primary);
        }
        match finalization {
            ScriptedBrowserFinalization::Graceful { gate } => {
                self.log(BrowserLifecycleEvent::GracefulClose);
                let mut force_required = cancellation_before_cleanup;
                if !force_required {
                    if let Some(gate) = gate {
                        match self
                            .await_gate(gate, control, control.browser_graceful_deadline())
                            .await
                        {
                            GateOutcome::Cancelled => {
                                set_cancellation(primary);
                                force_required = true;
                            }
                            GateOutcome::Deadline => force_required = true,
                            GateOutcome::Released => {}
                        }
                    }
                }
                if force_required {
                    self.log(BrowserLifecycleEvent::ForceTerminate);
                    self.log(BrowserLifecycleEvent::Reaped);
                }
                if control.is_cancelled() {
                    set_cancellation(primary);
                }
                self.log(BrowserLifecycleEvent::HandlerCompleted);
                self.log(BrowserLifecycleEvent::ActiveSessionReleased);
                self.log(BrowserLifecycleEvent::SessionFinalized);
                None
            }
            ScriptedBrowserFinalization::Forced { gate } => {
                self.log(BrowserLifecycleEvent::GracefulClose);
                self.log(BrowserLifecycleEvent::ForceTerminate);
                if let Some(gate) = gate {
                    let reaped = if cancellation_before_cleanup {
                        self.await_cleanup_gate_after_cancellation(
                            gate,
                            control.browser_force_deadline(),
                        )
                        .await
                    } else {
                        match self
                            .await_gate(gate.clone(), control, control.browser_force_deadline())
                            .await
                        {
                            GateOutcome::Released => true,
                            GateOutcome::Deadline => false,
                            GateOutcome::Cancelled => {
                                set_cancellation(primary);
                                self.await_cleanup_gate_after_cancellation(
                                    gate,
                                    control.browser_force_deadline(),
                                )
                                .await
                            }
                        }
                    };
                    if !reaped {
                        self.log(BrowserLifecycleEvent::ReapFailed);
                        self.log(BrowserLifecycleEvent::HandlerAborted);
                        self.log(BrowserLifecycleEvent::ActiveSessionReleased);
                        self.log(BrowserLifecycleEvent::SessionFinalized);
                        return Some(BrowserInfrastructureFailure {
                            message: "Browser process could not be reaped before the hard deadline"
                                .to_string(),
                        });
                    }
                }
                if control.is_cancelled() {
                    set_cancellation(primary);
                }
                self.log(BrowserLifecycleEvent::Reaped);
                self.log(BrowserLifecycleEvent::HandlerCompleted);
                self.log(BrowserLifecycleEvent::ActiveSessionReleased);
                self.log(BrowserLifecycleEvent::SessionFinalized);
                None
            }
            ScriptedBrowserFinalization::InfrastructureFailure { message } => {
                self.log(BrowserLifecycleEvent::GracefulClose);
                self.log(BrowserLifecycleEvent::ForceTerminate);
                self.log(BrowserLifecycleEvent::ReapFailed);
                self.log(BrowserLifecycleEvent::HandlerAborted);
                self.log(BrowserLifecycleEvent::ActiveSessionReleased);
                self.log(BrowserLifecycleEvent::SessionFinalized);
                Some(BrowserInfrastructureFailure { message })
            }
        }
    }
}

impl BrowserAcquisition for ScriptedBrowserAcquisition {
    fn acquire<'a>(
        &'a self,
        request: BrowserAcquisitionRequest<'a>,
    ) -> BoxedBrowserAcquisitionFuture<'a> {
        Box::pin(async move {
            let snapshot = request.snapshot();
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(snapshot.clone());
            self.log(BrowserLifecycleEvent::Reserved);
            let expectation = self
                .expectations
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pop_front();
            let (events, finalization, mut primary) = match expectation {
                Some(expectation) if expectation.request == snapshot => (
                    expectation.events,
                    expectation.finalization,
                    BrowserAcquisitionTerminalOrSuccess::Pending,
                ),
                Some(expectation) => {
                    let terminal = self.mismatch(format!(
                        "expected request {:?}, received {:?}",
                        expectation.request, snapshot
                    ));
                    (
                        Vec::new(),
                        ScriptedBrowserFinalization::default(),
                        BrowserAcquisitionTerminalOrSuccess::Terminal(terminal),
                    )
                }
                None => {
                    let terminal = self.mismatch(format!("unexpected request: {snapshot:?}"));
                    (
                        Vec::new(),
                        ScriptedBrowserFinalization::default(),
                        BrowserAcquisitionTerminalOrSuccess::Terminal(terminal),
                    )
                }
            };
            let mut navigated = false;
            let mut next_wait = 0usize;
            let mut next_interaction = 0usize;

            for event in events {
                if !matches!(primary, BrowserAcquisitionTerminalOrSuccess::Pending) {
                    break;
                }
                if request.control.is_cancelled() {
                    primary = cancelled();
                    break;
                }
                if request.control.deadline_is_expired() || Instant::now() >= request.hard_deadline
                {
                    request.control.mark_deadline();
                    primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                        BrowserAcquisitionTerminal::AllowanceStopped,
                    );
                    break;
                }
                match event {
                    ScriptedBrowserAcquisitionEvent::Gate(name) => {
                        match self
                            .await_gate(
                                name,
                                request.control,
                                request.control.browser_work_deadline(),
                            )
                            .await
                        {
                            GateOutcome::Released => {}
                            GateOutcome::Cancelled => primary = cancelled(),
                            GateOutcome::Deadline => {
                                request.control.mark_deadline();
                                primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                    BrowserAcquisitionTerminal::AllowanceStopped,
                                );
                            }
                        }
                    }
                    ScriptedBrowserAcquisitionEvent::Navigate => {
                        if navigated {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                self.mismatch("duplicate scripted navigation"),
                            );
                            continue;
                        }
                        if request
                            .control
                            .debit(AllowanceCharge {
                                requests: 1,
                                ..AllowanceCharge::default()
                            })
                            .is_err()
                        {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                BrowserAcquisitionTerminal::AllowanceStopped,
                            );
                            continue;
                        }
                        navigated = true;
                        self.log(BrowserLifecycleEvent::Navigation);
                    }
                    ScriptedBrowserAcquisitionEvent::Wait { wait_index } => {
                        if !navigated
                            || wait_index != next_wait
                            || wait_index >= request.waits.len()
                        {
                            primary =
                                BrowserAcquisitionTerminalOrSuccess::Terminal(self.mismatch(
                                    format!("unexpected scripted wait index {wait_index}"),
                                ));
                            continue;
                        }
                        if request.control.debit(AllowanceCharge::default()).is_err() {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                BrowserAcquisitionTerminal::AllowanceStopped,
                            );
                            continue;
                        }
                        next_wait += 1;
                        self.log(BrowserLifecycleEvent::Wait { wait_index });
                    }
                    ScriptedBrowserAcquisitionEvent::Interaction {
                        interaction_index,
                        attempted_clicks,
                    } => {
                        if !navigated
                            || next_wait != request.waits.len()
                            || interaction_index != next_interaction
                            || interaction_index >= request.interactions.len()
                            || attempted_clicks
                                > interaction_max_count(&request.interactions[interaction_index])
                        {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(self.mismatch(
                                format!(
                                    "invalid scripted interaction {interaction_index} with {attempted_clicks} attempted clicks"
                                ),
                            ));
                            continue;
                        }
                        for _ in 0..attempted_clicks {
                            if request
                                .control
                                .debit(AllowanceCharge {
                                    browser_actions: 1,
                                    ..AllowanceCharge::default()
                                })
                                .is_err()
                            {
                                primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                    BrowserAcquisitionTerminal::AllowanceStopped,
                                );
                                break;
                            }
                            self.log(BrowserLifecycleEvent::InteractionAttempt {
                                interaction_index,
                            });
                        }
                        next_interaction += 1;
                    }
                    ScriptedBrowserAcquisitionEvent::Content(content) => {
                        if !navigated
                            || next_wait != request.waits.len()
                            || next_interaction != request.interactions.len()
                        {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(self.mismatch(
                                "content was observed before all compiled waits/interactions completed",
                            ));
                            continue;
                        }
                        self.log(BrowserLifecycleEvent::ContentRead);
                        if request
                            .control
                            .admit_browser_rendered_bytes(content.len() as u64)
                            .is_err()
                        {
                            primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                BrowserAcquisitionTerminal::AllowanceStopped,
                            );
                        } else {
                            primary = BrowserAcquisitionTerminalOrSuccess::Success(
                                BrowserRenderedContent(content),
                            );
                        }
                    }
                    ScriptedBrowserAcquisitionEvent::Failure(failure) => {
                        let stage_admitted = match failure.kind {
                            BrowserAcquisitionFailureKind::RuntimeLaunch => !navigated,
                            BrowserAcquisitionFailureKind::Navigation if !navigated => {
                                if request
                                    .control
                                    .debit(AllowanceCharge {
                                        requests: 1,
                                        ..AllowanceCharge::default()
                                    })
                                    .is_err()
                                {
                                    primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                        BrowserAcquisitionTerminal::AllowanceStopped,
                                    );
                                    continue;
                                }
                                self.log(BrowserLifecycleEvent::Navigation);
                                true
                            }
                            BrowserAcquisitionFailureKind::Wait { wait_index }
                                if navigated
                                    && wait_index == next_wait
                                    && wait_index < request.waits.len() =>
                            {
                                if request.control.debit(AllowanceCharge::default()).is_err() {
                                    primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                        BrowserAcquisitionTerminal::AllowanceStopped,
                                    );
                                    continue;
                                }
                                self.log(BrowserLifecycleEvent::Wait { wait_index });
                                true
                            }
                            BrowserAcquisitionFailureKind::Interaction { interaction_index }
                                if navigated
                                    && next_wait == request.waits.len()
                                    && interaction_index == next_interaction
                                    && interaction_index < request.interactions.len() =>
                            {
                                if request
                                    .control
                                    .debit(AllowanceCharge {
                                        browser_actions: 1,
                                        ..AllowanceCharge::default()
                                    })
                                    .is_err()
                                {
                                    primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                                        BrowserAcquisitionTerminal::AllowanceStopped,
                                    );
                                    continue;
                                }
                                self.log(BrowserLifecycleEvent::InteractionAttempt {
                                    interaction_index,
                                });
                                true
                            }
                            BrowserAcquisitionFailureKind::ContentRead
                                if navigated
                                    && next_wait == request.waits.len()
                                    && next_interaction == request.interactions.len() =>
                            {
                                self.log(BrowserLifecycleEvent::ContentRead);
                                true
                            }
                            // A cumulative hard deadline is produced only by the real shared
                            // control. Scripts may gate time but cannot inject that terminal.
                            BrowserAcquisitionFailureKind::Deadline => false,
                            _ => false,
                        };
                        primary =
                            BrowserAcquisitionTerminalOrSuccess::Terminal(if stage_admitted {
                                BrowserAcquisitionTerminal::Failure(failure)
                            } else {
                                self.mismatch(
                                    "scripted failure did not match the reached acquisition stage",
                                )
                            });
                    }
                }
            }

            if matches!(primary, BrowserAcquisitionTerminalOrSuccess::Pending) {
                primary = BrowserAcquisitionTerminalOrSuccess::Terminal(
                    self.mismatch("script ended without rendered content or a typed failure"),
                );
            }

            if let Some(infrastructure) = self
                .finalize(finalization, request.control, &mut primary)
                .await
            {
                return Err(BrowserAcquisitionTerminal::InfrastructureFailure(
                    infrastructure,
                ));
            }
            match primary {
                BrowserAcquisitionTerminalOrSuccess::Success(content) => Ok(content),
                BrowserAcquisitionTerminalOrSuccess::Terminal(terminal) => Err(terminal),
                BrowserAcquisitionTerminalOrSuccess::Pending => unreachable!(),
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GateOutcome {
    Released,
    Cancelled,
    Deadline,
}

enum BrowserAcquisitionTerminalOrSuccess {
    Pending,
    Success(BrowserRenderedContent),
    Terminal(BrowserAcquisitionTerminal),
}

fn cancelled() -> BrowserAcquisitionTerminalOrSuccess {
    BrowserAcquisitionTerminalOrSuccess::Terminal(BrowserAcquisitionTerminal::Cancelled(
        BrowserAcquisitionCancellation {
            reason: BrowserAcquisitionCancellationReason::UserCancelled,
        },
    ))
}

fn set_cancellation(primary: &mut BrowserAcquisitionTerminalOrSuccess) {
    if !matches!(
        primary,
        BrowserAcquisitionTerminalOrSuccess::Terminal(
            BrowserAcquisitionTerminal::AllowanceStopped
                | BrowserAcquisitionTerminal::InfrastructureFailure(_)
        )
    ) {
        *primary = cancelled();
    }
}

fn interaction_max_count(interaction: &ExecutionPlanBrowserInteraction) -> u64 {
    match interaction {
        ExecutionPlanBrowserInteraction::ClickIfVisible { max_count, .. }
        | ExecutionPlanBrowserInteraction::ClickUntilGone { max_count, .. } => *max_count,
    }
}

/// Hidden deterministic invocation owner for external contract tests. Productive
/// phase callers create the same `InvocationAllowance` and pass its attached
/// `RuntimeExecutionContext` directly; Browser acquisition never owns this root.
#[doc(hidden)]
pub struct BrowserAcquisitionTestInvocation {
    allowance: InvocationAllowance,
}

impl BrowserAcquisitionTestInvocation {
    pub fn new(
        compiled: PhaseLimits,
        compiled_authored: bool,
        caller: Option<PhaseLimits>,
    ) -> Self {
        Self {
            allowance: InvocationAllowance::new(compiled, compiled_authored, caller),
        }
    }

    pub fn request(
        &self,
        target: impl Into<String>,
        waits: Vec<ExecutionPlanBrowserWait>,
        interactions: Vec<ExecutionPlanBrowserInteraction>,
    ) -> BrowserAcquisitionRequest<'_> {
        BrowserAcquisitionRequest::new(
            target.into(),
            waits,
            interactions,
            RuntimeExecutionContext::uncancellable().for_invocation(&self.allowance),
        )
        .expect("test invocation always supplies cumulative control")
    }

    pub fn request_with_cancellation<'a>(
        &'a self,
        target: impl Into<String>,
        waits: Vec<ExecutionPlanBrowserWait>,
        interactions: Vec<ExecutionPlanBrowserInteraction>,
        cancellation: &'a dyn RuntimeCancellation,
    ) -> BrowserAcquisitionRequest<'a> {
        BrowserAcquisitionRequest::new(
            target.into(),
            waits,
            interactions,
            RuntimeExecutionContext::with_cancellation(cancellation)
                .for_invocation(&self.allowance),
        )
        .expect("test invocation always supplies cumulative control")
    }

    pub fn report(&self, fallback: PhaseCompletion) -> PhaseExecutionReport {
        let completion = self
            .allowance
            .stop()
            .map(completion_for_stop)
            .unwrap_or(fallback);
        self.allowance.report(completion)
    }
}
