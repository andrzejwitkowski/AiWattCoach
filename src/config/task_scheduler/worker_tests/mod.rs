mod support;

use std::{sync::Arc, time::Duration};

use serde_json::json;
use tokio::time::timeout;

use super::worker::spawn_task_worker;
use crate::domain::{
    identity::Clock,
    task_scheduler::{
        NewTask, RetryStrategy, ScheduledTask, SharedTaskHandler, TaskRepository,
        TaskSchedulerError, TaskSchedulerService, TaskStatus, TaskWorkerConfig,
    },
};

use self::support::{
    wait_for_notify, InMemoryTaskRepository, InMemoryTaskWorkerRepository, PanicTaskHandler,
    StaticTaskHandler, TestClock,
};

#[test]
fn spawn_task_worker_rejects_duplicate_task_handlers() {
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let handler = StaticTaskHandler::new("duplicate.task");
    let duplicate_handlers: Vec<SharedTaskHandler> = vec![handler.clone(), handler];

    let error = spawn_task_worker(
        scheduler,
        "worker-1".to_string(),
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: 30,
            heartbeat_interval: Duration::from_secs(10),
            idle_poll_interval: Duration::from_millis(10),
            max_concurrency: 1,
        },
        duplicate_handlers,
    )
    .expect_err("duplicate handlers should return structured error");

    assert_eq!(
        error,
        TaskSchedulerError::Conflict(
            "duplicate task handler registered for duplicate.task".to_string()
        )
    );
}

#[test]
fn spawn_task_worker_rejects_non_positive_worker_timing() {
    let test_cases = [
        (
            TaskWorkerConfig {
                is_leader: false,
                lease_duration_seconds: 0,
                heartbeat_interval: Duration::from_secs(10),
                idle_poll_interval: Duration::from_millis(10),
                max_concurrency: 1,
            },
            "task worker lease_duration_seconds must be positive",
        ),
        (
            TaskWorkerConfig {
                is_leader: false,
                lease_duration_seconds: 30,
                heartbeat_interval: Duration::ZERO,
                idle_poll_interval: Duration::from_millis(10),
                max_concurrency: 1,
            },
            "task worker heartbeat_interval must be positive",
        ),
        (
            TaskWorkerConfig {
                is_leader: false,
                lease_duration_seconds: 30,
                heartbeat_interval: Duration::from_secs(10),
                idle_poll_interval: Duration::ZERO,
                max_concurrency: 1,
            },
            "task worker idle_poll_interval must be positive",
        ),
    ];

    for (config, expected_message) in test_cases {
        let scheduler = TaskSchedulerService::new(
            InMemoryTaskRepository::default(),
            InMemoryTaskWorkerRepository::default(),
            TestClock::default(),
        );
        let handler = StaticTaskHandler::new("timing.task");

        let error = spawn_task_worker(scheduler, "worker-1".to_string(), config, vec![handler])
            .expect_err("invalid timing config should return structured error");

        assert_eq!(
            error,
            TaskSchedulerError::Validation(expected_message.to_string())
        );
    }
}

#[tokio::test]
async fn task_worker_retries_and_clears_active_task_ids_when_handler_panics() {
    let task_repository = InMemoryTaskRepository::default();
    let worker_repository = InMemoryTaskWorkerRepository::default();
    let clock = TestClock::default();
    let panic_handler = PanicTaskHandler::new();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        worker_repository.clone(),
        clock.clone(),
    );
    let task = ScheduledTask::new(
        NewTask {
            id: "task-panic".to_string(),
            user_id: "user-1".to_string(),
            task_type: PanicTaskHandler::TASK_TYPE.to_string(),
            payload: json!({}),
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 2,
                delay_seconds: 5,
            },
            dedupe_key: "panic-task".to_string(),
            execution_timeout_seconds: 60,
            leader_only: false,
        },
        clock.now_epoch_seconds(),
    )
    .expect("panic task should be valid");
    task_repository
        .enqueue_if_absent(task)
        .await
        .expect("panic task should enqueue");

    let worker = spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: 30,
            heartbeat_interval: Duration::from_secs(10),
            idle_poll_interval: Duration::from_millis(10),
            max_concurrency: 1,
        },
        vec![panic_handler.clone()],
    )
    .expect("panic worker should spawn");

    wait_for_notify(&panic_handler.started, Duration::from_secs(2))
        .await
        .expect("panic handler should start before test timeout");

    timeout(Duration::from_secs(2), async {
        loop {
            let task = task_repository.only_task();
            if task.status == TaskStatus::Running && task.claimed_by.as_deref() == Some("worker-1")
            {
                break;
            }
            tokio::task::yield_now().await;
        }

        panic_handler.release.notify_one();

        loop {
            if worker_repository
                .worker("worker-1")
                .is_some_and(|worker| worker.active_task_ids.is_empty())
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker should clear active task ids after panic");

    let task = task_repository.only_task();
    assert_eq!(task.status, TaskStatus::RetryScheduled);
    assert_eq!(task.claimed_by, None);
    assert_eq!(
        task.error_message.as_deref(),
        Some("scheduled task handler panicked")
    );

    worker.shutdown().await;
}

#[tokio::test]
async fn wait_for_notify_times_out_when_handler_never_starts() {
    let notify = Arc::new(tokio::sync::Notify::new());

    let result = wait_for_notify(&notify, Duration::from_millis(10)).await;

    assert!(result.is_err());
}
