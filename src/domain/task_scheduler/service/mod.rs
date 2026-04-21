mod requests;
mod result;
mod timeout;
mod updates;
mod worker_state;

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
    pub retry_delay_seconds: Option<i64>,
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
pub(super) struct WorkerState {
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
    pub(super) tasks: Tasks,
    pub(super) workers: Workers,
    pub(super) clock: Time,
    pub(super) task_waiters: Arc<Mutex<HashMap<String, TaskWatchSender>>>,
    pub(super) worker_states: Arc<Mutex<HashMap<String, WorkerState>>>,
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

    pub fn list_tasks(
        &self,
        filter: TaskListFilter,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move { tasks.list(filter).await })
    }

    pub fn get_task(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move { tasks.find_by_id(&task_id).await })
    }

    #[cfg(test)]
    pub(crate) async fn test_waiter_count(&self) -> usize {
        self.task_waiters.lock().await.len()
    }

    #[cfg(test)]
    pub(crate) async fn test_worker_state_count(&self) -> usize {
        self.worker_states.lock().await.len()
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
