use serde_json::json;
use serial_test::serial;

use crate::domain::task_scheduler::{
    NewTask, RetryStrategy, SharedTaskHandler, TaskSchedulerError, TaskSchedulerService, TaskStatus,
};

use super::super::*;
use super::support::{
    direct_service, existing_summary, BlockingCoach, InMemoryTaskRepository,
    InMemoryTaskWorkerRepository, InMemoryWorkoutSummaryRepository, TestClock, TestIdGenerator,
};

#[tokio::test]
#[serial]
async fn workout_summary_task_runner_reports_active_task_ids() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let coach = BlockingCoach::new();
    let direct = direct_service(repository, coach.clone());
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let worker_repository = InMemoryTaskWorkerRepository::default();
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        worker_repository.clone(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let reply_task = tokio::spawn(async move {
        service
            .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
            .await
    });

    coach.started.notified().await;
    let running_worker = worker_repository
        .worker("worker-1")
        .expect("worker heartbeat should be recorded");
    assert_eq!(
        running_worker.enabled_task_types,
        vec![COACH_REPLY_TASK_TYPE.to_string()]
    );
    assert_eq!(running_worker.active_task_ids, vec!["task-0".to_string()]);

    coach.release.notify_one();
    let reply = reply_task
        .await
        .expect("reply task join should succeed")
        .expect("reply should succeed");
    assert_eq!(reply.coach_message.content, "Coach reply to: Need feedback");

    let idle_worker = worker_repository
        .worker("worker-1")
        .expect("worker heartbeat should remain recorded");
    assert!(idle_worker.active_task_ids.is_empty());

    worker.shutdown().await;
}

#[tokio::test]
#[serial]
async fn workout_summary_task_runner_fails_invalid_payload_without_feature_specific_loop_logic() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, BlockingCoach::new());
    let task_repository = InMemoryTaskRepository::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let task = crate::domain::task_scheduler::ScheduledTask::new(
        NewTask {
            id: "task-invalid".to_string(),
            user_id: "user-1".to_string(),
            task_type: COACH_REPLY_TASK_TYPE.to_string(),
            payload: json!({ "user_id": "user-1" }),
            retry_strategy: RetryStrategy::Never,
            dedupe_key: "invalid-payload".to_string(),
            execution_timeout_seconds: COACH_REPLY_EXECUTION_TIMEOUT_SECONDS,
            leader_only: false,
        },
        TestClock.now_epoch_seconds(),
    )
    .expect("task should be valid apart from payload shape");
    task_repository
        .enqueue_if_absent(task)
        .await
        .expect("task should enqueue");

    let worker =
        spawn_workout_summary_coach_reply_task_runner(direct, scheduler, "worker-1".to_string())
            .expect("worker should spawn");

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let task = task_repository.only_task();
            if task.status == TaskStatus::Failed {
                assert_eq!(
                    task.error_message,
                    Some(
                        "invalid workout summary coach reply task payload: missing field `workout_id`"
                            .to_string()
                    )
                );
                assert!(task.checkpoint.is_none());
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("runner should fail invalid payload task");

    worker.shutdown().await;
}

#[test]
fn spawn_task_worker_rejects_duplicate_task_handlers() {
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let handler = workout_summary_coach_reply_task_handler(direct_service(
        InMemoryWorkoutSummaryRepository::with_summary(existing_summary()),
        BlockingCoach::new(),
    ));
    let duplicate_handlers: Vec<SharedTaskHandler> = vec![handler.clone(), handler];

    let error = spawn_task_worker(
        scheduler,
        "worker-1".to_string(),
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: 30,
            heartbeat_interval: std::time::Duration::from_secs(10),
            idle_poll_interval: std::time::Duration::from_millis(10),
            max_concurrency: 1,
        },
        duplicate_handlers,
    )
    .expect_err("duplicate handlers should return structured error");

    assert_eq!(
        error,
        TaskSchedulerError::Conflict(
            "duplicate task handler registered for workout_summary.coach_reply".to_string()
        )
    );
}
