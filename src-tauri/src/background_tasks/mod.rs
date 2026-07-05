use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BACKGROUND_TASK_UPDATED_EVENT: &str = "background-task://updated";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackgroundTaskSchedulerConfig {
    pub max_running_tasks: usize,
    pub max_queued_tasks: usize,
}

impl Default for BackgroundTaskSchedulerConfig {
    fn default() -> Self {
        Self {
            max_running_tasks: 2,
            max_queued_tasks: 50,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskKind {
    SearchRun,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackgroundTaskSpec {
    pub kind: BackgroundTaskKind,
    pub incompatibility_key: Option<String>,
}

impl BackgroundTaskSpec {
    pub fn search_run() -> Self {
        Self {
            kind: BackgroundTaskKind::SearchRun,
            incompatibility_key: Some("search_run".to_string()),
        }
    }

    #[cfg(test)]
    pub(crate) fn compatible_fixture(name: impl Into<String>) -> Self {
        Self {
            kind: BackgroundTaskKind::Other(name.into()),
            incompatibility_key: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn incompatible_fixture(
        name: impl Into<String>,
        incompatibility_key: impl Into<String>,
    ) -> Self {
        Self {
            kind: BackgroundTaskKind::Other(name.into()),
            incompatibility_key: Some(incompatibility_key.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskState {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTaskProgress {
    pub message: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTaskSnapshot {
    pub task_id: String,
    pub kind: BackgroundTaskKind,
    pub state: BackgroundTaskState,
    pub progress: Option<BackgroundTaskProgress>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub diagnostics: crate::profile_dsl::diagnostics::Diagnostics,
}

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

pub struct BackgroundTaskContext {
    pub cancellation_token: CancellationToken,
    pub progress: BackgroundTaskProgressReporter,
}

#[derive(Clone)]
pub struct BackgroundTaskProgressReporter {
    scheduler: BackgroundTaskScheduler,
    task_id: String,
}

impl BackgroundTaskProgressReporter {
    pub fn report(
        &self,
        message: impl Into<String>,
        current: Option<u64>,
        total: Option<u64>,
    ) -> Result<(), String> {
        self.scheduler.update_progress(
            &self.task_id,
            BackgroundTaskProgress {
                message: message.into(),
                current,
                total,
            },
        )
    }
}

pub enum BackgroundTaskCompletion {
    Succeeded {
        result: Value,
    },
    Failed {
        error: String,
        diagnostics: crate::profile_dsl::diagnostics::Diagnostics,
    },
    Cancelled {
        error: Option<String>,
        result: Option<Value>,
        diagnostics: crate::profile_dsl::diagnostics::Diagnostics,
    },
}

type BackgroundTaskFuture = Pin<Box<dyn Future<Output = BackgroundTaskCompletion> + Send>>;
type BackgroundTaskWork = Box<dyn FnOnce(BackgroundTaskContext) -> BackgroundTaskFuture + Send>;

pub trait BackgroundTaskNotifier: Send + Sync {
    fn task_updated(&self, snapshot: &BackgroundTaskSnapshot);
}

#[derive(Default)]
pub struct NoopBackgroundTaskNotifier;

impl BackgroundTaskNotifier for NoopBackgroundTaskNotifier {
    fn task_updated(&self, _snapshot: &BackgroundTaskSnapshot) {}
}

#[derive(Clone)]
pub struct BackgroundTaskScheduler {
    inner: Arc<Mutex<BackgroundTaskSchedulerInner>>,
    notifier: Arc<dyn BackgroundTaskNotifier>,
}

struct BackgroundTaskSchedulerInner {
    config: BackgroundTaskSchedulerConfig,
    next_id: u64,
    tasks: BTreeMap<String, BackgroundTaskEntry>,
    queued: VecDeque<String>,
    running: HashSet<String>,
}

struct BackgroundTaskEntry {
    snapshot: BackgroundTaskSnapshot,
    spec: BackgroundTaskSpec,
    cancellation_token: CancellationToken,
    work: Option<BackgroundTaskWork>,
}

impl BackgroundTaskScheduler {
    #[cfg(test)]
    pub fn new(config: BackgroundTaskSchedulerConfig) -> Self {
        Self::new_with_notifier(config, Arc::new(NoopBackgroundTaskNotifier))
    }

    pub fn new_with_notifier(
        config: BackgroundTaskSchedulerConfig,
        notifier: Arc<dyn BackgroundTaskNotifier>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BackgroundTaskSchedulerInner {
                config,
                next_id: 0,
                tasks: BTreeMap::new(),
                queued: VecDeque::new(),
                running: HashSet::new(),
            })),
            notifier,
        }
    }

    pub fn schedule<F, Fut>(
        &self,
        spec: BackgroundTaskSpec,
        work: F,
    ) -> Result<BackgroundTaskSnapshot, String>
    where
        F: FnOnce(BackgroundTaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = BackgroundTaskCompletion> + Send + 'static,
    {
        let work: BackgroundTaskWork = Box::new(move |context| Box::pin(work(context)));
        let (snapshot, start) = {
            let mut inner = self.lock_inner()?;
            let task_id = format!("task-{}", inner.next_id + 1);
            inner.next_id += 1;
            let can_start = inner.can_start(&spec);
            if !can_start && inner.queued.len() >= inner.config.max_queued_tasks {
                return Err(format!(
                    "background task queue is full (maxQueuedTasks={})",
                    inner.config.max_queued_tasks
                ));
            }

            let state = if can_start {
                BackgroundTaskState::Running
            } else {
                BackgroundTaskState::Queued
            };
            let snapshot = BackgroundTaskSnapshot {
                task_id: task_id.clone(),
                kind: spec.kind.clone(),
                state,
                progress: None,
                result: None,
                error: None,
                diagnostics: Vec::new(),
            };
            let cancellation_token = CancellationToken::new();
            if can_start {
                inner.tasks.insert(
                    task_id.clone(),
                    BackgroundTaskEntry {
                        snapshot: snapshot.clone(),
                        spec,
                        cancellation_token: cancellation_token.clone(),
                        work: None,
                    },
                );
                inner.running.insert(task_id.clone());
                (snapshot, Some((task_id, cancellation_token, work)))
            } else {
                inner.tasks.insert(
                    task_id.clone(),
                    BackgroundTaskEntry {
                        snapshot: snapshot.clone(),
                        spec,
                        cancellation_token,
                        work: Some(work),
                    },
                );
                inner.queued.push_back(task_id);
                (snapshot, None)
            }
        };

        self.notify(&snapshot);
        if let Some((task_id, cancellation_token, work)) = start {
            self.spawn_task(task_id, cancellation_token, work);
        }

        Ok(snapshot)
    }

    pub fn get(&self, task_id: &str) -> Result<BackgroundTaskSnapshot, String> {
        let inner = self.lock_inner()?;
        inner
            .tasks
            .get(task_id)
            .map(|entry| entry.snapshot.clone())
            .ok_or_else(|| format!("background task `{task_id}` not found"))
    }

    pub fn cancel(&self, task_id: &str) -> Result<BackgroundTaskSnapshot, String> {
        let (snapshot, queued_cancelled) = {
            let mut inner = self.lock_inner()?;
            let entry = inner
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("background task `{task_id}` not found"))?;

            match entry.snapshot.state {
                BackgroundTaskState::Queued => {
                    entry.cancellation_token.cancel();
                    entry.work = None;
                    entry.snapshot.state = BackgroundTaskState::Cancelled;
                    entry.snapshot.error =
                        Some("background task cancelled before start".to_string());
                    let snapshot = entry.snapshot.clone();
                    inner.queued.retain(|queued_id| queued_id != task_id);
                    (snapshot, true)
                }
                BackgroundTaskState::Running => {
                    entry.cancellation_token.cancel();
                    entry.snapshot.state = BackgroundTaskState::Cancelling;
                    (entry.snapshot.clone(), false)
                }
                BackgroundTaskState::Cancelling => {
                    entry.cancellation_token.cancel();
                    (entry.snapshot.clone(), false)
                }
                BackgroundTaskState::Succeeded
                | BackgroundTaskState::Failed
                | BackgroundTaskState::Cancelled => (entry.snapshot.clone(), false),
            }
        };

        self.notify(&snapshot);
        if queued_cancelled {
            self.pump_queue()?;
        }
        Ok(snapshot)
    }

    fn update_progress(
        &self,
        task_id: &str,
        progress: BackgroundTaskProgress,
    ) -> Result<(), String> {
        let snapshot = {
            let mut inner = self.lock_inner()?;
            let entry = inner
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("background task `{task_id}` not found"))?;
            entry.snapshot.progress = Some(progress);
            entry.snapshot.clone()
        };
        self.notify(&snapshot);
        Ok(())
    }

    fn spawn_task(
        &self,
        task_id: String,
        cancellation_token: CancellationToken,
        work: BackgroundTaskWork,
    ) {
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            let context = BackgroundTaskContext {
                cancellation_token,
                progress: BackgroundTaskProgressReporter {
                    scheduler: scheduler.clone(),
                    task_id: task_id.clone(),
                },
            };
            let completion = work(context).await;
            let _ = scheduler.finish_task(&task_id, completion);
        });
    }

    fn finish_task(
        &self,
        task_id: &str,
        completion: BackgroundTaskCompletion,
    ) -> Result<(), String> {
        let snapshot = {
            let mut inner = self.lock_inner()?;
            inner.running.remove(task_id);
            let entry = inner
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| format!("background task `{task_id}` not found"))?;
            match completion {
                BackgroundTaskCompletion::Succeeded { result } => {
                    entry.snapshot.state = BackgroundTaskState::Succeeded;
                    entry.snapshot.result = Some(result);
                    entry.snapshot.error = None;
                    entry.snapshot.diagnostics = Vec::new();
                }
                BackgroundTaskCompletion::Failed { error, diagnostics } => {
                    entry.snapshot.state = BackgroundTaskState::Failed;
                    entry.snapshot.result = None;
                    entry.snapshot.error = Some(error);
                    entry.snapshot.diagnostics = diagnostics;
                }
                BackgroundTaskCompletion::Cancelled {
                    error,
                    result,
                    diagnostics,
                } => {
                    entry.snapshot.state = BackgroundTaskState::Cancelled;
                    entry.snapshot.result = result;
                    entry.snapshot.error = error;
                    entry.snapshot.diagnostics = diagnostics;
                }
            }
            entry.snapshot.clone()
        };
        self.notify(&snapshot);
        self.pump_queue()
    }

    fn pump_queue(&self) -> Result<(), String> {
        loop {
            let next = {
                let mut inner = self.lock_inner()?;
                let Some(position) = inner.queued.iter().position(|task_id| {
                    inner
                        .tasks
                        .get(task_id)
                        .is_some_and(|entry| inner.can_start(&entry.spec))
                }) else {
                    return Ok(());
                };
                let task_id = inner
                    .queued
                    .remove(position)
                    .expect("queue position should contain a task id");
                inner.running.insert(task_id.clone());
                let entry = inner
                    .tasks
                    .get_mut(&task_id)
                    .expect("queued task should have an entry");
                entry.snapshot.state = BackgroundTaskState::Running;
                let snapshot = entry.snapshot.clone();
                let cancellation_token = entry.cancellation_token.clone();
                let work = entry
                    .work
                    .take()
                    .expect("queued runnable task should have work");
                (snapshot, task_id, cancellation_token, work)
            };

            let (snapshot, task_id, cancellation_token, work) = next;
            self.notify(&snapshot);
            self.spawn_task(task_id, cancellation_token, work);
        }
    }

    fn notify(&self, snapshot: &BackgroundTaskSnapshot) {
        self.notifier.task_updated(snapshot);
    }

    fn lock_inner(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BackgroundTaskSchedulerInner>, String> {
        self.inner
            .lock()
            .map_err(|_| "background task scheduler is unavailable".to_string())
    }
}

impl BackgroundTaskSchedulerInner {
    fn can_start(&self, spec: &BackgroundTaskSpec) -> bool {
        self.running.len() < self.config.max_running_tasks
            && self.running.iter().all(|running_id| {
                self.tasks
                    .get(running_id)
                    .is_none_or(|running_entry| compatible(spec, &running_entry.spec))
            })
    }
}

fn compatible(left: &BackgroundTaskSpec, right: &BackgroundTaskSpec) -> bool {
    match (&left.incompatibility_key, &right.incompatibility_key) {
        (Some(left), Some(right)) => left != right,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::{mpsc, oneshot};

    #[test]
    fn scheduled_task_returns_id_and_finishes_with_queryable_result() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig::default());

            let scheduled = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("fixture"),
                    |_context| async {
                        BackgroundTaskCompletion::Succeeded {
                            result: json!({ "ok": true }),
                        }
                    },
                )
                .unwrap();

            assert_eq!(scheduled.task_id, "task-1");
            assert_eq!(scheduled.state, BackgroundTaskState::Running);
            let finished = wait_for_state(
                &scheduler,
                &scheduled.task_id,
                BackgroundTaskState::Succeeded,
            )
            .await;
            assert_eq!(finished.result, Some(json!({ "ok": true })));
        });
    }

    #[test]
    fn incompatible_tasks_are_queued_until_running_task_finishes() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig {
                max_running_tasks: 2,
                max_queued_tasks: 10,
            });
            let (release_first, first_released) = oneshot::channel::<()>();
            let (release_second, second_released) = oneshot::channel::<()>();

            let first = scheduler
                .schedule(
                    BackgroundTaskSpec::incompatible_fixture("first", "search_run"),
                    move |_context| async move {
                        let _ = first_released.await;
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("first"),
                        }
                    },
                )
                .unwrap();
            let second = scheduler
                .schedule(
                    BackgroundTaskSpec::incompatible_fixture("second", "search_run"),
                    move |_context| async move {
                        let _ = second_released.await;
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("second"),
                        }
                    },
                )
                .unwrap();

            assert_eq!(first.state, BackgroundTaskState::Running);
            assert_eq!(second.state, BackgroundTaskState::Queued);
            release_first.send(()).unwrap();
            wait_for_state(&scheduler, &first.task_id, BackgroundTaskState::Succeeded).await;
            let second_running =
                wait_for_state(&scheduler, &second.task_id, BackgroundTaskState::Running).await;
            assert_eq!(second_running.state, BackgroundTaskState::Running);
            release_second.send(()).unwrap();
            wait_for_state(&scheduler, &second.task_id, BackgroundTaskState::Succeeded).await;
        });
    }

    #[test]
    fn compatible_tasks_run_concurrently_within_configured_bounds() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig {
                max_running_tasks: 2,
                max_queued_tasks: 10,
            });
            let (started_tx, mut started_rx) = mpsc::unbounded_channel::<String>();
            let (release_first, first_released) = oneshot::channel::<()>();
            let (release_second, second_released) = oneshot::channel::<()>();

            let first_started_tx = started_tx.clone();
            let first = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("first"),
                    move |_context| {
                        let started_tx = first_started_tx.clone();
                        async move {
                            started_tx.send("first".to_string()).unwrap();
                            let _ = first_released.await;
                            BackgroundTaskCompletion::Succeeded {
                                result: json!("first"),
                            }
                        }
                    },
                )
                .unwrap();
            let second = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("second"),
                    move |_context| async move {
                        started_tx.send("second".to_string()).unwrap();
                        let _ = second_released.await;
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("second"),
                        }
                    },
                )
                .unwrap();

            assert_eq!(first.state, BackgroundTaskState::Running);
            assert_eq!(second.state, BackgroundTaskState::Running);
            let mut started = vec![
                started_rx.recv().await.unwrap(),
                started_rx.recv().await.unwrap(),
            ];
            started.sort();
            assert_eq!(started, vec!["first", "second"]);
            release_first.send(()).unwrap();
            release_second.send(()).unwrap();
            wait_for_state(&scheduler, &first.task_id, BackgroundTaskState::Succeeded).await;
            wait_for_state(&scheduler, &second.task_id, BackgroundTaskState::Succeeded).await;
        });
    }

    #[test]
    fn notifier_receives_task_state_and_progress_updates_for_ui_events() {
        tauri::async_runtime::block_on(async {
            let snapshots = Arc::new(Mutex::new(Vec::<BackgroundTaskSnapshot>::new()));
            let notifier = Arc::new(RecordingNotifier {
                snapshots: snapshots.clone(),
            });
            let scheduler = BackgroundTaskScheduler::new_with_notifier(
                BackgroundTaskSchedulerConfig::default(),
                notifier,
            );

            let task = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("notify"),
                    |context| async move {
                        context
                            .progress
                            .report("working", Some(1), Some(1))
                            .unwrap();
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("done"),
                        }
                    },
                )
                .unwrap();
            wait_for_state(&scheduler, &task.task_id, BackgroundTaskState::Succeeded).await;

            let snapshots = snapshots.lock().unwrap();
            assert!(snapshots.iter().any(|snapshot| {
                snapshot.task_id == task.task_id && snapshot.state == BackgroundTaskState::Running
            }));
            assert!(snapshots.iter().any(|snapshot| {
                snapshot.task_id == task.task_id
                    && snapshot
                        .progress
                        .as_ref()
                        .is_some_and(|progress| progress.message == "working")
            }));
            assert!(snapshots.iter().any(|snapshot| {
                snapshot.task_id == task.task_id && snapshot.state == BackgroundTaskState::Succeeded
            }));
        });
    }

    struct RecordingNotifier {
        snapshots: Arc<Mutex<Vec<BackgroundTaskSnapshot>>>,
    }

    impl BackgroundTaskNotifier for RecordingNotifier {
        fn task_updated(&self, snapshot: &BackgroundTaskSnapshot) {
            self.snapshots.lock().unwrap().push(snapshot.clone());
        }
    }

    #[test]
    fn task_progress_is_queryable_while_task_is_running() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig::default());
            let (release, released) = oneshot::channel::<()>();

            let task = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("progress"),
                    move |context| async move {
                        context
                            .progress
                            .report("half done", Some(1), Some(2))
                            .unwrap();
                        let _ = released.await;
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("done"),
                        }
                    },
                )
                .unwrap();

            let progressed = wait_for_progress(&scheduler, &task.task_id, "half done").await;
            assert_eq!(progressed.progress.unwrap().current, Some(1));
            release.send(()).unwrap();
            wait_for_state(&scheduler, &task.task_id, BackgroundTaskState::Succeeded).await;
        });
    }

    #[test]
    fn queue_backpressure_rejects_tasks_beyond_explicit_limit() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig {
                max_running_tasks: 1,
                max_queued_tasks: 1,
            });
            let (_release_first, first_released) = oneshot::channel::<()>();
            scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("first"),
                    move |_context| async move {
                        let _ = first_released.await;
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("first"),
                        }
                    },
                )
                .unwrap();
            let queued = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("second"),
                    |_context| async {
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("second"),
                        }
                    },
                )
                .unwrap();
            assert_eq!(queued.state, BackgroundTaskState::Queued);

            let error = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("third"),
                    |_context| async {
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("third"),
                        }
                    },
                )
                .unwrap_err();

            assert!(error.contains("background task queue is full"));
        });
    }

    #[test]
    fn failed_task_does_not_poison_queued_work() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig {
                max_running_tasks: 1,
                max_queued_tasks: 10,
            });
            let first = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("first"),
                    |_context| async {
                        BackgroundTaskCompletion::Failed {
                            error: "boom".to_string(),
                            diagnostics: Vec::new(),
                        }
                    },
                )
                .unwrap();
            let second = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("second"),
                    |_context| async {
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("second"),
                        }
                    },
                )
                .unwrap();

            wait_for_state(&scheduler, &first.task_id, BackgroundTaskState::Failed).await;
            let finished =
                wait_for_state(&scheduler, &second.task_id, BackgroundTaskState::Succeeded).await;
            assert_eq!(finished.result, Some(json!("second")));
        });
    }

    #[test]
    fn queued_and_running_tasks_can_be_cancelled_cooperatively() {
        tauri::async_runtime::block_on(async {
            let scheduler = BackgroundTaskScheduler::new(BackgroundTaskSchedulerConfig {
                max_running_tasks: 1,
                max_queued_tasks: 10,
            });
            let first = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("first"),
                    move |context| async move {
                        context.cancellation_token.cancelled().await;
                        BackgroundTaskCompletion::Cancelled {
                            error: Some("cancelled".to_string()),
                            result: None,
                            diagnostics: Vec::new(),
                        }
                    },
                )
                .unwrap();
            let queued = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("queued"),
                    |_context| async {
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("queued"),
                        }
                    },
                )
                .unwrap();

            let queued_cancelled = scheduler.cancel(&queued.task_id).unwrap();
            assert_eq!(queued_cancelled.state, BackgroundTaskState::Cancelled);
            let first_cancelling = scheduler.cancel(&first.task_id).unwrap();
            assert_eq!(first_cancelling.state, BackgroundTaskState::Cancelling);
            let first_cancelled =
                wait_for_state(&scheduler, &first.task_id, BackgroundTaskState::Cancelled).await;
            assert_eq!(first_cancelled.error.as_deref(), Some("cancelled"));
        });
    }

    #[test]
    fn running_task_cancellation_request_is_immediately_observable() {
        tauri::async_runtime::block_on(async {
            let snapshots = Arc::new(Mutex::new(Vec::<BackgroundTaskSnapshot>::new()));
            let notifier = Arc::new(RecordingNotifier {
                snapshots: snapshots.clone(),
            });
            let scheduler = BackgroundTaskScheduler::new_with_notifier(
                BackgroundTaskSchedulerConfig::default(),
                notifier,
            );
            let (started_tx, started_rx) = oneshot::channel::<()>();
            let (release, released) = oneshot::channel::<()>();

            let task = scheduler
                .schedule(
                    BackgroundTaskSpec::compatible_fixture("cancellable"),
                    move |context| async move {
                        started_tx.send(()).unwrap();
                        let _ = released.await;
                        if context.cancellation_token.is_cancelled() {
                            return BackgroundTaskCompletion::Cancelled {
                                error: Some("cancelled".to_string()),
                                result: None,
                                diagnostics: Vec::new(),
                            };
                        }
                        BackgroundTaskCompletion::Succeeded {
                            result: json!("done"),
                        }
                    },
                )
                .unwrap();
            started_rx.await.unwrap();

            let cancelling = scheduler.cancel(&task.task_id).unwrap();

            assert_eq!(cancelling.state, BackgroundTaskState::Cancelling);
            assert_eq!(
                scheduler.get(&task.task_id).unwrap().state,
                BackgroundTaskState::Cancelling
            );
            assert!(snapshots.lock().unwrap().iter().any(|snapshot| {
                snapshot.task_id == task.task_id
                    && snapshot.state == BackgroundTaskState::Cancelling
            }));
            release.send(()).unwrap();
            wait_for_state(&scheduler, &task.task_id, BackgroundTaskState::Cancelled).await;
        });
    }

    async fn wait_for_state(
        scheduler: &BackgroundTaskScheduler,
        task_id: &str,
        state: BackgroundTaskState,
    ) -> BackgroundTaskSnapshot {
        for _ in 0..100 {
            let snapshot = scheduler.get(task_id).unwrap();
            if snapshot.state == state {
                return snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task {task_id} did not reach state {state:?}");
    }

    async fn wait_for_progress(
        scheduler: &BackgroundTaskScheduler,
        task_id: &str,
        message: &str,
    ) -> BackgroundTaskSnapshot {
        for _ in 0..100 {
            let snapshot = scheduler.get(task_id).unwrap();
            if snapshot
                .progress
                .as_ref()
                .is_some_and(|progress| progress.message == message)
            {
                return snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task {task_id} did not report progress {message}");
    }
}
