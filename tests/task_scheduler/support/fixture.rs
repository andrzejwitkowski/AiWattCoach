use aiwattcoach::domain::task_scheduler::{
    NewTask, RetryStrategy, ScheduledTask, TaskSchedulerService,
};
use serde_json::json;

use super::{InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock};

pub fn task(
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

pub fn service(
    clock: &TestClock,
) -> TaskSchedulerService<InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock> {
    TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock.clone(),
    )
}
