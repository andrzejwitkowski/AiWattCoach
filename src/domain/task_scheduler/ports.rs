use std::{future::Future, pin::Pin};

use super::{
    ScheduledTask, TaskCheckpointRequest, TaskClaimRequest, TaskCompleteRequest, TaskEnqueueResult,
    TaskFailRequest, TaskHeartbeatRequest, TaskListFilter, TaskMarkTimedOutRequest,
    TaskRecoverRequest, TaskRetryRequest, TaskSchedulerError, TaskWorker,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait TaskRepository: Clone + Send + Sync + 'static {
    fn enqueue_if_absent(
        &self,
        task: ScheduledTask,
    ) -> BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>>;

    fn claim_next_due(
        &self,
        request: TaskClaimRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn heartbeat(
        &self,
        request: TaskHeartbeatRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn save_checkpoint(
        &self,
        request: TaskCheckpointRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn complete(
        &self,
        request: TaskCompleteRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn fail(
        &self,
        request: TaskFailRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn list_timeout_candidates(
        &self,
        now_epoch_seconds: i64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>;

    fn mark_timed_out(
        &self,
        request: TaskMarkTimedOutRequest,
    ) -> BoxFuture<Result<bool, TaskSchedulerError>>;

    fn recover(&self, request: TaskRecoverRequest) -> BoxFuture<Result<bool, TaskSchedulerError>>;

    fn retry(
        &self,
        request: TaskRetryRequest,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn find_by_id(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>>;

    fn list(
        &self,
        filter: TaskListFilter,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>>;
}

pub trait TaskWorkerRepository: Clone + Send + Sync + 'static {
    fn upsert(&self, worker: TaskWorker) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>>;

    fn touch_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        last_heartbeat_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>>;

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>>;
}
