use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::Notify;

use crate::domain::{
    identity::Clock,
    task_scheduler::{
        BoxFuture, ScheduledTask, TaskClaimRequest, TaskCompleteRequest, TaskEnqueueResult,
        TaskFailRequest, TaskHandler, TaskHeartbeatRequest, TaskListFilter,
        TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
        TaskRunOutcome, TaskSchedulerError, TaskStatus, TaskWorker, TaskWorkerRepository,
    },
};

pub async fn wait_for_notify(
    notify: &Arc<Notify>,
    timeout_duration: Duration,
) -> Result<(), tokio::time::error::Elapsed> {
    tokio::time::timeout(timeout_duration, notify.notified()).await
}

#[derive(Clone, Default)]
pub struct InMemoryTaskRepository {
    tasks: Arc<Mutex<HashMap<String, ScheduledTask>>>,
}

impl InMemoryTaskRepository {
    pub fn only_task(&self) -> ScheduledTask {
        self.tasks
            .lock()
            .expect("task repo mutex poisoned")
            .values()
            .next()
            .cloned()
            .expect("expected one task in repo")
    }
}

impl TaskRepository for InMemoryTaskRepository {
    fn enqueue_if_absent(
        &self,
        task: ScheduledTask,
    ) -> BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            if let Some(existing) = tasks.values().find(|existing| {
                existing.user_id == task.user_id && existing.dedupe_key == task.dedupe_key
            }) {
                return Ok(TaskEnqueueResult {
                    task: existing.clone(),
                    created: false,
                });
            }
            tasks.insert(task.id.clone(), task.clone());
            Ok(TaskEnqueueResult {
                task,
                created: true,
            })
        })
    }

    fn claim_next_due(
        &self,
        request: TaskClaimRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let task_id = tasks
                .values()
                .find(|task| {
                    matches!(task.status, TaskStatus::Queued | TaskStatus::RetryScheduled)
                        && task.next_attempt_at_epoch_seconds <= request.now_epoch_seconds
                        && request
                            .enabled_task_types
                            .iter()
                            .any(|value| value == &task.task_type)
                        && (request.is_leader || !task.leader_only)
                })
                .map(|task| task.id.clone());
            let Some(task_id) = task_id else {
                return Ok(None);
            };
            let task = tasks.get_mut(&task_id).expect("task should exist");
            task.status = TaskStatus::Running;
            task.claimed_by = Some(request.worker_id);
            task.lease_expires_at_epoch_seconds = Some(request.lease_expires_at_epoch_seconds);
            task.last_heartbeat_at_epoch_seconds = Some(request.now_epoch_seconds);
            task.started_at_epoch_seconds = Some(request.now_epoch_seconds);
            task.updated_at_epoch_seconds = request.now_epoch_seconds;
            task.attempt_count = task.attempt_count.saturating_add(1);
            Ok(Some(task.clone()))
        })
    }

    fn heartbeat(
        &self,
        request: TaskHeartbeatRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.claimed_by.as_deref() != Some(request.worker_id.as_str()) {
                return Ok(None);
            }
            task.last_heartbeat_at_epoch_seconds = Some(request.last_heartbeat_at_epoch_seconds);
            task.lease_expires_at_epoch_seconds = Some(request.lease_expires_at_epoch_seconds);
            task.updated_at_epoch_seconds = request.last_heartbeat_at_epoch_seconds;
            Ok(Some(task.clone()))
        })
    }

    fn save_checkpoint(
        &self,
        _request: crate::domain::task_scheduler::TaskCheckpointRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        Box::pin(async { Ok(None) })
    }

    fn complete(
        &self,
        request: TaskCompleteRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.claimed_by.as_deref() != Some(request.worker_id.as_str()) {
                return Ok(None);
            }
            task.status = TaskStatus::Completed;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.updated_at_epoch_seconds = request.completed_at_epoch_seconds;
            task.finished_at_epoch_seconds = Some(request.completed_at_epoch_seconds);
            task.checkpoint = request.checkpoint;
            Ok(Some(task.clone()))
        })
    }

    fn fail(
        &self,
        request: TaskFailRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.claimed_by.as_deref() != Some(request.worker_id.as_str()) {
                return Ok(None);
            }
            task.status = if request.retry_at_epoch_seconds.is_some() {
                TaskStatus::RetryScheduled
            } else {
                TaskStatus::Failed
            };
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.updated_at_epoch_seconds = request.failed_at_epoch_seconds;
            task.next_attempt_at_epoch_seconds = request
                .retry_at_epoch_seconds
                .unwrap_or(request.failed_at_epoch_seconds);
            task.finished_at_epoch_seconds = request
                .retry_at_epoch_seconds
                .map(|_| None)
                .unwrap_or(Some(request.failed_at_epoch_seconds));
            task.checkpoint = request.checkpoint;
            task.error_message = Some(request.error_message);
            Ok(Some(task.clone()))
        })
    }

    fn list_timeout_candidates(
        &self,
        _now_epoch_seconds: i64,
        _limit: usize,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn mark_timed_out(
        &self,
        _request: TaskMarkTimedOutRequest,
    ) -> BoxFuture<Result<bool, TaskSchedulerError>> {
        Box::pin(async { Ok(false) })
    }

    fn recover(&self, _request: TaskRecoverRequest) -> BoxFuture<Result<bool, TaskSchedulerError>> {
        Box::pin(async { Ok(false) })
    }

    fn retry(
        &self,
        request: TaskRetryRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if !task.can_retry_manually() {
                return Ok(None);
            }
            task.status = TaskStatus::Queued;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.timed_out_at_epoch_seconds = None;
            task.finished_at_epoch_seconds = None;
            task.next_attempt_at_epoch_seconds = request.retried_at_epoch_seconds;
            task.updated_at_epoch_seconds = request.retried_at_epoch_seconds;
            Ok(Some(task.clone()))
        })
    }

    fn find_by_id(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            Ok(tasks
                .lock()
                .expect("task repo mutex poisoned")
                .get(&task_id)
                .cloned())
        })
    }

    fn list(
        &self,
        _filter: TaskListFilter,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
pub struct InMemoryTaskWorkerRepository {
    workers: Arc<Mutex<HashMap<String, TaskWorker>>>,
}

impl InMemoryTaskWorkerRepository {
    pub fn worker(&self, worker_id: &str) -> Option<TaskWorker> {
        self.workers
            .lock()
            .expect("worker repo mutex poisoned")
            .get(worker_id)
            .cloned()
    }
}

impl TaskWorkerRepository for InMemoryTaskWorkerRepository {
    fn upsert(&self, worker: TaskWorker) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        Box::pin(async move {
            workers
                .lock()
                .expect("worker repo mutex poisoned")
                .insert(worker.worker_id.clone(), worker.clone());
            Ok(worker)
        })
    }

    fn touch_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        last_heartbeat_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            let worker = TaskWorker {
                worker_id: worker_id.clone(),
                is_leader,
                enabled_task_types,
                active_task_ids: Vec::new(),
                last_heartbeat_at_epoch_seconds,
            };
            workers
                .lock()
                .expect("worker repo mutex poisoned")
                .insert(worker_id, worker.clone());
            Ok(worker)
        })
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>> {
        let workers = self.workers.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            Ok(workers
                .lock()
                .expect("worker repo mutex poisoned")
                .get(&worker_id)
                .cloned())
        })
    }
}

#[derive(Clone)]
pub struct TestClock {
    now_epoch_seconds: Arc<Mutex<i64>>,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::new(1_700_000_000)
    }
}

impl TestClock {
    pub fn new(now_epoch_seconds: i64) -> Self {
        Self {
            now_epoch_seconds: Arc::new(Mutex::new(now_epoch_seconds)),
        }
    }
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        *self.now_epoch_seconds.lock().expect("clock mutex poisoned")
    }
}

pub struct StaticTaskHandler {
    task_type: &'static str,
}

impl StaticTaskHandler {
    pub fn new(task_type: &'static str) -> Arc<Self> {
        Arc::new(Self { task_type })
    }
}

impl TaskHandler for StaticTaskHandler {
    fn task_type(&self) -> &'static str {
        self.task_type
    }

    fn run(&self, _task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        Box::pin(async { TaskRunOutcome::Completed { checkpoint: None } })
    }
}

pub struct PanicTaskHandler {
    pub started: Arc<Notify>,
    pub release: Arc<Notify>,
}

impl PanicTaskHandler {
    pub const TASK_TYPE: &'static str = "panic.task";

    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        })
    }
}

impl TaskHandler for PanicTaskHandler {
    fn task_type(&self) -> &'static str {
        Self::TASK_TYPE
    }

    fn run(&self, _task: ScheduledTask) -> BoxFuture<TaskRunOutcome> {
        let started = self.started.clone();
        let release = self.release.clone();
        Box::pin(async move {
            started.notify_one();
            release.notified().await;
            panic!("panic task handler boom");
        })
    }
}
