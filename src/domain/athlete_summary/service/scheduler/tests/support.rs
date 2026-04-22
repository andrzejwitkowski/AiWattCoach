use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use tokio::sync::watch;

use crate::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryGenerationClaimResult,
        AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationRepository,
        AthleteSummaryGenerationOperationStatus, AthleteSummaryGenerator, AthleteSummaryRepository,
    },
    identity::{Clock, IdGenerator},
    llm::{LlmCacheUsage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
    task_scheduler::{
        FailTaskInput, ScheduledTask, SharedTaskHandler, TaskClaimRequest, TaskCompleteRequest,
        TaskEnqueueResult, TaskFailRequest, TaskHeartbeatRequest, TaskListFilter,
        TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
        TaskRunOutcome, TaskSchedulerError, TaskSchedulerService, TaskStatus, TaskWorker,
        TaskWorkerConfig, TaskWorkerRepository,
    },
};

pub(super) const USER_ID: &str = "user-1";
pub(super) const NOW_EPOCH_SECONDS: i64 = 1_775_564_800;
pub(super) const THIS_WEEK_EPOCH_SECONDS: i64 = 1_775_520_000;
pub(super) const LAST_WEEK_EPOCH_SECONDS: i64 = 1_775_347_200;
const MODEL: &str = "google/gemini-3-flash-preview";

#[derive(Clone)]
pub(super) struct InMemoryAthleteSummaryRepository {
    summary: Arc<Mutex<Option<AthleteSummary>>>,
}

impl Default for InMemoryAthleteSummaryRepository {
    fn default() -> Self {
        Self {
            summary: Arc::new(Mutex::new(None)),
        }
    }
}

impl InMemoryAthleteSummaryRepository {
    pub(super) fn with_summary(summary: AthleteSummary) -> Self {
        Self {
            summary: Arc::new(Mutex::new(Some(summary))),
        }
    }
}

impl AthleteSummaryRepository for InMemoryAthleteSummaryRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummary>, AthleteSummaryError>,
    > {
        let summary = self.summary.lock().unwrap().clone();
        Box::pin(async move { Ok(summary) })
    }

    fn upsert(
        &self,
        summary: AthleteSummary,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let store = self.summary.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(summary.clone());
            Ok(summary)
        })
    }
}

#[derive(Clone)]
pub(super) struct InMemoryAthleteSummaryOperationRepository {
    operation: Arc<Mutex<Option<AthleteSummaryGenerationOperation>>>,
}

impl Default for InMemoryAthleteSummaryOperationRepository {
    fn default() -> Self {
        Self {
            operation: Arc::new(Mutex::new(None)),
        }
    }
}

impl AthleteSummaryGenerationOperationRepository for InMemoryAthleteSummaryOperationRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummaryGenerationOperation>, AthleteSummaryError>,
    > {
        let operation = self.operation.lock().unwrap().clone();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: AthleteSummaryGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> crate::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationClaimResult, AthleteSummaryError>,
    > {
        let store = self.operation.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let claim_result = match store.clone() {
                None => {
                    *store = Some(operation.clone());
                    AthleteSummaryGenerationClaimResult::Claimed(operation)
                }
                Some(existing)
                    if existing.status == AthleteSummaryGenerationOperationStatus::Failed
                        || (existing.status
                            == AthleteSummaryGenerationOperationStatus::Pending
                            && existing.last_attempt_at_epoch_seconds
                                <= stale_before_epoch_seconds) =>
                {
                    let reclaimed = AthleteSummaryGenerationOperation {
                        user_id: existing.user_id.clone(),
                        status: AthleteSummaryGenerationOperationStatus::Pending,
                        summary_text: existing.summary_text.clone(),
                        provider: existing.provider.clone(),
                        model: existing.model.clone(),
                        error_message: None,
                        started_at_epoch_seconds: existing.started_at_epoch_seconds,
                        last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
                        attempt_count: existing.attempt_count.saturating_add(1),
                        created_at_epoch_seconds: existing.created_at_epoch_seconds,
                        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
                    };
                    *store = Some(reclaimed.clone());
                    AthleteSummaryGenerationClaimResult::Claimed(reclaimed)
                }
                Some(existing) => AthleteSummaryGenerationClaimResult::Existing(existing),
            };

            Ok(claim_result)
        })
    }

    fn upsert(
        &self,
        operation: AthleteSummaryGenerationOperation,
    ) -> crate::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationOperation, AthleteSummaryError>,
    > {
        let store = self.operation.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(operation.clone());
            Ok(operation)
        })
    }
}

impl InMemoryAthleteSummaryOperationRepository {
    pub(super) fn seed(&self, operation: AthleteSummaryGenerationOperation) {
        *self.operation.lock().unwrap() = Some(operation);
    }
}

#[derive(Clone)]
pub(super) struct StubGenerator {
    calls: Arc<Mutex<u32>>,
    responses: Arc<Mutex<VecDeque<Result<LlmChatResponse, LlmError>>>>,
}

impl StubGenerator {
    pub(super) fn succeeds_with(message: &str) -> Self {
        Self::queued(vec![Ok(llm_response(message))])
    }

    pub(super) fn failing(error: LlmError) -> Self {
        Self::queued(vec![Err(error)])
    }

    pub(super) fn queued(responses: Vec<Result<LlmChatResponse, LlmError>>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(0)),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }

    pub(super) fn call_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

impl AthleteSummaryGenerator for StubGenerator {
    fn generate(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<LlmChatResponse, LlmError>> {
        *self.calls.lock().unwrap() += 1;
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected queued generator response");
        Box::pin(async move { response })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryTaskRepository {
    tasks: Arc<Mutex<HashMap<String, ScheduledTask>>>,
}

impl InMemoryTaskRepository {
    pub(super) fn only_task(&self) -> ScheduledTask {
        self.tasks.lock().unwrap().values().next().cloned().unwrap()
    }

    pub(super) fn only_task_if_present(&self) -> Option<ScheduledTask> {
        self.tasks.lock().unwrap().values().next().cloned()
    }
}

impl TaskRepository for InMemoryTaskRepository {
    fn enqueue_if_absent(
        &self,
        task: ScheduledTask,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
            let task = tasks.get_mut(&task_id).unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        Box::pin(async { Ok(None) })
    }

    fn complete(
        &self,
        request: TaskCompleteRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>
    {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn mark_timed_out(
        &self,
        _request: TaskMarkTimedOutRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        Box::pin(async { Ok(false) })
    }

    fn recover(
        &self,
        _request: TaskRecoverRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        Box::pin(async { Ok(false) })
    }

    fn retry(
        &self,
        request: TaskRetryRequest,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().unwrap();
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>
    {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move { Ok(tasks.lock().unwrap().get(&task_id).cloned()) })
    }

    fn list(
        &self,
        _filter: TaskListFilter,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>
    {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryTaskWorkerRepository {
    workers: Arc<Mutex<HashMap<String, TaskWorker>>>,
}

impl TaskWorkerRepository for InMemoryTaskWorkerRepository {
    fn upsert(
        &self,
        worker: TaskWorker,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        Box::pin(async move {
            workers
                .lock()
                .unwrap()
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
    ) -> crate::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
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
            workers.lock().unwrap().insert(worker_id, worker.clone());
            Ok(worker)
        })
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> crate::domain::task_scheduler::BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>>
    {
        let workers = self.workers.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move { Ok(workers.lock().unwrap().get(&worker_id).cloned()) })
    }
}

pub(super) fn spawn_test_task_worker<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    config: TaskWorkerConfig,
    handlers: Vec<SharedTaskHandler>,
) -> Result<TestTaskWorkerHandle, TaskSchedulerError>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let mut enabled_task_types = Vec::with_capacity(handlers.len());
    let mut handlers_by_type = HashMap::with_capacity(handlers.len());
    for handler in handlers {
        let task_type = handler.task_type().to_string();
        if handlers_by_type
            .insert(task_type.clone(), handler)
            .is_some()
        {
            return Err(TaskSchedulerError::Conflict(format!(
                "duplicate task handler registered for {task_type}"
            )));
        }
        enabled_task_types.push(task_type);
    }

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let join_handle = tokio::spawn(async move {
        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            match scheduler
                .claim_next_due(
                    &worker_id,
                    enabled_task_types.clone(),
                    config.is_leader,
                    config.lease_duration_seconds,
                )
                .await
            {
                Ok(Some(task)) => {
                    let handler = handlers_by_type.get(&task.task_type).cloned();
                    run_test_task(&scheduler, &worker_id, task, handler).await;
                }
                Ok(None) | Err(_) => {
                    let should_stop = tokio::select! {
                        _ = shutdown_rx.changed() => true,
                        _ = tokio::time::sleep(config.idle_poll_interval) => *shutdown_rx.borrow(),
                    };
                    if should_stop {
                        break;
                    }
                }
            }
        }
    });

    Ok(TestTaskWorkerHandle {
        shutdown: shutdown_tx,
        join_handle: Some(join_handle),
    })
}

pub(super) struct TestTaskWorkerHandle {
    shutdown: watch::Sender<bool>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

impl TestTaskWorkerHandle {
    pub(super) async fn shutdown(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.await;
        }
    }
}

impl Drop for TestTaskWorkerHandle {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.abort();
        }
    }
}

async fn run_test_task<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    task: ScheduledTask,
    handler: Option<SharedTaskHandler>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let outcome = match handler {
        Some(handler) => handler.run(task.clone()).await,
        None => TaskRunOutcome::Failed {
            checkpoint: None,
            error_message: format!(
                "no task handler registered for task type {}",
                task.task_type
            ),
            retryable: false,
            retry_delay_seconds: None,
        },
    };

    match outcome {
        TaskRunOutcome::Completed { checkpoint } => {
            let _ = scheduler
                .complete_task(&task.id, worker_id, checkpoint)
                .await;
        }
        TaskRunOutcome::Failed {
            checkpoint,
            error_message,
            retryable,
            retry_delay_seconds,
        } => {
            let _ = scheduler
                .fail_task(FailTaskInput {
                    task_id: &task.id,
                    worker_id,
                    checkpoint,
                    error_message,
                    retryable,
                    retry_delay_seconds,
                    retry_strategy: &task.retry_strategy,
                    attempt_count: task.attempt_count,
                })
                .await;
        }
    }
}

#[derive(Clone)]
pub(super) struct TestClock {
    now_epoch_seconds: Arc<Mutex<i64>>,
}

impl TestClock {
    pub(super) fn new(now_epoch_seconds: i64) -> Self {
        Self {
            now_epoch_seconds: Arc::new(Mutex::new(now_epoch_seconds)),
        }
    }

    pub(super) fn set_now(&self, now_epoch_seconds: i64) {
        *self.now_epoch_seconds.lock().unwrap() = now_epoch_seconds;
    }
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        *self.now_epoch_seconds.lock().unwrap()
    }
}

#[derive(Clone, Default)]
pub(super) struct TestIdGenerator {
    next_id: Arc<Mutex<usize>>,
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        let mut next_id = self.next_id.lock().unwrap();
        let id = format!("{prefix}-{}", *next_id);
        *next_id += 1;
        id
    }
}

pub(super) fn summary(summary_text: &str, generated_at_epoch_seconds: i64) -> AthleteSummary {
    AthleteSummary {
        user_id: USER_ID.to_string(),
        summary_text: summary_text.to_string(),
        generated_at_epoch_seconds,
        created_at_epoch_seconds: generated_at_epoch_seconds,
        updated_at_epoch_seconds: generated_at_epoch_seconds,
        provider: Some("openrouter".to_string()),
        model: Some(MODEL.to_string()),
    }
}

pub(super) fn llm_response(message: &str) -> LlmChatResponse {
    LlmChatResponse {
        provider: LlmProvider::OpenRouter,
        model: MODEL.to_string(),
        message: message.to_string(),
        provider_request_id: None,
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    }
}
