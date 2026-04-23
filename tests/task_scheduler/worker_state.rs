use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use aiwattcoach::domain::{
    identity::Clock,
    task_scheduler::{BoxFuture, TaskSchedulerError, TaskWorker, TaskWorkerRepository},
};
use tokio::sync::Notify;

use crate::support::{service, task, TestClock};

#[tokio::test]
async fn worker_heartbeat_keeps_leader_flag_when_active_tasks_change() {
    let clock = TestClock::new(100);
    let service = service(&clock);

    service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("worker heartbeat should succeed");
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
        .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
        .await
        .expect("adding active task should succeed");
    let running_worker = service
        .heartbeat_worker(
            "worker-1",
            true,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect("worker reload should succeed");

    assert!(running_worker.is_leader);
    assert_eq!(running_worker.active_task_ids, vec!["task-1".to_string()]);

    service
        .remove_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
        .await
        .expect("removing active task should succeed");
    let idle_worker = service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("worker reload should succeed");

    assert!(idle_worker.is_leader);
    assert!(idle_worker.active_task_ids.is_empty());
}

#[tokio::test]
async fn worker_active_task_update_rolls_back_cache_when_persist_fails() {
    let clock = TestClock::new(100);
    let workers = RecordingTaskWorkerRepository::default();
    let service = aiwattcoach::domain::task_scheduler::TaskSchedulerService::new(
        crate::support::InMemoryTaskRepository::default(),
        workers.clone(),
        clock,
    );

    service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("initial worker heartbeat should succeed");
    workers.fail_next_upsert();

    let error = service
        .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
        .await
        .expect_err("persist failure should bubble up");

    assert_eq!(
        error,
        TaskSchedulerError::Repository("worker upsert failed".to_string())
    );

    let persisted = service
        .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-2")
        .await
        .expect("next update should succeed");

    assert_eq!(persisted.active_task_ids, vec!["task-2".to_string()]);
    assert_eq!(
        workers
            .find_by_worker_id_blocking("worker-1")
            .expect("worker should exist after successful retry")
            .active_task_ids,
        vec!["task-2".to_string()]
    );
}

#[tokio::test]
async fn worker_heartbeat_rolls_back_cache_when_persist_fails() {
    let clock = TestClock::new(100);
    let workers = RecordingTaskWorkerRepository::default();
    let service = aiwattcoach::domain::task_scheduler::TaskSchedulerService::new(
        crate::support::InMemoryTaskRepository::default(),
        workers.clone(),
        clock,
    );

    service
        .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
        .await
        .expect("initial worker heartbeat should succeed");
    workers.fail_next_upsert();

    let error = service
        .heartbeat_worker(
            "worker-1",
            true,
            vec!["summary".to_string()],
            vec!["task-1".to_string()],
        )
        .await
        .expect_err("persist failure should bubble up");

    assert_eq!(
        error,
        TaskSchedulerError::Repository("worker upsert failed".to_string())
    );

    let persisted = service
        .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-2")
        .await
        .expect("next update should succeed");

    assert_eq!(persisted.active_task_ids, vec!["task-2".to_string()]);
    assert_eq!(
        workers
            .find_by_worker_id_blocking("worker-1")
            .expect("worker should exist after successful retry")
            .active_task_ids,
        vec!["task-2".to_string()]
    );
}

#[tokio::test]
async fn worker_active_task_updates_are_serialized_before_persist() {
    let clock = TestClock::new(100);
    let workers = BlockingTaskWorkerRepository::default();
    let service = aiwattcoach::domain::task_scheduler::TaskSchedulerService::new(
        crate::support::InMemoryTaskRepository::default(),
        workers.clone(),
        clock,
    );

    let first_service = service.clone();
    let first = tokio::spawn(async move {
        first_service
            .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
            .await
    });
    workers.wait_until_first_upsert_starts().await;

    let second_service = service.clone();
    let second = tokio::spawn(async move {
        second_service
            .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-2")
            .await
    });

    tokio::task::yield_now().await;
    assert_eq!(workers.upsert_calls(), 1);

    workers.release_first_upsert();

    let first_persisted = first
        .await
        .expect("first task should join")
        .expect("first persist should succeed");
    let second_persisted = second
        .await
        .expect("second task should join")
        .expect("second persist should succeed");

    assert_eq!(first_persisted.active_task_ids, vec!["task-1".to_string()]);
    assert_eq!(
        second_persisted.active_task_ids,
        vec!["task-1".to_string(), "task-2".to_string()]
    );
    assert_eq!(workers.upsert_calls(), 2);
    assert_eq!(
        workers
            .find_by_worker_id_blocking("worker-1")
            .expect("worker should be persisted")
            .active_task_ids,
        vec!["task-1".to_string(), "task-2".to_string()]
    );
}

#[tokio::test]
async fn worker_heartbeat_is_serialized_with_active_task_updates() {
    let clock = TestClock::new(100);
    let workers = BlockingTaskWorkerRepository::default();
    let service = aiwattcoach::domain::task_scheduler::TaskSchedulerService::new(
        crate::support::InMemoryTaskRepository::default(),
        workers.clone(),
        clock,
    );

    let heartbeat_service = service.clone();
    let heartbeat = tokio::spawn(async move {
        heartbeat_service
            .heartbeat_worker("worker-1", true, vec!["summary".to_string()], Vec::new())
            .await
    });
    workers.wait_until_first_upsert_starts().await;

    let active_task_service = service.clone();
    let active_task = tokio::spawn(async move {
        active_task_service
            .add_worker_active_task("worker-1", true, vec!["summary".to_string()], "task-1")
            .await
    });

    tokio::task::yield_now().await;
    assert_eq!(workers.upsert_calls(), 1);

    workers.release_first_upsert();

    heartbeat
        .await
        .expect("heartbeat task should join")
        .expect("heartbeat persist should succeed");
    let persisted = active_task
        .await
        .expect("active task should join")
        .expect("active task persist should succeed");

    assert_eq!(persisted.active_task_ids, vec!["task-1".to_string()]);
    assert_eq!(workers.upsert_calls(), 2);
    assert_eq!(
        workers
            .find_by_worker_id_blocking("worker-1")
            .expect("worker should be persisted")
            .active_task_ids,
        vec!["task-1".to_string()]
    );
}

#[derive(Clone, Default)]
struct RecordingTaskWorkerRepository {
    workers: Arc<Mutex<HashMap<String, TaskWorker>>>,
    fail_next_upsert: Arc<AtomicBool>,
}

impl RecordingTaskWorkerRepository {
    fn fail_next_upsert(&self) {
        self.fail_next_upsert.store(true, Ordering::SeqCst);
    }

    fn find_by_worker_id_blocking(&self, worker_id: &str) -> Option<TaskWorker> {
        self.workers
            .lock()
            .expect("worker mutex poisoned")
            .get(worker_id)
            .cloned()
    }
}

impl TaskWorkerRepository for RecordingTaskWorkerRepository {
    fn upsert(&self, worker: TaskWorker) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let workers = self.workers.clone();
        let fail_next_upsert = self.fail_next_upsert.clone();
        Box::pin(async move {
            if fail_next_upsert.swap(false, Ordering::SeqCst) {
                return Err(TaskSchedulerError::Repository(
                    "worker upsert failed".to_string(),
                ));
            }

            workers
                .lock()
                .expect("worker mutex poisoned")
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
        self.upsert(TaskWorker {
            worker_id: worker_id.to_string(),
            is_leader,
            enabled_task_types,
            active_task_ids: Vec::new(),
            last_heartbeat_at_epoch_seconds,
        })
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>> {
        let worker = self.find_by_worker_id_blocking(worker_id);
        Box::pin(async move { Ok(worker) })
    }
}

#[derive(Clone, Default)]
struct BlockingTaskWorkerRepository {
    delegate: RecordingTaskWorkerRepository,
    upsert_calls: Arc<AtomicUsize>,
    first_upsert_started: Arc<Notify>,
    release_first_upsert: Arc<Notify>,
}

impl BlockingTaskWorkerRepository {
    async fn wait_until_first_upsert_starts(&self) {
        self.first_upsert_started.notified().await;
    }

    fn release_first_upsert(&self) {
        self.release_first_upsert.notify_waiters();
    }

    fn upsert_calls(&self) -> usize {
        self.upsert_calls.load(Ordering::SeqCst)
    }

    fn find_by_worker_id_blocking(&self, worker_id: &str) -> Option<TaskWorker> {
        self.delegate.find_by_worker_id_blocking(worker_id)
    }
}

impl TaskWorkerRepository for BlockingTaskWorkerRepository {
    fn upsert(&self, worker: TaskWorker) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        let delegate = self.delegate.clone();
        let upsert_calls = self.upsert_calls.clone();
        let first_upsert_started = self.first_upsert_started.clone();
        let release_first_upsert = self.release_first_upsert.clone();
        Box::pin(async move {
            if upsert_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                first_upsert_started.notify_one();
                release_first_upsert.notified().await;
            }
            delegate.upsert(worker).await
        })
    }

    fn touch_heartbeat(
        &self,
        worker_id: &str,
        is_leader: bool,
        enabled_task_types: Vec<String>,
        last_heartbeat_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<TaskWorker, TaskSchedulerError>> {
        self.delegate.touch_heartbeat(
            worker_id,
            is_leader,
            enabled_task_types,
            last_heartbeat_at_epoch_seconds,
        )
    }

    fn find_by_worker_id(
        &self,
        worker_id: &str,
    ) -> BoxFuture<Result<Option<TaskWorker>, TaskSchedulerError>> {
        self.delegate.find_by_worker_id(worker_id)
    }
}
