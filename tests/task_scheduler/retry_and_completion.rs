use aiwattcoach::domain::identity::Clock;
use aiwattcoach::domain::task_scheduler::{
    FailTaskInput, NewTask, RetryStrategy, ScheduledTask, TaskSchedulerError, TaskStatus,
};
use serde_json::json;

use crate::support::{service, task, TestClock};

#[test]
fn scheduled_task_rejects_invalid_retry_strategies() {
    let fixed_invalid = ScheduledTask::new(
        NewTask {
            id: "task-1".to_string(),
            user_id: "user-1".to_string(),
            task_type: "summary".to_string(),
            payload: json!({}),
            retry_strategy: RetryStrategy::Fixed {
                max_attempts: 0,
                delay_seconds: 30,
            },
            dedupe_key: "dedupe-1".to_string(),
            execution_timeout_seconds: 30,
            leader_only: false,
        },
        100,
    );
    let exponential_invalid = ScheduledTask::new(
        NewTask {
            id: "task-2".to_string(),
            user_id: "user-1".to_string(),
            task_type: "summary".to_string(),
            payload: json!({}),
            retry_strategy: RetryStrategy::Exponential {
                max_attempts: 1,
                initial_delay_seconds: 60,
                max_delay_seconds: 30,
            },
            dedupe_key: "dedupe-2".to_string(),
            execution_timeout_seconds: 30,
            leader_only: false,
        },
        100,
    );

    assert!(matches!(
        fixed_invalid,
        Err(TaskSchedulerError::Validation(_))
    ));
    assert!(matches!(
        exponential_invalid,
        Err(TaskSchedulerError::Validation(_))
    ));
}

#[tokio::test]
async fn retry_task_requeues_timed_out_task_for_manual_reclaim() {
    let clock = TestClock::new(100);
    let service = service(&clock);

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
async fn complete_task_marks_running_task_completed() {
    let clock = TestClock::new(100);
    let service = service(&clock);

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
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let completed = service
        .complete_task("task-1", "worker-1", Some(json!({ "done": true })))
        .await
        .expect("complete should succeed")
        .expect("running task should complete");

    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.checkpoint, Some(json!({ "done": true })));
    assert!(completed.claimed_by.is_none());
}

#[tokio::test]
async fn fail_task_schedules_retry_for_retryable_task() {
    let clock = TestClock::new(100);
    let service = service(&clock);

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
    let claimed = service
        .claim_next_due("worker-1", vec!["summary".to_string()], false, 30)
        .await
        .expect("claim should succeed")
        .expect("task should be claimed");

    let failed = service
        .fail_task(FailTaskInput {
            task_id: "task-1",
            worker_id: "worker-1",
            checkpoint: Some(json!({ "stage": "provider" })),
            error_message: "temporary error".to_string(),
            retryable: true,
            retry_strategy: &claimed.retry_strategy,
            attempt_count: claimed.attempt_count,
        })
        .await
        .expect("fail should succeed")
        .expect("running task should fail");

    assert_eq!(failed.status, TaskStatus::RetryScheduled);
    assert_eq!(failed.next_attempt_at_epoch_seconds, 130);
    assert_eq!(failed.checkpoint, Some(json!({ "stage": "provider" })));
    assert_eq!(failed.error_message.as_deref(), Some("temporary error"));
}
