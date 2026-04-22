use std::sync::Arc;

use crate::domain::{
    athlete_summary::{
        athlete_summary_generate_task_handler, AthleteSummaryError,
        AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationStatus,
        AthleteSummaryService, AthleteSummaryUseCases, SchedulerBackedAthleteSummaryService,
    },
    llm::LlmError,
    task_scheduler::{ScheduledTask, TaskSchedulerService, TaskStatus, TaskWorkerConfig},
};

use super::super::super::core::{
    GENERATION_ALREADY_PENDING_MESSAGE, STALE_PENDING_TIMEOUT_SECONDS,
};
use super::support::{
    llm_response, summary, InMemoryAthleteSummaryOperationRepository,
    InMemoryAthleteSummaryRepository, InMemoryTaskRepository, InMemoryTaskWorkerRepository,
    StubGenerator, TestClock, TestIdGenerator, LAST_WEEK_EPOCH_SECONDS, NOW_EPOCH_SECONDS,
    THIS_WEEK_EPOCH_SECONDS, USER_ID,
};

#[tokio::test]
async fn scheduler_backed_generate_summary_returns_fresh_summary_without_enqueuing() {
    let repository =
        InMemoryAthleteSummaryRepository::with_summary(summary("fresh", THIS_WEEK_EPOCH_SECONDS));
    let (scheduler, tasks) = test_scheduler();
    let service = SchedulerBackedAthleteSummaryService::new(
        Arc::new(AthleteSummaryService::new(
            repository,
            InMemoryAthleteSummaryOperationRepository::default(),
            StubGenerator::succeeds_with("should not run"),
            TestClock::new(NOW_EPOCH_SECONDS),
        )),
        scheduler,
        TestIdGenerator::default(),
    );

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "fresh");
    assert!(tasks.only_task_if_present().is_none());
}

#[tokio::test]
async fn scheduler_backed_generate_summary_runs_through_shared_worker() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::default(),
        InMemoryAthleteSummaryOperationRepository::default(),
        StubGenerator::succeeds_with("generated"),
        clock.clone(),
    ));
    let (scheduler, _) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service =
        SchedulerBackedAthleteSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "generated");
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_summary_retries_retryable_llm_failure_before_failing() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let generator = StubGenerator::queued(vec![
        Err(LlmError::RateLimited("slow down".to_string())),
        Err(LlmError::RateLimited("slow down".to_string())),
        Err(LlmError::RateLimited("slow down".to_string())),
    ]);
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::default(),
        InMemoryAthleteSummaryOperationRepository::default(),
        generator.clone(),
        clock.clone(),
    ));
    let (scheduler, tasks) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service = SchedulerBackedAthleteSummaryService::new(
        direct,
        scheduler.clone(),
        TestIdGenerator::default(),
    );
    let error_future = tokio::spawn(async move { service.generate_summary(USER_ID, false).await });

    let first_retry = wait_for_only_task_status(&tasks, TaskStatus::RetryScheduled, Some(1)).await;
    assert_eq!(
        first_retry.next_attempt_at_epoch_seconds,
        NOW_EPOCH_SECONDS + 30
    );

    clock.set_now(NOW_EPOCH_SECONDS + 30);

    let second_retry = wait_for_only_task_status(&tasks, TaskStatus::RetryScheduled, Some(2)).await;
    assert_eq!(
        second_retry.next_attempt_at_epoch_seconds,
        NOW_EPOCH_SECONDS + 60
    );

    clock.set_now(NOW_EPOCH_SECONDS + 60);

    let error = tokio::time::timeout(std::time::Duration::from_secs(2), error_future)
        .await
        .expect("scheduler-backed generate should stop waiting after terminal failure")
        .expect("generate task join should succeed")
        .expect_err("scheduler-backed generate should surface the final retryable error");
    let task = tasks.only_task();

    assert_eq!(task.status, TaskStatus::Failed);
    assert_eq!(task.attempt_count, 3);
    assert_eq!(
        error,
        AthleteSummaryError::Llm(LlmError::RateLimited("slow down".to_string()))
    );
    assert_eq!(generator.call_count(), 3);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_summary_returns_non_retryable_llm_failure() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let generator = StubGenerator::failing(LlmError::ProviderRejected("invalid model".to_string()));
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::default(),
        InMemoryAthleteSummaryOperationRepository::default(),
        generator.clone(),
        clock.clone(),
    ));
    let (scheduler, tasks) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service = SchedulerBackedAthleteSummaryService::new(
        direct,
        scheduler.clone(),
        TestIdGenerator::default(),
    );

    let error = service.generate_summary(USER_ID, false).await.unwrap_err();
    let task = tasks.only_task();

    assert_eq!(task.status, TaskStatus::Failed);
    assert_eq!(task.attempt_count, 1);
    assert_eq!(
        error,
        AthleteSummaryError::Llm(LlmError::ProviderRejected("invalid model".to_string()))
    );
    assert_eq!(generator.call_count(), 1);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_summary_waits_for_pending_operation_reclaim_window() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let operations = InMemoryAthleteSummaryOperationRepository::default();
    operations.seed(AthleteSummaryGenerationOperation {
        user_id: USER_ID.to_string(),
        status: AthleteSummaryGenerationOperationStatus::Pending,
        summary_text: None,
        provider: None,
        model: None,
        error_message: None,
        started_at_epoch_seconds: NOW_EPOCH_SECONDS,
        last_attempt_at_epoch_seconds: NOW_EPOCH_SECONDS,
        attempt_count: 1,
        created_at_epoch_seconds: NOW_EPOCH_SECONDS,
        updated_at_epoch_seconds: NOW_EPOCH_SECONDS,
    });
    let generator = StubGenerator::succeeds_with("generated after reclaim");
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::default(),
        operations,
        generator.clone(),
        clock.clone(),
    ));
    let (scheduler, tasks) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service = SchedulerBackedAthleteSummaryService::new(
        direct,
        scheduler.clone(),
        TestIdGenerator::default(),
    );
    let summary_future =
        tokio::spawn(async move { service.generate_summary(USER_ID, false).await });

    let pending_retry =
        wait_for_only_task_status(&tasks, TaskStatus::RetryScheduled, Some(1)).await;
    assert_eq!(
        pending_retry.error_message.as_deref(),
        Some(GENERATION_ALREADY_PENDING_MESSAGE)
    );
    assert_eq!(
        pending_retry.next_attempt_at_epoch_seconds,
        NOW_EPOCH_SECONDS + STALE_PENDING_TIMEOUT_SECONDS
    );

    clock.set_now(NOW_EPOCH_SECONDS + STALE_PENDING_TIMEOUT_SECONDS);

    let summary = tokio::time::timeout(std::time::Duration::from_secs(2), summary_future)
        .await
        .expect("scheduler-backed generate should finish after the reclaim window opens")
        .expect("generate task join should succeed")
        .expect("scheduler-backed generate should succeed after reclaim");
    let task = tasks.only_task();

    assert_eq!(summary.summary_text, "generated after reclaim");
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.attempt_count, 2);
    assert_eq!(generator.call_count(), 1);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_summary_allows_repeated_force_requests() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let generator = StubGenerator::queued(vec![
        Ok(llm_response("forced-1")),
        Ok(llm_response("forced-2")),
    ]);
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::default(),
        InMemoryAthleteSummaryOperationRepository::default(),
        generator.clone(),
        clock.clone(),
    ));
    let (scheduler, _) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service =
        SchedulerBackedAthleteSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let first = service.generate_summary(USER_ID, true).await.unwrap();
    let second = service.generate_summary(USER_ID, true).await.unwrap();

    assert_eq!(first.summary_text, "forced-1");
    assert_eq!(second.summary_text, "forced-2");
    assert_eq!(generator.call_count(), 2);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_ensure_fresh_summary_state_reports_regeneration() {
    let clock = TestClock::new(NOW_EPOCH_SECONDS);
    let direct = Arc::new(AthleteSummaryService::new(
        InMemoryAthleteSummaryRepository::with_summary(summary("old", LAST_WEEK_EPOCH_SECONDS)),
        InMemoryAthleteSummaryOperationRepository::default(),
        StubGenerator::succeeds_with("refreshed"),
        clock.clone(),
    ));
    let (scheduler, _) = test_scheduler_with_clock(clock.clone());
    let worker = crate::config::spawn_task_worker(
        scheduler.clone(),
        "worker-1".to_string(),
        worker_config(),
        vec![athlete_summary_generate_task_handler(direct.clone())],
    )
    .unwrap();
    let service =
        SchedulerBackedAthleteSummaryService::new(direct, scheduler, TestIdGenerator::default());

    let ensured = service.ensure_fresh_summary_state(USER_ID).await.unwrap();

    assert!(ensured.was_regenerated);
    assert_eq!(ensured.summary.summary_text, "refreshed");
    worker.shutdown().await;
}

fn test_scheduler() -> (
    TaskSchedulerService<InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock>,
    InMemoryTaskRepository,
) {
    test_scheduler_with_clock(TestClock::new(NOW_EPOCH_SECONDS))
}

fn test_scheduler_with_clock(
    clock: TestClock,
) -> (
    TaskSchedulerService<InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock>,
    InMemoryTaskRepository,
) {
    let tasks = InMemoryTaskRepository::default();
    (
        TaskSchedulerService::new(
            tasks.clone(),
            InMemoryTaskWorkerRepository::default(),
            clock,
        ),
        tasks,
    )
}

fn worker_config() -> TaskWorkerConfig {
    TaskWorkerConfig {
        is_leader: false,
        lease_duration_seconds: 30,
        heartbeat_interval: std::time::Duration::from_secs(10),
        idle_poll_interval: std::time::Duration::from_millis(10),
        max_concurrency: 2,
    }
}

async fn wait_for_only_task_status(
    tasks: &InMemoryTaskRepository,
    expected_status: TaskStatus,
    expected_attempt_count: Option<u32>,
) -> ScheduledTask {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let Some(task) = tasks.only_task_if_present() else {
                tokio::task::yield_now().await;
                continue;
            };

            if task.status == expected_status
                && expected_attempt_count
                    .map(|attempt_count| task.attempt_count == attempt_count)
                    .unwrap_or(true)
            {
                return task;
            }

            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("task should reach expected state")
}
