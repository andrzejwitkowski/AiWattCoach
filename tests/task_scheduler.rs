use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    identity::Clock,
    task_scheduler::{
        NewTask, RetryStrategy, ScheduledTask, TaskClaimRequest, TaskEnqueueResult,
        TaskHeartbeatRequest, TaskListFilter, TaskMarkTimedOutRequest, TaskRecoverRequest,
        TaskRepository, TaskRetryRequest, TaskSchedulerError, TaskSchedulerService, TaskStatus,
        TaskWorker, TaskWorkerRepository,
    },
};
use serde_json::json;

#[derive(Clone)]
struct TestClock {
    now: Arc<Mutex<i64>>,
}

impl TestClock {
    fn new(now_epoch_seconds: i64) -> Self {
        Self {
            now: Arc::new(Mutex::new(now_epoch_seconds)),
        }
    }

    fn set_now(&self, now_epoch_seconds: i64) {
        *self.now.lock().expect("clock mutex poisoned") = now_epoch_seconds;
    }
}

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        *self.now.lock().expect("clock mutex poisoned")
    }
}

#[derive(Clone, Default)]
struct InMemoryTaskRepository {
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
            if let Some(existing) = tasks
                .values()
                .find(|existing| existing.dedupe_key == task.dedupe_key)
            {
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
struct InMemoryTaskWorkerRepository {
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
            let active_task_ids = workers
                .get(&worker_id)
                .map(|worker| worker.active_task_ids.clone())
                .unwrap_or_default();
            let worker = TaskWorker {
                worker_id: worker_id.clone(),
                is_leader,
                enabled_task_types,
                active_task_ids,
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

fn task(
    id: &str,
    task_type: &str,
    dedupe_key: &str,
    leader_only: bool,
    now_epoch_seconds: i64,
) -> ScheduledTask {
    ScheduledTask::new(
        NewTask {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            task_type: task_type.to_string(),
            payload: json!({ "payload": dedupe_key }),
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 3,
                delay_seconds: 30,
            },
            dedupe_key: dedupe_key.to_string(),
            execution_timeout_seconds: 30,
            leader_only,
        },
        now_epoch_seconds,
    )
    .expect("task fixture should be valid")
}

#[tokio::test]
async fn claim_next_due_does_not_double_claim_same_task() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");

    let claimed_by_worker_one = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("first claim should succeed");
    let claimed_by_worker_two = service
        .claim_next_due("worker-2", vec!["summary".to_string()], false, 30)
        .await
        .expect("second claim should succeed");

    assert_eq!(
        claimed_by_worker_one.expect("worker one should claim").id,
        "task-1"
    );
    assert!(claimed_by_worker_two.is_none());
}

#[tokio::test]
async fn leader_only_task_requires_leader_worker() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            true,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");

    let claimed_by_non_leader = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("non-leader claim should not error");
    let claimed_by_leader = service
        .claim_next_due("worker-2", vec!["summary".to_string()], true, 30)
        .await
        .expect("leader claim should not error");

    assert!(claimed_by_non_leader.is_none());
    assert_eq!(claimed_by_leader.expect("leader should claim").id, "task-1");
}

#[tokio::test]
async fn timeout_sweep_keeps_running_task_when_owner_reports_it_active() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 0);
    assert_eq!(task.status, TaskStatus::Running);
}

#[tokio::test]
async fn timeout_sweep_marks_task_timed_out_when_owner_is_stale() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");
    clock.set_now(160);

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 1);
    assert_eq!(task.status, TaskStatus::TimedOut);
    assert!(task.claimed_by.is_none());
}

#[tokio::test]
async fn timeout_sweep_recovers_task_when_worker_disappears() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(recovered, 1);
    assert_eq!(task.status, TaskStatus::RetryScheduled);
    assert!(task.claimed_by.is_none());
}

#[tokio::test]
async fn timeout_sweep_recovers_task_when_worker_restarts_without_active_claim() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .touch_worker_heartbeat("worker-1", false, vec!["summary".to_string()])
        .await
        .expect("worker heartbeat should succeed");

    let recovered = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let reclaimed = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("reclaim should succeed")
        .expect("restarted worker should reclaim recovered task");

    assert_eq!(recovered, 1);
    assert_eq!(reclaimed.id, "task-1");
    assert_eq!(reclaimed.claimed_by.as_deref(), Some("worker-1"));
}

#[tokio::test]
async fn retry_task_requeues_timed_out_task_for_manual_reclaim() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");
    clock.set_now(160);
    service
        .heartbeat_worker(
            "worker-1",
            false,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker heartbeat should succeed");
    service
        .heartbeat_task("task-1", "worker-1", 5)
        .await
        .expect("task heartbeat should succeed")
        .expect("task should accept heartbeat");
    clock.set_now(400);
    service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");

    let timed_out = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out.status, TaskStatus::TimedOut);

    let retried = service
        .retry_task("task-1")
        .await
        .expect("retry should succeed")
        .expect("timed out task should be retryable");

    assert_eq!(retried.status, TaskStatus::Queued);
    assert_eq!(retried.next_attempt_at_epoch_seconds, 400);
    assert!(retried.timed_out_at_epoch_seconds.is_none());
}

#[tokio::test]
async fn timeout_sweep_keeps_running_task_when_task_heartbeat_is_fresh() {
    let clock = TestClock::new(100);
    let tasks = InMemoryTaskRepository::default();
    let workers = InMemoryTaskWorkerRepository::default();
    let service = TaskSchedulerService::new(tasks, workers, clock.clone());

    service
        .enqueue(task(
            "task-1",
            "summary",
            "dedupe-1",
            false,
            clock.now_epoch_seconds(),
        ))
        .await
        .expect("enqueue should succeed");
    service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 5)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    clock.set_now(160);
    service
        .heartbeat_task("task-1", "worker-1", 30)
        .await
        .expect("task heartbeat should succeed")
        .expect("task heartbeat should keep task alive");

    let timed_out = service
        .sweep_timed_out_tasks(30, 100)
        .await
        .expect("timeout sweep should succeed");
    let task = service
        .get_task("task-1")
        .await
        .expect("task lookup should succeed")
        .expect("task should exist");

    assert_eq!(timed_out, 0);
    assert_eq!(task.status, TaskStatus::Running);
}
