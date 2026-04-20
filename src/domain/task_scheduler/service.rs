use std::{collections::HashMap, sync::Arc};

use tokio::sync::{watch, Mutex};

use crate::domain::identity::Clock;

use super::{
    BoxFuture, ScheduledTask, TaskCheckpointRequest, TaskClaimRequest, TaskCompleteRequest,
    TaskEnqueueResult, TaskFailRequest, TaskHeartbeatRequest, TaskListFilter,
    TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
    TaskSchedulerError, TaskStatus, TaskWorker, TaskWorkerRepository,
};

pub struct FailTaskInput<'a> {
    pub task_id: &'a str,
    pub worker_id: &'a str,
    pub checkpoint: Option<serde_json::Value>,
    pub error_message: String,
    pub retryable: bool,
    pub retry_strategy: &'a crate::domain::task_scheduler::RetryStrategy,
    pub attempt_count: u32,
}

pub trait ResultTaskHandler: Send + Sync + 'static {
    type Completed: Send + 'static;
    type Output: Send + 'static;
    type Error: Send + 'static;

    fn task_disappeared(&self, task_id: &str) -> Self::Error;

    fn task_timed_out(&self, task_id: &str) -> Self::Error;

    fn parse_completed(&self, task: &ScheduledTask) -> Result<Self::Completed, Self::Error>;

    fn parse_failed(&self, task: &ScheduledTask) -> Result<Self::Error, Self::Error>;

    fn finish(&self, completed: Self::Completed) -> BoxFuture<Result<Self::Output, Self::Error>>;
}

type TaskWatchReceiver = watch::Receiver<Option<ScheduledTask>>;
type TaskWatchSender = watch::Sender<Option<ScheduledTask>>;

#[derive(Clone, Debug)]
struct WorkerState {
    is_leader: bool,
    enabled_task_types: Vec<String>,
    active_task_ids: Vec<String>,
}

#[derive(Clone)]
pub struct TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    tasks: Tasks,
    workers: Workers,
    clock: Time,
    task_waiters: Arc<Mutex<HashMap<String, TaskWatchSender>>>,
    worker_states: Arc<Mutex<HashMap<String, WorkerState>>>,
}

impl<Tasks, Workers, Time> TaskSchedulerService<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    pub fn new(tasks: Tasks, workers: Workers, clock: Time) -> Self {
        Self {
            tasks,
            workers,
            clock,
            task_waiters: Arc::new(Mutex::new(HashMap::new())),
            worker_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn enqueue(
        &self,
        task: ScheduledTask,
    ) -> BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        Box::pin(async move {
            let result = tasks.enqueue_if_absent(task).await?;
            scheduler.publish_task_update(result.task.clone()).await;
            Ok(result)
        })
    }

    pub fn enqueue_no_result_task(
        &self,
        task: ScheduledTask,
    ) -> BoxFuture<Result<ScheduledTask, TaskSchedulerError>> {
        let scheduler = self.clone();
        Box::pin(async move { scheduler.enqueue(task).await.map(|result| result.task) })
    }

    pub fn now_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds()
    }

    pub fn claim_next_due(
        &self,
        worker_id: &str,
        enabled_task_types: Vec<String>,
        is_leader: bool,
        lease_duration_seconds: i64,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        match self.build_claim_request(
            worker_id,
            enabled_task_types,
            is_leader,
            lease_duration_seconds,
        ) {
            Ok(request) => {
                let tasks = self.tasks.clone();
                let scheduler = self.clone();
                Box::pin(async move {
                    let task = tasks.claim_next_due(request).await?;
                    if let Some(task) = task.clone() {
                        scheduler.publish_task_update(task).await;
                    }
                    Ok(task)
                })
            }
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    pub fn heartbeat_task(
        &self,
        task_id: &str,
        worker_id: &str,
        lease_duration_seconds: i64,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        match self.build_heartbeat_request(task_id, worker_id, lease_duration_seconds) {
            Ok(request) => {
                let tasks = self.tasks.clone();
                let scheduler = self.clone();
                Box::pin(async move {
                    let task = tasks.heartbeat(request).await?;
                    if let Some(task) = task.clone() {
                        scheduler.publish_task_update(task).await;
                    }
                    Ok(task)
                })
            }
            Err(error) => Box::pin(async move { Err(error) }),
        }
    }

    pub fn save_task_checkpoint(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: serde_json::Value,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        let request = self.build_checkpoint_request(task_id, worker_id, checkpoint);
        Box::pin(async move {
            let task = tasks.save_checkpoint(request).await?;
            if let Some(task) = task.clone() {
                scheduler.publish_task_update(task).await;
            }
            Ok(task)
        })
    }

    pub fn complete_task(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: Option<serde_json::Value>,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        let request = self.build_complete_request(task_id, worker_id, checkpoint);
        Box::pin(async move {
            let task = tasks.complete(request).await?;
            scheduler.publish_terminal_task_update(task.clone()).await;
            Ok(task)
        })
    }

    pub fn fail_task(
        &self,
        input: FailTaskInput<'_>,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        let request = self.build_fail_request(input);
        Box::pin(async move {
            let task = tasks.fail(request).await?;
            if task.as_ref().is_some_and(task_is_terminal) {
                scheduler.publish_terminal_task_update(task.clone()).await;
            } else if let Some(task) = task.clone() {
                scheduler.publish_task_update(task).await;
            }
            Ok(task)
        })
    }

    pub fn heartbeat_worker(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            scheduler
                .set_worker_state(worker_id, is_leader, enabled_task_types, active_task_ids)
                .await
        })
    }

    pub fn touch_worker_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            scheduler
                .touch_cached_worker_state(worker_id, is_leader, enabled_task_types)
                .await
        })
    }

    pub fn sweep_timed_out_tasks(
        &self,
        worker_stale_after_seconds: i64,
        limit: usize,
    ) -> BoxFuture<Result<usize, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let workers = self.workers.clone();
        let scheduler = self.clone();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let candidates = tasks
                .list_timeout_candidates(now_epoch_seconds, limit)
                .await?;
            let mut timed_out_count = 0usize;

            for task in candidates {
                if let Some(updated_task) = sweep_timeout_candidate(
                    &tasks,
                    &workers,
                    task,
                    now_epoch_seconds,
                    worker_stale_after_seconds,
                )
                .await?
                {
                    scheduler.publish_task_update(updated_task.clone()).await;
                    if task_is_terminal(&updated_task) {
                        scheduler
                            .publish_terminal_task_update(Some(updated_task))
                            .await;
                    }
                    timed_out_count += 1;
                }
            }

            Ok(timed_out_count)
        })
    }

    pub fn retry_task(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let scheduler = self.clone();
        let request = TaskRetryRequest {
            task_id: task_id.to_string(),
            retried_at_epoch_seconds: self.clock.now_epoch_seconds(),
        };
        Box::pin(async move {
            let task = tasks.retry(request).await?;
            if let Some(task) = task.clone() {
                scheduler.publish_task_update(task).await;
            }
            Ok(task)
        })
    }

    pub fn get_task(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move { tasks.find_by_id(&task_id).await })
    }

    pub fn enqueue_result_task<Handler>(
        &self,
        task: ScheduledTask,
        map_scheduler_error: fn(TaskSchedulerError) -> Handler::Error,
        handler: Handler,
    ) -> BoxFuture<Result<Handler::Output, Handler::Error>>
    where
        Handler: ResultTaskHandler,
    {
        let scheduler = self.clone();
        Box::pin(async move {
            let task = scheduler
                .enqueue(task)
                .await
                .map(|result| result.task)
                .map_err(map_scheduler_error)?;
            let task = scheduler
                .retry_if_terminal(task)
                .await
                .map_err(map_scheduler_error)?;
            scheduler
                .wait_for_result_task(&task.id, map_scheduler_error, handler)
                .await
        })
    }

    pub fn wait_for_result_task<Handler>(
        &self,
        task_id: &str,
        map_scheduler_error: fn(TaskSchedulerError) -> Handler::Error,
        handler: Handler,
    ) -> BoxFuture<Result<Handler::Output, Handler::Error>>
    where
        Handler: ResultTaskHandler,
    {
        let scheduler = self.clone();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let mut watcher = scheduler.subscribe_to_task_updates(&task_id).await;
            let mut current = scheduler
                .get_task(&task_id)
                .await
                .map_err(map_scheduler_error)?;

            loop {
                match current {
                    Some(task) => match task.status {
                        TaskStatus::Completed => {
                            let completed = handler.parse_completed(&task)?;
                            return handler.finish(completed).await;
                        }
                        TaskStatus::Failed => return Err(handler.parse_failed(&task)?),
                        TaskStatus::TimedOut => return Err(handler.task_timed_out(&task_id)),
                        _ => {}
                    },
                    None => return Err(handler.task_disappeared(&task_id)),
                }

                watcher
                    .changed()
                    .await
                    .map_err(|_| handler.task_disappeared(&task_id))?;
                current = watcher.borrow().clone();
            }
        })
    }

    pub fn list_tasks(
        &self,
        filter: TaskListFilter,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move { tasks.list(filter).await })
    }

    fn build_claim_request(
        &self,
        worker_id: &str,
        enabled_task_types: Vec<String>,
        is_leader: bool,
        lease_duration_seconds: i64,
    ) -> Result<TaskClaimRequest, TaskSchedulerError> {
        let now_epoch_seconds = validate_positive_duration(
            lease_duration_seconds,
            "task lease duration must be positive",
        )
        .map(|_| self.clock.now_epoch_seconds())?;
        Ok(TaskClaimRequest {
            worker_id: worker_id.to_string(),
            enabled_task_types,
            is_leader,
            now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        })
    }

    fn build_heartbeat_request(
        &self,
        task_id: &str,
        worker_id: &str,
        lease_duration_seconds: i64,
    ) -> Result<TaskHeartbeatRequest, TaskSchedulerError> {
        let now_epoch_seconds = validate_positive_duration(
            lease_duration_seconds,
            "task lease duration must be positive",
        )
        .map(|_| self.clock.now_epoch_seconds())?;
        Ok(TaskHeartbeatRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            last_heartbeat_at_epoch_seconds: now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        })
    }

    fn build_checkpoint_request(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: serde_json::Value,
    ) -> TaskCheckpointRequest {
        TaskCheckpointRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            checkpoint,
            updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    fn build_complete_request(
        &self,
        task_id: &str,
        worker_id: &str,
        checkpoint: Option<serde_json::Value>,
    ) -> TaskCompleteRequest {
        TaskCompleteRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            checkpoint,
            completed_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    fn build_fail_request(&self, input: FailTaskInput<'_>) -> TaskFailRequest {
        let failed_at_epoch_seconds = self.clock.now_epoch_seconds();
        let retry_at_epoch_seconds = input.retryable.then(|| {
            input
                .retry_strategy
                .next_retry_at(input.attempt_count, failed_at_epoch_seconds)
        });
        TaskFailRequest {
            task_id: input.task_id.to_string(),
            worker_id: input.worker_id.to_string(),
            checkpoint: input.checkpoint,
            error_message: input.error_message,
            failed_at_epoch_seconds,
            retry_at_epoch_seconds: retry_at_epoch_seconds.flatten(),
        }
    }

    fn build_task_worker(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> TaskWorker {
        TaskWorker {
            worker_id: worker_id.to_string(),
            is_leader,
            enabled_task_types,
            active_task_ids,
            last_heartbeat_at_epoch_seconds: self.clock.now_epoch_seconds(),
        }
    }

    async fn set_worker_state(
        &self,
        worker_id: String,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> Result<TaskWorker, TaskSchedulerError> {
        let worker = self.build_task_worker(
            &worker_id,
            is_leader,
            enabled_task_types.clone(),
            active_task_ids.clone(),
        );
        let persisted = self.workers.clone().upsert(worker).await?;
        self.worker_states.lock().await.insert(
            worker_id,
            WorkerState {
                is_leader,
                enabled_task_types,
                active_task_ids,
            },
        );
        Ok(persisted)
    }

    async fn touch_cached_worker_state(
        &self,
        worker_id: String,
        is_leader: bool,
        enabled_task_types: Vec<String>,
    ) -> Result<TaskWorker, TaskSchedulerError> {
        {
            let mut worker_states = self.worker_states.lock().await;
            let state = worker_states
                .entry(worker_id.clone())
                .or_insert_with(|| WorkerState {
                    is_leader,
                    enabled_task_types: enabled_task_types.clone(),
                    active_task_ids: Vec::new(),
                });
            state.is_leader = is_leader;
            state.enabled_task_types = enabled_task_types.clone();
            state.active_task_ids.clear();
        }

        self.workers
            .clone()
            .upsert(self.build_task_worker(&worker_id, is_leader, enabled_task_types, Vec::new()))
            .await
    }

    pub fn add_worker_active_task(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        task_id: &str,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let active_task_ids = {
                let mut worker_states = scheduler.worker_states.lock().await;
                let state = worker_states
                    .entry(worker_id.clone())
                    .or_insert_with(|| WorkerState {
                        is_leader,
                        enabled_task_types: enabled_task_types.clone(),
                        active_task_ids: Vec::new(),
                    });
                state.is_leader = is_leader;
                state.enabled_task_types = enabled_task_types.clone();
                if !state
                    .active_task_ids
                    .iter()
                    .any(|active| active == &task_id)
                {
                    state.active_task_ids.push(task_id.clone());
                }
                state.active_task_ids.clone()
            };

            scheduler
                .workers
                .clone()
                .upsert(scheduler.build_task_worker(
                    &worker_id,
                    is_leader,
                    enabled_task_types,
                    active_task_ids,
                ))
                .await
        })
    }

    pub fn remove_worker_active_task(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        task_id: &str,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let scheduler = self.clone();
        let worker_id = worker_id.to_string();
        let task_id = task_id.to_string();
        Box::pin(async move {
            let active_task_ids = {
                let mut worker_states = scheduler.worker_states.lock().await;
                let state = worker_states
                    .entry(worker_id.clone())
                    .or_insert_with(|| WorkerState {
                        is_leader,
                        enabled_task_types: enabled_task_types.clone(),
                        active_task_ids: Vec::new(),
                    });
                state.is_leader = is_leader;
                state.enabled_task_types = enabled_task_types.clone();
                state.active_task_ids.retain(|active| active != &task_id);
                state.active_task_ids.clone()
            };

            scheduler
                .workers
                .clone()
                .upsert(scheduler.build_task_worker(
                    &worker_id,
                    is_leader,
                    enabled_task_types,
                    active_task_ids,
                ))
                .await
        })
    }

    async fn retry_if_terminal(
        &self,
        task: ScheduledTask,
    ) -> Result<ScheduledTask, TaskSchedulerError> {
        if !task.can_retry_manually() {
            return Ok(task);
        }

        Ok(self.retry_task(&task.id).await?.unwrap_or(task))
    }

    async fn subscribe_to_task_updates(&self, task_id: &str) -> TaskWatchReceiver {
        let mut waiters = self.task_waiters.lock().await;
        waiters
            .entry(task_id.to_string())
            .or_insert_with(|| {
                let (sender, _) = watch::channel(None);
                sender
            })
            .subscribe()
    }

    async fn publish_task_update(&self, task: ScheduledTask) {
        let sender = {
            let mut waiters = self.task_waiters.lock().await;
            waiters
                .entry(task.id.clone())
                .or_insert_with(|| {
                    let (sender, _) = watch::channel(None);
                    sender
                })
                .clone()
        };
        let _ = sender.send(Some(task));
    }

    async fn publish_terminal_task_update(&self, task: Option<ScheduledTask>) {
        let Some(task) = task else {
            return;
        };

        let sender = {
            let mut waiters = self.task_waiters.lock().await;
            waiters.remove(&task.id)
        };

        if let Some(sender) = sender {
            let _ = sender.send(Some(task));
        }
    }
}

fn validate_positive_duration(value: i64, message: &str) -> Result<i64, TaskSchedulerError> {
    if value <= 0 {
        return Err(TaskSchedulerError::Validation(message.to_string()));
    }
    Ok(value)
}

fn task_is_terminal(task: &ScheduledTask) -> bool {
    matches!(
        task.status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::TimedOut
    )
}

async fn sweep_timeout_candidate<Tasks, Workers>(
    tasks: &Tasks,
    workers: &Workers,
    task: ScheduledTask,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
{
    let Some(worker_id) = task.claimed_by.clone() else {
        if task_heartbeat_is_fresh(&task, now_epoch_seconds, worker_stale_after_seconds) {
            return Ok(None);
        }

        return recover_task(tasks, &task, None, now_epoch_seconds).await;
    };

    let worker = workers.find_by_worker_id(&worker_id).await?;
    let worker_is_fresh = worker_is_fresh(
        worker.as_ref(),
        now_epoch_seconds,
        worker_stale_after_seconds,
    );
    let worker_reports_task_active = worker_reports_task_active(worker.as_ref(), &task.id);

    if worker_is_fresh && worker_reports_task_active {
        return Ok(None);
    }

    if worker_is_fresh && !worker_reports_task_active {
        return recover_task(tasks, &task, Some(worker_id), now_epoch_seconds).await;
    }

    if task_heartbeat_is_fresh(&task, now_epoch_seconds, worker_stale_after_seconds) {
        return Ok(None);
    }

    if worker.is_none() {
        return recover_task(tasks, &task, Some(worker_id), now_epoch_seconds).await;
    }

    mark_task_timed_out(tasks, &task, worker_id, now_epoch_seconds).await
}

fn task_heartbeat_is_fresh(
    task: &ScheduledTask,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> bool {
    task.last_heartbeat_at_epoch_seconds
        .is_some_and(|last_heartbeat_at_epoch_seconds| {
            last_heartbeat_at_epoch_seconds >= now_epoch_seconds - worker_stale_after_seconds
        })
}

fn worker_is_fresh(
    worker: Option<&TaskWorker>,
    now_epoch_seconds: i64,
    worker_stale_after_seconds: i64,
) -> bool {
    worker.is_some_and(|worker| {
        worker.last_heartbeat_at_epoch_seconds >= now_epoch_seconds - worker_stale_after_seconds
    })
}

fn worker_reports_task_active(worker: Option<&TaskWorker>, task_id: &str) -> bool {
    worker.is_some_and(|worker| {
        worker
            .active_task_ids
            .iter()
            .any(|active| active == task_id)
    })
}

async fn recover_task<Tasks>(
    tasks: &Tasks,
    task: &ScheduledTask,
    expected_claimed_by: Option<String>,
    recovered_at_epoch_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
{
    let changed = tasks
        .recover(TaskRecoverRequest {
            task_id: task.id.clone(),
            expected_claimed_by,
            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
            recovered_at_epoch_seconds,
        })
        .await?;

    if !changed {
        return Ok(None);
    }

    tasks.find_by_id(&task.id).await
}

async fn mark_task_timed_out<Tasks>(
    tasks: &Tasks,
    task: &ScheduledTask,
    worker_id: String,
    timed_out_at_epoch_seconds: i64,
) -> Result<Option<ScheduledTask>, TaskSchedulerError>
where
    Tasks: TaskRepository,
{
    let changed = tasks
        .mark_timed_out(TaskMarkTimedOutRequest {
            task_id: task.id.clone(),
            expected_claimed_by: Some(worker_id),
            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
            timed_out_at_epoch_seconds,
        })
        .await?;

    if !changed {
        return Ok(None);
    }

    tasks.find_by_id(&task.id).await
}
