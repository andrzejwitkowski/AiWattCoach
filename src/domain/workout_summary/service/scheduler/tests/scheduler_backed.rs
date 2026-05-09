use crate::domain::{
    identity::Clock,
    llm::LlmError,
    task_scheduler::{TaskSchedulerService, TaskStatus},
    workout_summary::{CoachReplyOperation, CoachReplyOperationStatus, MessageRole},
};
use serial_test::serial;

use super::super::*;
use super::support::{
    direct_service, direct_service_with_athlete_summary, direct_service_with_operation_repository,
    existing_summary, InMemoryCoachReplyOperationRepository, InMemoryTaskRepository,
    InMemoryTaskWorkerRepository, InMemoryWorkoutSummaryRepository, TestClock, TestCoach,
    TestIdGenerator,
};
use crate::test_support::spawn_test_workout_summary_task_worker;

#[tokio::test]
#[serial]
async fn scheduler_backed_send_message_waits_for_background_task_result() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, TestCoach::successful());
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
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

    worker.shutdown().await;
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
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
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

    worker.shutdown().await;
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
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let result = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
        .await
        .expect("scheduler-backed coach reply should succeed");

    assert!(result.athlete_summary_was_regenerated);

    worker.shutdown().await;
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
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
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

    worker.shutdown().await;
}

#[tokio::test]
#[serial]
async fn scheduler_backed_generate_coach_reply_waits_for_pending_operation_reclaim_window() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let reply_operations = InMemoryCoachReplyOperationRepository::default();
    let clock = TestClock::default();
    let direct = direct_service_with_operation_repository(
        repository,
        reply_operations.clone(),
        clock.clone(),
        TestCoach::successful(),
    );
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let user_message_id = persisted.user_message.id.clone();
    let asserted_user_message_id = user_message_id.clone();
    reply_operations.seed(CoachReplyOperation::pending(
        "user-1".to_string(),
        "workout-1".to_string(),
        user_message_id.clone(),
        Some("workout-summary:user-1:workout-1".to_string()),
        "message-pending".to_string(),
        clock.now_epoch_seconds(),
    ));

    let task_repository = InMemoryTaskRepository::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        clock.clone(),
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
    let service =
        SchedulerBackedWorkoutSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let reply_future = tokio::spawn(async move {
        service
            .generate_coach_reply("user-1", "workout-1", user_message_id)
            .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let Some(task) = task_repository.only_task_if_present() else {
                tokio::task::yield_now().await;
                continue;
            };
            if task.status == TaskStatus::RetryScheduled {
                assert_eq!(
                    task.error_message.as_deref(),
                    Some("coach reply generation is already pending for this message")
                );
                assert_eq!(task.next_attempt_at_epoch_seconds, 1_700_000_300);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("runner should schedule a delayed retry");

    let scheduled_retry = task_repository.only_task();
    assert_eq!(scheduled_retry.status, TaskStatus::RetryScheduled);

    clock.set_now(1_700_000_300);

    let reply = tokio::time::timeout(std::time::Duration::from_secs(2), reply_future)
        .await
        .expect("scheduler-backed reply should finish after the reclaim window opens")
        .expect("reply task join should succeed")
        .expect("reply should succeed after delayed retry");

    assert_eq!(reply.coach_message.id, "message-pending");
    assert_eq!(reply.coach_message.role, MessageRole::Coach);

    let stored_task = task_repository.only_task();
    assert_eq!(stored_task.status, TaskStatus::Completed);
    let stored_operation = reply_operations
        .get("user-1", "workout-1", &asserted_user_message_id)
        .expect("reclaimed operation should still exist");
    assert_eq!(
        stored_operation.status,
        CoachReplyOperationStatus::Completed
    );

    worker.shutdown().await;
}

#[tokio::test]
#[serial]
async fn scheduler_backed_generate_coach_reply_retries_after_failed_task_on_explicit_retry() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let coach = TestCoach::failing(LlmError::ProviderRejected("invalid model".to_string()));
    let direct = direct_service(repository, coach.clone());
    let persisted = direct
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .expect("user message should persist");
    let task_repository = InMemoryTaskRepository::default();
    let clock = TestClock::default();
    let scheduler = TaskSchedulerService::new(
        task_repository.clone(),
        InMemoryTaskWorkerRepository::default(),
        clock,
    );
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
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

    let reply = service
        .generate_coach_reply("user-1", "workout-1", persisted.user_message.id)
        .await
        .expect("second reply attempt should retry task and succeed");

    assert_eq!(reply.coach_message.role, MessageRole::Coach);
    assert_eq!(reply.coach_message.content, "Coach reply to: Need feedback");

    worker.shutdown().await;
}

#[tokio::test]
#[serial]
async fn scheduler_backed_send_message_does_not_accumulate_scheduler_state_across_repeated_runs() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, TestCoach::successful());
    let task_repository = InMemoryTaskRepository::default();
    let worker_repository = InMemoryTaskWorkerRepository::default();
    let scheduler =
        TaskSchedulerService::new(task_repository, worker_repository, TestClock::default());
    let worker = spawn_test_workout_summary_task_worker(
        direct.clone(),
        scheduler.clone(),
        "worker-1".to_string(),
    )
    .expect("worker should spawn");
    let service = SchedulerBackedWorkoutSummaryService::new(
        direct,
        scheduler.clone(),
        TestIdGenerator::default(),
    );

    for attempt in 0..20 {
        let result = service
            .send_message("user-1", "workout-1", format!("Need feedback {attempt}"))
            .await
            .expect("scheduler-backed send should succeed repeatedly");

        assert_eq!(result.user_message.role, MessageRole::User);
        assert_eq!(result.coach_message.role, MessageRole::Coach);
        assert_eq!(scheduler.test_waiter_count().await, 0);
        assert_eq!(scheduler.test_worker_state_count().await, 1);
    }

    worker.shutdown().await;
    assert_eq!(scheduler.test_waiter_count().await, 0);
}

#[tokio::test]
#[serial]
async fn scheduler_backed_worker_restarts_do_not_accumulate_scheduler_state() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let direct = direct_service(repository, TestCoach::successful());
    let task_repository = InMemoryTaskRepository::default();
    let worker_repository = InMemoryTaskWorkerRepository::default();
    let scheduler =
        TaskSchedulerService::new(task_repository, worker_repository, TestClock::default());
    let service = SchedulerBackedWorkoutSummaryService::new(
        direct.clone(),
        scheduler.clone(),
        TestIdGenerator::default(),
    );

    for attempt in 0..10 {
        let worker = spawn_test_workout_summary_task_worker(
            direct.clone(),
            scheduler.clone(),
            "worker-1".to_string(),
        )
        .expect("worker should spawn");

        let result = service
            .send_message(
                "user-1",
                "workout-1",
                format!("Need feedback restart {attempt}"),
            )
            .await
            .expect("scheduler-backed send should succeed after restart");

        assert_eq!(result.user_message.role, MessageRole::User);
        assert_eq!(result.coach_message.role, MessageRole::Coach);
        worker.shutdown().await;
        assert_eq!(scheduler.test_waiter_count().await, 0);
        assert_eq!(scheduler.test_worker_state_count().await, 1);
    }
}
