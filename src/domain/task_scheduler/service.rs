use crate::domain::identity::Clock;

use super::{
    BoxFuture, ScheduledTask, TaskClaimRequest, TaskEnqueueResult, TaskHeartbeatRequest,
    TaskListFilter, TaskMarkTimedOutRequest, TaskRecoverRequest, TaskRepository, TaskRetryRequest,
    TaskSchedulerError, TaskWorker, TaskWorkerRepository,
};

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
        }
    }

    pub fn enqueue(
        &self,
        task: ScheduledTask,
    ) -> BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move { tasks.enqueue_if_absent(task).await })
    }

    pub fn claim_next_due(
        &self,
        worker_id: &str,
        enabled_task_types: Vec<String>,
        is_leader: bool,
        lease_duration_seconds: i64,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        if lease_duration_seconds <= 0 {
            return Box::pin(async {
                Err(TaskSchedulerError::Validation(
                    "task lease duration must be positive".to_string(),
                ))
            });
        }

        let tasks = self.tasks.clone();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        let request = TaskClaimRequest {
            worker_id: worker_id.to_string(),
            enabled_task_types,
            is_leader,
            now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        };
        Box::pin(async move { tasks.claim_next_due(request).await })
    }

    pub fn heartbeat_task(
        &self,
        task_id: &str,
        worker_id: &str,
        lease_duration_seconds: i64,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        if lease_duration_seconds <= 0 {
            return Box::pin(async {
                Err(TaskSchedulerError::Validation(
                    "task lease duration must be positive".to_string(),
                ))
            });
        }

        let tasks = self.tasks.clone();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        let request = TaskHeartbeatRequest {
            task_id: task_id.to_string(),
            worker_id: worker_id.to_string(),
            last_heartbeat_at_epoch_seconds: now_epoch_seconds,
            lease_expires_at_epoch_seconds: now_epoch_seconds + lease_duration_seconds,
        };
        Box::pin(async move { tasks.heartbeat(request).await })
    }

    pub fn heartbeat_worker(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        active_task_ids: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        let worker = TaskWorker {
            worker_id: worker_id.to_string(),
            is_leader,
            enabled_task_types,
            active_task_ids,
            last_heartbeat_at_epoch_seconds: self.clock.now_epoch_seconds(),
        };
        Box::pin(async move { workers.upsert(worker).await })
    }

    pub fn touch_worker_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        let worker_id = worker_id.to_string();
        let last_heartbeat_at_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            workers
                .touch_heartbeat(
                    &worker_id,
                    is_leader,
                    enabled_task_types,
                    last_heartbeat_at_epoch_seconds,
                )
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
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let candidates = tasks
                .list_timeout_candidates(now_epoch_seconds, limit)
                .await?;
            let mut timed_out_count = 0usize;

            for task in candidates {
                let task_heartbeat_is_fresh = task.last_heartbeat_at_epoch_seconds.is_some_and(
                    |last_heartbeat_at_epoch_seconds| {
                        last_heartbeat_at_epoch_seconds
                            >= now_epoch_seconds - worker_stale_after_seconds
                    },
                );
                if task_heartbeat_is_fresh {
                    continue;
                }

                let Some(worker_id) = task.claimed_by.clone() else {
                    let changed = tasks
                        .recover(TaskRecoverRequest {
                            task_id: task.id.clone(),
                            expected_claimed_by: None,
                            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
                            recovered_at_epoch_seconds: now_epoch_seconds,
                        })
                        .await?;
                    if changed {
                        timed_out_count += 1;
                    }
                    continue;
                };

                let worker = workers.find_by_worker_id(&worker_id).await?;
                if worker.is_none() {
                    let changed = tasks
                        .recover(TaskRecoverRequest {
                            task_id: task.id.clone(),
                            expected_claimed_by: Some(worker_id),
                            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
                            recovered_at_epoch_seconds: now_epoch_seconds,
                        })
                        .await?;
                    if changed {
                        timed_out_count += 1;
                    }
                    continue;
                }

                let worker_is_fresh = worker.as_ref().is_some_and(|worker| {
                    worker.last_heartbeat_at_epoch_seconds
                        >= now_epoch_seconds - worker_stale_after_seconds
                });
                let worker_reports_task_active = worker.as_ref().is_some_and(|worker| {
                    worker
                        .active_task_ids
                        .iter()
                        .any(|task_id| task_id == &task.id)
                });

                if worker_is_fresh && worker_reports_task_active {
                    continue;
                }

                if worker_is_fresh && !worker_reports_task_active {
                    let changed = tasks
                        .recover(TaskRecoverRequest {
                            task_id: task.id.clone(),
                            expected_claimed_by: Some(worker_id),
                            expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
                            recovered_at_epoch_seconds: now_epoch_seconds,
                        })
                        .await?;
                    if changed {
                        timed_out_count += 1;
                    }
                    continue;
                }

                let changed = tasks
                    .mark_timed_out(TaskMarkTimedOutRequest {
                        task_id: task.id.clone(),
                        expected_claimed_by: Some(worker_id),
                        expected_updated_at_epoch_seconds: task.updated_at_epoch_seconds,
                        timed_out_at_epoch_seconds: now_epoch_seconds,
                    })
                    .await?;
                if changed {
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
        let request = TaskRetryRequest {
            task_id: task_id.to_string(),
            retried_at_epoch_seconds: self.clock.now_epoch_seconds(),
        };
        Box::pin(async move { tasks.retry(request).await })
    }

    pub fn get_task(
        &self,
        task_id: &str,
    ) -> BoxFuture<Result<Option<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        let task_id = task_id.to_string();
        Box::pin(async move { tasks.find_by_id(&task_id).await })
    }

    pub fn list_tasks(
        &self,
        filter: TaskListFilter,
    ) -> BoxFuture<Result<Vec<ScheduledTask>, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move { tasks.list(filter).await })
    }
}
