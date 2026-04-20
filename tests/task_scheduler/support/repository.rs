use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::task_scheduler::{
    ScheduledTask, TaskCheckpointRequest, TaskClaimRequest, TaskCompleteRequest, TaskEnqueueResult,
    TaskFailRequest, TaskHeartbeatRequest, TaskListFilter, TaskMarkTimedOutRequest,
    TaskRecoverRequest, TaskRepository, TaskRetryRequest, TaskSchedulerError, TaskStatus,
    TaskWorker, TaskWorkerRepository,
};

#[derive(Clone, Default)]
pub struct InMemoryTaskRepository {
    tasks: Arc<Mutex<HashMap<String, ScheduledTask>>>,
}

impl TaskRepository for InMemoryTaskRepository {
    fn enqueue_if_absent(
        &self,
        task: ScheduledTask,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<Result<TaskEnqueueResult, TaskSchedulerError>>
    {
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
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let mut candidate_id = tasks
                .values()
                .filter(|task| {
                    matches!(task.status, TaskStatus::Queued | TaskStatus::RetryScheduled)
                })
                .filter(|task| task.next_attempt_at_epoch_seconds <= request.now_epoch_seconds)
                .filter(|task| {
                    request
                        .enabled_task_types
                        .iter()
                        .any(|task_type| task_type == &task.task_type)
                })
                .filter(|task| request.is_leader || !task.leader_only)
                .map(|task| {
                    (
                        task.id.clone(),
                        task.next_attempt_at_epoch_seconds,
                        task.created_at_epoch_seconds,
                    )
                })
                .collect::<Vec<_>>();
            candidate_id.sort_by(|left, right| left.1.cmp(&right.1).then(left.2.cmp(&right.2)));
            let Some((task_id, _, _)) = candidate_id.into_iter().next() else {
                return Ok(None);
            };
            let task = tasks
                .get_mut(&task_id)
                .expect("task candidate should still exist in memory repo");
            task.status = TaskStatus::Running;
            task.claimed_by = Some(request.worker_id);
            task.lease_expires_at_epoch_seconds = Some(request.lease_expires_at_epoch_seconds);
            task.last_heartbeat_at_epoch_seconds = Some(request.now_epoch_seconds);
            task.started_at_epoch_seconds = Some(request.now_epoch_seconds);
            task.finished_at_epoch_seconds = None;
            task.timed_out_at_epoch_seconds = None;
            task.updated_at_epoch_seconds = request.now_epoch_seconds;
            task.attempt_count = task.attempt_count.saturating_add(1);
            Ok(Some(task.clone()))
        })
    }

    fn heartbeat(
        &self,
        request: TaskHeartbeatRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.status != TaskStatus::Running
                || task.claimed_by.as_deref() != Some(request.worker_id.as_str())
            {
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
        request: TaskCheckpointRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.status != TaskStatus::Running
                || task.claimed_by.as_deref() != Some(request.worker_id.as_str())
            {
                return Ok(None);
            }
            task.checkpoint = Some(request.checkpoint);
            task.updated_at_epoch_seconds = request.updated_at_epoch_seconds;
            Ok(Some(task.clone()))
        })
    }

    fn complete(
        &self,
        request: TaskCompleteRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.status != TaskStatus::Running
                || task.claimed_by.as_deref() != Some(request.worker_id.as_str())
            {
                return Ok(None);
            }
            if let Some(checkpoint) = request.checkpoint {
                task.checkpoint = Some(checkpoint);
            }
            task.status = TaskStatus::Completed;
            task.updated_at_epoch_seconds = request.completed_at_epoch_seconds;
            task.finished_at_epoch_seconds = Some(request.completed_at_epoch_seconds);
            task.error_message = None;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.timed_out_at_epoch_seconds = None;
            Ok(Some(task.clone()))
        })
    }

    fn fail(
        &self,
        request: TaskFailRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(None);
            };
            if task.status != TaskStatus::Running
                || task.claimed_by.as_deref() != Some(request.worker_id.as_str())
            {
                return Ok(None);
            }
            if let Some(checkpoint) = request.checkpoint {
                task.checkpoint = Some(checkpoint);
            }
            task.error_message = Some(request.error_message);
            task.updated_at_epoch_seconds = request.failed_at_epoch_seconds;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.next_attempt_at_epoch_seconds = request
                .retry_at_epoch_seconds
                .unwrap_or(request.failed_at_epoch_seconds);
            task.finished_at_epoch_seconds = if request.retry_at_epoch_seconds.is_some() {
                None
            } else {
                Some(request.failed_at_epoch_seconds)
            };
            task.timed_out_at_epoch_seconds = None;
            task.status = if request.retry_at_epoch_seconds.is_some() {
                TaskStatus::RetryScheduled
            } else {
                TaskStatus::Failed
            };
            Ok(Some(task.clone()))
        })
    }

    fn list_timeout_candidates(
        &self,
        now_epoch_seconds: i64,
        limit: usize,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Vec<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut candidates = tasks
                .lock()
                .expect("task repo mutex poisoned")
                .values()
                .filter(|task| task.is_timeout_candidate(now_epoch_seconds))
                .cloned()
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                left.lease_expires_at_epoch_seconds
                    .unwrap_or(i64::MAX)
                    .cmp(&right.lease_expires_at_epoch_seconds.unwrap_or(i64::MAX))
                    .then(
                        left.updated_at_epoch_seconds
                            .cmp(&right.updated_at_epoch_seconds),
                    )
            });
            candidates.truncate(limit);
            Ok(candidates)
        })
    }

    fn mark_timed_out(
        &self,
        request: TaskMarkTimedOutRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(false);
            };
            if task.status != TaskStatus::Running {
                return Ok(false);
            }
            if task.updated_at_epoch_seconds != request.expected_updated_at_epoch_seconds {
                return Ok(false);
            }
            if task.claimed_by != request.expected_claimed_by {
                return Ok(false);
            }
            task.status = TaskStatus::TimedOut;
            task.timed_out_at_epoch_seconds = Some(request.timed_out_at_epoch_seconds);
            task.finished_at_epoch_seconds = Some(request.timed_out_at_epoch_seconds);
            task.updated_at_epoch_seconds = request.timed_out_at_epoch_seconds;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            Ok(true)
        })
    }

    fn recover(
        &self,
        request: TaskRecoverRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<Result<bool, TaskSchedulerError>> {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            let mut tasks = tasks.lock().expect("task repo mutex poisoned");
            let Some(task) = tasks.get_mut(&request.task_id) else {
                return Ok(false);
            };
            if task.status != TaskStatus::Running {
                return Ok(false);
            }
            if task.updated_at_epoch_seconds != request.expected_updated_at_epoch_seconds {
                return Ok(false);
            }
            if task.claimed_by != request.expected_claimed_by {
                return Ok(false);
            }
            task.status = TaskStatus::RetryScheduled;
            task.next_attempt_at_epoch_seconds = request.recovered_at_epoch_seconds;
            task.updated_at_epoch_seconds = request.recovered_at_epoch_seconds;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.last_heartbeat_at_epoch_seconds = None;
            task.started_at_epoch_seconds = None;
            task.finished_at_epoch_seconds = None;
            task.timed_out_at_epoch_seconds = None;
            Ok(true)
        })
    }

    fn retry(
        &self,
        request: TaskRetryRequest,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
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
            task.next_attempt_at_epoch_seconds = request.retried_at_epoch_seconds;
            task.updated_at_epoch_seconds = request.retried_at_epoch_seconds;
            task.error_message = None;
            task.claimed_by = None;
            task.lease_expires_at_epoch_seconds = None;
            task.timed_out_at_epoch_seconds = None;
            task.started_at_epoch_seconds = None;
            task.finished_at_epoch_seconds = None;
            Ok(Some(task.clone()))
        })
    }

    fn find_by_id(
        &self,
        task_id: &str,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<ScheduledTask>, TaskSchedulerError>,
    > {
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
        filter: TaskListFilter,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Vec<ScheduledTask>, TaskSchedulerError>,
    > {
        let tasks = self.tasks.clone();
        Box::pin(async move {
            Ok(tasks
                .lock()
                .expect("task repo mutex poisoned")
                .values()
                .filter(|task| {
                    filter.task_types.is_empty()
                        || filter
                            .task_types
                            .iter()
                            .any(|task_type| task_type == &task.task_type)
                })
                .filter(|task| {
                    filter.statuses.is_empty()
                        || filter.statuses.iter().any(|status| status == &task.status)
                })
                .filter(|task| {
                    filter
                        .user_id
                        .as_deref()
                        .is_none_or(|user_id| user_id == task.user_id)
                })
                .cloned()
                .collect())
        })
    }
}

#[derive(Clone, Default)]
pub struct InMemoryTaskWorkerRepository {
    workers: Arc<Mutex<HashMap<String, TaskWorker>>>,
}

impl TaskWorkerRepository for InMemoryTaskWorkerRepository {
    fn upsert(
        &self,
        worker: TaskWorker,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>>
    {
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
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<Result<TaskWorker, TaskSchedulerError>>
    {
        let workers = self.workers.clone();
        let worker_id = worker_id.to_string();
        Box::pin(async move {
            let mut workers = workers.lock().expect("worker repo mutex poisoned");
            let worker = TaskWorker {
                worker_id: worker_id.clone(),
                is_leader,
                enabled_task_types,
                active_task_ids: Vec::new(),
                last_heartbeat_at_epoch_seconds,
            };
            workers.insert(worker_id, worker.clone());
            Ok(worker)
        })
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> aiwattcoach::domain::task_scheduler::BoxFuture<
        Result<Option<TaskWorker>, TaskSchedulerError>,
    > {
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
