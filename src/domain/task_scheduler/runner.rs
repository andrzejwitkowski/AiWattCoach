use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::MissedTickBehavior;
use tracing::{info, warn};

use crate::domain::identity::Clock;

use super::{
    BoxFuture, FailTaskInput, ScheduledTask, TaskRepository, TaskSchedulerService,
    TaskWorkerRepository,
};

#[derive(Clone, Debug)]
pub struct TaskWorkerConfig {
    pub is_leader: bool,
    pub lease_duration_seconds: i64,
    pub heartbeat_interval: Duration,
    pub idle_poll_interval: Duration,
    pub max_concurrency: usize,
}

#[derive(Debug)]
pub enum TaskRunOutcome {
    Completed {
        checkpoint: Option<serde_json::Value>,
    },
    Failed {
        checkpoint: Option<serde_json::Value>,
        error_message: String,
        retryable: bool,
    },
}

pub trait TaskHandler: Send + Sync + 'static {
    fn task_type(&self) -> &'static str;

    fn run(&self, task: ScheduledTask) -> BoxFuture<TaskRunOutcome>;
}

pub type SharedTaskHandler = Arc<dyn TaskHandler>;

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
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let registry = build_task_handler_registry(handlers);
    let concurrency_limit = config.max_concurrency.max(1);
    let runtime_state = Arc::new(WorkerRuntimeState::default());
    tokio::spawn(async move {
        run_task_worker_loop(
            scheduler,
            worker_id,
            config,
            registry,
            Arc::new(Semaphore::new(concurrency_limit)),
            runtime_state,
        )
        .await;
    });
}

fn build_task_handler_registry(handlers: Vec<SharedTaskHandler>) -> TaskHandlerRegistry {
    let mut enabled_task_types = Vec::with_capacity(handlers.len());
    let mut handlers_by_type = HashMap::with_capacity(handlers.len());

    for handler in handlers {
        let task_type = handler.task_type().to_string();
        let replaced = handlers_by_type.insert(task_type.clone(), handler);
        assert!(
            replaced.is_none(),
            "duplicate task handler registered for {task_type}"
        );
        enabled_task_types.push(task_type);
    }

    TaskHandlerRegistry {
        enabled_task_types,
        handlers_by_type,
    }
}

async fn run_task_worker_loop<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
    config: TaskWorkerConfig,
    registry: TaskHandlerRegistry,
    semaphore: Arc<Semaphore>,
    runtime_state: Arc<WorkerRuntimeState>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    let mut idle_ticker = tokio::time::interval(config.idle_poll_interval);
    idle_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    persist_worker_state(&scheduler, &worker_id, &config, &registry, &runtime_state).await;

    loop {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("task worker semaphore should stay alive");

        let Some(claimed_task) = claim_next_task(
            &scheduler,
            &worker_id,
            &config,
            &registry,
            &mut idle_ticker,
            permit,
            runtime_state.clone(),
        )
        .await
        else {
            continue;
        };

        tokio::spawn(run_claimed_task(claimed_task));
    }
}

async fn claim_next_task<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    idle_ticker: &mut tokio::time::Interval,
    permit: OwnedSemaphorePermit,
    runtime_state: Arc<WorkerRuntimeState>,
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
        enabled_task_types,
        lease_duration_seconds,
        heartbeat_interval,
        task,
        handler,
        runtime_state,
        _permit,
    } = claimed_task;

    let heartbeat = spawn_task_heartbeat(
        scheduler.clone(),
        worker_id.clone(),
        lease_duration_seconds,
        heartbeat_interval,
        enabled_task_types.clone(),
        task.id.clone(),
        runtime_state.clone(),
    );

    info!(
        task_id = %task.id,
        task_type = %task.task_type,
        worker_id = %worker_id,
        "running scheduled task"
    );

    let outcome = handler.run(task.clone()).await;
    heartbeat.abort();

    persist_task_outcome(&scheduler, &worker_id, &task, outcome).await;
    persist_worker_task_release(
        &scheduler,
        &worker_id,
        false,
        enabled_task_types,
        &task.id,
        &runtime_state,
    )
    .await;
}

async fn wait_for_next_claim_attempt<Tasks, Workers, Time>(
    scheduler: &TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: &str,
    config: &TaskWorkerConfig,
    registry: &TaskHandlerRegistry,
    idle_ticker: &mut tokio::time::Interval,
    runtime_state: &Arc<WorkerRuntimeState>,
) where
    Tasks: TaskRepository,
    Workers: TaskWorkerRepository,
    Time: Clock,
{
    persist_worker_state(scheduler, worker_id, config, registry, runtime_state).await;
    idle_ticker.tick().await;
}

fn spawn_task_heartbeat<Tasks, Workers, Time>(
    scheduler: TaskSchedulerService<Tasks, Workers, Time>,
    worker_id: String,
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

            let task_is_still_owned = scheduler
                .heartbeat_task(&task_id, &worker_id, lease_duration_seconds)
                .await
                .ok()
                .flatten()
                .is_some();
            if !task_is_still_owned {
                break;
            }

            persist_worker_task_claim(
                &scheduler,
                &worker_id,
                false,
                enabled_task_types.clone(),
                &task_id,
                &runtime_state,
            )
            .await;
        }
    })
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
        } => {
            if let Err(error) = scheduler
                .fail_task(FailTaskInput {
                    task_id: &task.id,
                    worker_id,
                    checkpoint,
                    error_message,
                    retryable,
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
