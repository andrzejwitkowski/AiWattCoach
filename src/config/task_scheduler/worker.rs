use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::future::join_all;
use tokio::sync::{watch, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::MissedTickBehavior;
use tracing::warn;

use crate::{
    domain::{
        identity::Clock,
        task_scheduler::{
            FailTaskInput, ScheduledTask, SharedTaskHandler, TaskRepository, TaskRunOutcome,
            TaskSchedulerError, TaskSchedulerService, TaskWorkerConfig, TaskWorkerRepository,
        },
    },
    BackgroundTaskHandle,
};

struct AbortOnDropHandle<T>(Option<tokio::task::JoinHandle<T>>);

impl<T> AbortOnDropHandle<T> {
    fn new(handle: tokio::task::JoinHandle<T>) -> Self {
        Self(Some(handle))
    }

    fn abort(&self) {
        if let Some(handle) = &self.0 {
            handle.abort();
        }
    }

    async fn join(mut self) -> Result<T, tokio::task::JoinError> {
        self.0
            .as_mut()
            .expect("join handle should be present")
            .await
    }
}

impl<T> Drop for AbortOnDropHandle<T> {
    fn drop(&mut self) {
        if let Some(handle) = &self.0 {
            handle.abort();
        }
    }
}

struct TaskHandlerRegistry {
    enabled_task_types: Vec<String>,
    handlers_by_type: HashMap<String, SharedTaskHandler>,
}

#[derive(Default)]
struct WorkerRuntimeState {
    active_task_ids: Mutex<Vec<String>>,
}

struct ClaimedTask<Tasks, Workers, Time>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    is_leader: bool,
    enabled_task_types: Vec<String>,
    lease_duration_seconds: i64,
    heartbeat_interval: Duration,
    task: ScheduledTask,
    handler: SharedTaskHandler,
    runtime_state: Arc<WorkerRuntimeState>,
    _permit: OwnedSemaphorePermit,
}

pub fn spawn_task_worker<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    config: TaskWorkerConfig,
    handlers: Vec<SharedTaskHandler>,
) -> Result<BackgroundTaskHandle, TaskSchedulerError>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    validate_task_worker_config(&config)?;
    let registry = build_task_handler_registry(handlers)?;
    let concurrency_limit = config.max_concurrency.max(1);
    let runtime_state = Arc::new(WorkerRuntimeState::default());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let join_handle = tokio::spawn(async move {
        run_task_worker_loop(
            scheduler,
            worker_id,
            config,
            registry,
            Arc::new(Semaphore::new(concurrency_limit)),
            runtime_state,
            shutdown_rx,
        )
        .await;
    });

    Ok(BackgroundTaskHandle::new(
        "task-worker",
        shutdown_tx,
        join_handle,
    ))
}

fn validate_task_worker_config(config: &TaskWorkerConfig) -> Result<(), TaskSchedulerError> {
    if config.lease_duration_seconds <= 0 {
        return Err(TaskSchedulerError::Validation(
            "task worker lease_duration_seconds must be positive".to_string(),
        ));
    }

    if config.heartbeat_interval.is_zero() {
        return Err(TaskSchedulerError::Validation(
            "task worker heartbeat_interval must be positive".to_string(),
        ));
    }

    if config.idle_poll_interval.is_zero() {
        return Err(TaskSchedulerError::Validation(
            "task worker idle_poll_interval must be positive".to_string(),
        ));
    }

    Ok(())
}

fn build_task_handler_registry(
    handlers: Vec<SharedTaskHandler>,
) -> Result<TaskHandlerRegistry, TaskSchedulerError> {
    let mut enabled_task_types = Vec::with_capacity(handlers.len());
    let mut handlers_by_type = HashMap::with_capacity(handlers.len());

    for handler in handlers {
        let task_type = handler.task_type().to_string();
        let replaced = handlers_by_type.insert(task_type.clone(), handler);
        if replaced.is_some() {
            return Err(TaskSchedulerError::Conflict(format!(
                "duplicate task handler registered for {task_type}"
            )));
        }
        enabled_task_types.push(task_type);
    }

    Ok(TaskHandlerRegistry {
        enabled_task_types,
        handlers_by_type,
    })
}

async fn run_task_worker_loop<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    config: TaskWorkerConfig,
    registry: TaskHandlerRegistry,
    semaphore: Arc<Semaphore>,
    runtime_state: Arc<WorkerRuntimeState>,
    mut shutdown: watch::Receiver<bool>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let mut idle_ticker = tokio::time::interval(config.idle_poll_interval);
    idle_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let active_tasks = Arc::new(Mutex::new(Vec::new()));

    persist_worker_state_with_retry(
        &scheduler,
        &worker_id,
        &config,
        &registry,
        &runtime_state,
        &mut shutdown,
    )
    .await;

    loop {
        if *shutdown.borrow() {
            break;
        }

        let permit = tokio::select! {
            _ = shutdown.changed() => {
                break;
            }
            permit = semaphore.clone().acquire_owned() => {
                permit.expect("task worker semaphore should stay alive")
            }
        };

        let Some(claimed_task) = claim_next_task(
            &scheduler,
            &worker_id,
            &config,
            &registry,
            &mut idle_ticker,
            permit,
            runtime_state.clone(),
            &mut shutdown,
        )
        .await
        else {
            continue;
        };

        let task_join = tokio::spawn(run_claimed_task(claimed_task));
        active_tasks.lock().await.push(task_join);
        reap_finished_background_tasks(&active_tasks).await;
    }

    abort_active_background_tasks(active_tasks).await;
}

#[allow(clippy::too_many_arguments)]
async fn claim_next_task<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    idle_ticker: &mut tokio::time::Interval,
    permit: OwnedSemaphorePermit,
    runtime_state: Arc<WorkerRuntimeState>,
    shutdown: &mut watch::Receiver<bool>,
) -> Option<ClaimedTask<Tasks, Workers, Time>>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    match scheduler
        .claim_next_due(
            worker_id,
            registry.enabled_task_types.clone(),
            config.is_leader,
            config.lease_duration_seconds,
        )
        .await
    {
        Ok(Some(task)) => {
            let Some(handler) = registry.handlers_by_type.get(&task.task_type).cloned() else {
                fail_unhandled_task(scheduler, worker_id, &task).await;
                return None;
            };

            persist_worker_task_claim(
                scheduler,
                worker_id,
                config.is_leader,
                registry.enabled_task_types.clone(),
                &task.id,
                &runtime_state,
            )
            .await;

            Some(ClaimedTask {
                scheduler: scheduler.clone(),
                worker_id: worker_id.to_string(),
                is_leader: config.is_leader,
                enabled_task_types: registry.enabled_task_types.clone(),
                lease_duration_seconds: config.lease_duration_seconds,
                heartbeat_interval: config.heartbeat_interval,
                task,
                handler,
                runtime_state,
                _permit: permit,
            })
        }
        Ok(None) => {
            drop(permit);
            wait_for_next_claim_attempt(
                scheduler,
                worker_id,
                config,
                registry,
                idle_ticker,
                &runtime_state,
                shutdown,
            )
            .await;
            None
        }
        Err(error) => {
            drop(permit);
            warn!(
                worker_id = %worker_id,
                enabled_task_types = ?registry.enabled_task_types,
                %error,
                "task worker failed to claim task"
            );
            wait_for_next_claim_attempt(
                scheduler,
                worker_id,
                config,
                registry,
                idle_ticker,
                &runtime_state,
                shutdown,
            )
            .await;
            None
        }
    }
}

async fn run_claimed_task<Tasks, Workers, Time>(claimed_task: ClaimedTask<Tasks, Workers, Time>)
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let ClaimedTask {
        scheduler,
        worker_id,
        is_leader,
        enabled_task_types,
        lease_duration_seconds,
        heartbeat_interval,
        task,
        handler,
        runtime_state,
        _permit,
    } = claimed_task;

    let task_id = task.id.clone();

    let heartbeat = AbortOnDropHandle::new(spawn_task_heartbeat(
        scheduler.clone(),
        worker_id.clone(),
        is_leader,
        lease_duration_seconds,
        heartbeat_interval,
        enabled_task_types.clone(),
        task_id.clone(),
        runtime_state.clone(),
    ));

    tracing::info!(
        task_id = %task_id,
        task_type = %task.task_type,
        worker_id = %worker_id,
        "running scheduled task"
    );

    let run_result = run_task_handler(handler, task.clone()).await;
    heartbeat.abort();
    if let Err(error) = heartbeat.join().await {
        if !error.is_cancelled() {
            warn!(task_id = %task_id, worker_id = %worker_id, %error, "task heartbeat exited unexpectedly");
        }
    }

    let outcome = match run_result {
        TaskHandlerRunResult::Completed(outcome) => outcome,
        TaskHandlerRunResult::Panicked(join_error) => {
            warn!(task_id = %task_id, worker_id = %worker_id, %join_error, "scheduled task handler panicked");
            TaskRunOutcome::Failed {
                checkpoint: None,
                error_message: "scheduled task handler panicked".to_string(),
                retryable: true,
                retry_delay_seconds: None,
            }
        }
    };

    persist_task_outcome(&scheduler, &worker_id, &task, outcome).await;

    persist_worker_task_release(
        &scheduler,
        &worker_id,
        is_leader,
        enabled_task_types,
        &task_id,
        &runtime_state,
    )
    .await;
}

enum TaskHandlerRunResult {
    Completed(TaskRunOutcome),
    Panicked(tokio::task::JoinError),
}

async fn run_task_handler(handler: SharedTaskHandler, task: ScheduledTask) -> TaskHandlerRunResult {
    let task = AbortOnDropHandle::new(tokio::spawn(async move { handler.run(task).await }));
    match task.join().await {
        Ok(outcome) => TaskHandlerRunResult::Completed(outcome),
        Err(join_error) => TaskHandlerRunResult::Panicked(join_error),
    }
}

async fn wait_for_next_claim_attempt<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    idle_ticker: &mut tokio::time::Interval,
    runtime_state: &Arc<WorkerRuntimeState>,
    shutdown: &mut watch::Receiver<bool>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    persist_worker_state(scheduler, worker_id, config, registry, runtime_state).await;
    tokio::select! {
        _ = shutdown.changed() => {}
        _ = idle_ticker.tick() => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_task_heartbeat<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    is_leader: bool,
    lease_duration_seconds: i64,
    heartbeat_interval: Duration,
    enabled_task_types: Vec<String>,
    task_id: String,
    runtime_state: Arc<WorkerRuntimeState>,
) -> tokio::task::JoinHandle<()>
where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(heartbeat_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            match scheduler
                .heartbeat_task(&task_id, &worker_id, lease_duration_seconds)
                .await
            {
                Ok(Some(_)) => {
                    persist_worker_task_claim(
                        &scheduler,
                        &worker_id,
                        is_leader,
                        enabled_task_types.clone(),
                        &task_id,
                        &runtime_state,
                    )
                    .await;
                }
                Ok(None) => break,
                Err(error) => {
                    warn!(
                        task_id = %task_id,
                        worker_id = %worker_id,
                        %error,
                        "task heartbeat failed; keeping task runner alive for retry"
                    );
                }
            }
        }
    })
}

async fn reap_finished_background_tasks(active_tasks: &Mutex<Vec<tokio::task::JoinHandle<()>>>) {
    let finished_tasks = {
        let mut active_tasks = active_tasks.lock().await;
        let mut finished = Vec::new();
        let mut index = 0;
        while index < active_tasks.len() {
            if active_tasks[index].is_finished() {
                finished.push(active_tasks.swap_remove(index));
            } else {
                index += 1;
            }
        }
        finished
    };

    for task in finished_tasks {
        if let Err(error) = task.await {
            if error.is_cancelled() {
                continue;
            }

            warn!(%error, "claimed task runner exited unexpectedly");
        }
    }
}

async fn abort_active_background_tasks(active_tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>) {
    let active_tasks = {
        let mut active_tasks = active_tasks.lock().await;
        std::mem::take(&mut *active_tasks)
    };

    for task in &active_tasks {
        task.abort();
    }

    for result in join_all(active_tasks).await {
        if let Err(error) = result {
            if error.is_cancelled() {
                continue;
            }

            warn!(%error, "claimed task runner exited unexpectedly during shutdown");
        }
    }
}

async fn fail_unhandled_task<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    task: &ScheduledTask,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    warn!(
        task_id = %task.id,
        task_type = %task.task_type,
        "no task handler registered for claimed task type"
    );

    if let Err(error) = scheduler
        .fail_task(FailTaskInput {
            task_id: &task.id,
            worker_id,
            checkpoint: None,
            error_message: format!(
                "no task handler registered for task type {}",
                task.task_type
            ),
            retryable: false,
            retry_delay_seconds: None,
            retry_strategy: &task.retry_strategy,
            attempt_count: task.attempt_count,
        })
        .await
    {
        warn!(
            task_id = %task.id,
            task_type = %task.task_type,
            error = %error,
            "failed to persist unhandled task error"
        );
    }
}

async fn persist_task_outcome<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    task: &ScheduledTask,
    outcome: TaskRunOutcome,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    match outcome {
        TaskRunOutcome::Completed { checkpoint } => {
            if let Err(error) = scheduler
                .complete_task(&task.id, worker_id, checkpoint)
                .await
            {
                warn!(
                    task_id = %task.id,
                    task_type = %task.task_type,
                    %error,
                    "failed to mark scheduled task completed"
                );
            }
        }
        TaskRunOutcome::Failed {
            checkpoint,
            error_message,
            retryable,
            retry_delay_seconds,
        } => {
            if let Err(error) = scheduler
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
                .await
            {
                warn!(
                    task_id = %task.id,
                    task_type = %task.task_type,
                    error = %error,
                    "failed to persist scheduled task error"
                );
            }
        }
    }
}

async fn persist_worker_state<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    runtime_state: &Arc<WorkerRuntimeState>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let active_task_ids = runtime_state.active_task_ids.lock().await.clone();
    persist_worker_heartbeat(
        scheduler,
        worker_id,
        config.is_leader,
        registry.enabled_task_types.clone(),
        active_task_ids,
    )
    .await;
}

async fn persist_worker_state_with_retry<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    runtime_state: &Arc<WorkerRuntimeState>,
    shutdown: &mut watch::Receiver<bool>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let mut backoff_seconds: u64 = 1;
    let max_backoff_seconds: u64 = 30;

    loop {
        let active_task_ids = runtime_state.active_task_ids.lock().await.clone();
        match scheduler
            .heartbeat_worker(
                worker_id,
                config.is_leader,
                registry.enabled_task_types.clone(),
                active_task_ids,
            )
            .await
        {
            Ok(worker) => {
                tracing::info!(
                    worker_id = %worker.worker_id,
                    is_leader = worker.is_leader,
                    enabled_task_types = ?worker.enabled_task_types,
                    "task worker startup state persisted"
                );
                break;
            }
            Err(error) => {
                tracing::warn!(
                    worker_id = %worker_id,
                    backoff_seconds,
                    %error,
                    "failed to persist task worker startup state; retrying"
                );
            }
        }

        tokio::select! {
            _ = shutdown.changed() => {
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(backoff_seconds)) => {}
        }

        backoff_seconds = (backoff_seconds.saturating_mul(2)).min(max_backoff_seconds);
    }
}

async fn persist_worker_heartbeat<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    is_leader: bool,
    enabled_task_types: Vec<String>,
    active_task_ids: Vec<String>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let active_task_count = active_task_ids.len();
    if let Err(error) = scheduler
        .heartbeat_worker(
            worker_id,
            is_leader,
            enabled_task_types.clone(),
            active_task_ids,
        )
        .await
    {
        warn!(
            worker_id = %worker_id,
            enabled_task_types = ?enabled_task_types,
            %error,
            "failed to persist task worker heartbeat"
        );
    } else {
        tracing::debug!(
            worker_id = %worker_id,
            is_leader,
            active_task_count,
            "task worker state persisted"
        );
    }
}

async fn persist_worker_task_claim<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    is_leader: bool,
    enabled_task_types: Vec<String>,
    task_id: &str,
    runtime_state: &Arc<WorkerRuntimeState>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    {
        let mut active_task_ids = runtime_state.active_task_ids.lock().await;
        if !active_task_ids.iter().any(|active| active == task_id) {
            active_task_ids.push(task_id.to_string());
        }
    }
    if let Err(error) = scheduler
        .add_worker_active_task(worker_id, is_leader, enabled_task_types.clone(), task_id)
        .await
    {
        warn!(
            worker_id = %worker_id,
            task_id = %task_id,
            enabled_task_types = ?enabled_task_types,
            %error,
            "failed to persist claimed task in worker state"
        );
    }
}

async fn persist_worker_task_release<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    is_leader: bool,
    enabled_task_types: Vec<String>,
    task_id: &str,
    runtime_state: &Arc<WorkerRuntimeState>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    {
        let mut active_task_ids = runtime_state.active_task_ids.lock().await;
        active_task_ids.retain(|active| active != task_id);
    }
    if let Err(error) = scheduler
        .remove_worker_active_task(worker_id, is_leader, enabled_task_types.clone(), task_id)
        .await
    {
        warn!(
            worker_id = %worker_id,
            task_id = %task_id,
            enabled_task_types = ?enabled_task_types,
            %error,
            "failed to remove task from worker state"
        );
    }
}
