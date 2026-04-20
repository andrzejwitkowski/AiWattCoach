use serial_test::serial;
use std::sync::Arc;

use crate::domain::{
    llm::LlmError,
    task_scheduler::{TaskSchedulerService, TaskStatus},
    workout_summary::MessageRole,
};

use super::super::*;
use super::support::{
    direct_service, direct_service_with_athlete_summary, existing_summary, InMemoryTaskRepository,
    InMemoryTaskWorkerRepository, InMemoryWorkoutSummaryRepository, TestClock, TestCoach,
    TestIdGenerator,
};

#[tokio::test]
#[serial]
async fn scheduler_backed_send_message_waits_for_background_task_result() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, TestCoach::successful());
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    );
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let result = service
        .send_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("scheduler-backed send should succeed");

    assert_eq!(result.user_message.role, MessageRole::User);
    assert_eq!(result.coach_message.role, MessageRole::Coach);
    assert_eq!(
        result.coach_message.content,
        "Coach reply to: Need feedback"
    );

    worker.abort();
}

#[tokio::test]
#[serial]
async fn scheduler_backed_generate_coach_reply_waits_for_background_task_result() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, TestCoach::successful());
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    );
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let result = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
        .await
        .expect("scheduler-backed coach reply should succeed");

    assert_eq!(result.coach_message.role, MessageRole::Coach);
    assert_eq!(
        result.coach_message.content,
        "Coach reply to: Need feedback"
    );

    worker.abort();
}

#[tokio::test]
#[serial]
async fn scheduler_backed_generate_coach_reply_preserves_athlete_summary_regeneration_flag() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service_with_athlete_summary(repository, TestCoach::successful(), true);
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    );
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let result = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
        .await
        .expect("scheduler-backed coach reply should succeed");

    assert!(result.athlete_summary_was_regenerated);

    worker.abort();
}

#[tokio::test]
#[serial]
async fn scheduler_backed_send_message_returns_failed_task_error() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(
        repository,
        TestCoach::failing(LlmError::ProviderRejected("invalid model".to_string())),
    );
    let task_repository = InMemoryTaskRepository::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    );
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let error = service
        .send_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect_err("scheduler-backed send should surface task failure");

    let stored_task = task_repository.only_task();
    assert_eq!(stored_task.status, TaskStatus::Failed);
    assert!(stored_task.checkpoint.is_some());
    assert_eq!(
        error,
        crate::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::ProviderRejected(
            "invalid model".to_string()
        ))
    );

    worker.abort();
}

#[tokio::test]
#[serial]
async fn scheduler_backed_generate_coach_reply_retries_after_failed_task_on_explicit_retry() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let coach = Arc::new(TestCoach::default());
    *coach.fail_with.lock().expect("coach mutex poisoned") =
        Some(LlmError::ProviderRejected("invalid model".to_string()));
    let direct = direct_service(repository, coach.clone());
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let task_repository = InMemoryTaskRepository::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        TestClock,
    );
    let worker = spawn_workout_summary_coach_reply_task_runner(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    );
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let first_error = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id.clone())
        .await
        .expect_err("first reply attempt should fail");
    let stored_task = task_repository.only_task();
    assert_eq!(stored_task.status, TaskStatus::Failed);
    assert!(stored_task.checkpoint.is_some());
    assert_eq!(
        first_error,
        crate::domain::workout_summary::WorkoutSummaryError::Llm(LlmError::ProviderRejected(
            "invalid model".to_string()
        ))
    );

    *coach.fail_with.lock().expect("coach mutex poisoned") = None;

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
        .await
        .expect("second reply attempt should retry task and succeed");

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(reply.coach_message.content, "Coach reply to: Need feedback");

    worker.abort();
}
