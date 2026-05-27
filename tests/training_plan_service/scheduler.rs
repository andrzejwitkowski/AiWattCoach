use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use aiwattcoach::{
    config::spawn_task_worker,
    domain::{
        calendar_view::{
            CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
            NoopCalendarEntryViewRefresh,
        },
        identity::IdGenerator,
        task_scheduler::{
            ScheduledTask, TaskListFilter, TaskSchedulerService, TaskStatus, TaskWorkerConfig,
        },
        training_plan::{
            training_plan_generate_task_handler, SchedulerBackedTrainingPlanService,
            TrainingPlanError, TrainingPlanGenerationService, TrainingPlanGenerator,
            TrainingPlanPhaseOutput, TrainingPlanPlanningContext, TrainingPlanUseCases,
        },
        workout_summary::WorkoutRecap,
    },
};
use tokio::sync::Notify;

use crate::{
    task_scheduler_clock_support::TestClock as SchedulerClock,
    task_scheduler_repository_support::{InMemoryTaskRepository, InMemoryTaskWorkerRepository},
};

use super::support::*;

type Scheduler =
    TaskSchedulerService<InMemoryTaskRepository, InMemoryTaskWorkerRepository, SchedulerClock>;
type DirectService<Generator, Refresh = NoopCalendarEntryViewRefresh> =
    TrainingPlanGenerationService<
        InMemoryTrainingPlanSnapshotRepository,
        InMemoryTrainingPlanProjectedDayRepository,
        InMemoryTrainingPlanOperationRepository,
        Generator,
        StubWorkoutSummaryPort,
        SchedulerClock,
        Refresh,
    >;
type WrappedService<Generator, Refresh = NoopCalendarEntryViewRefresh> =
    SchedulerBackedTrainingPlanService<
        DirectService<Generator, Refresh>,
        InMemoryTaskRepository,
        InMemoryTaskWorkerRepository,
        SchedulerClock,
        TestIdGenerator,
    >;

struct SchedulerFixture<Generator, Refresh = NoopCalendarEntryViewRefresh>
where
    Generator: TrainingPlanGenerator + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    direct: Arc<DirectService<Generator, Refresh>>,
    wrapped: WrappedService<Generator, Refresh>,
    scheduler: Scheduler,
    clock: SchedulerClock,
    projected_days: InMemoryTrainingPlanProjectedDayRepository,
    operations: InMemoryTrainingPlanOperationRepository,
}

#[tokio::test]
async fn scheduler_backed_generate_for_saved_workout_runs_through_shared_worker_and_replays() {
    let call_log = new_call_log();
    let generator = StubTrainingPlanGenerator::new(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let fixture = build_fixture_with_generator(
        date_epoch(FIRST_DAY),
        call_log,
        generator.clone(),
        NoopCalendarEntryViewRefresh,
    );
    let worker = spawn_worker(&fixture, "worker-1");

    let first = fixture
        .wrapped
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();
    let replay = fixture
        .wrapped
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();
    let task = only_task(&fixture.scheduler).await;

    assert!(first.was_generated);
    assert!(!replay.was_generated);
    assert_eq!(first.snapshot.operation_key, replay.snapshot.operation_key);
    assert_eq!(generator.recap_call_count(), 1);
    assert_eq!(generator.initial_plan_call_count(), 1);
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.attempt_count, 1);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_for_saved_workout_preserves_training_plan_error_category() {
    let call_log = new_call_log();
    let generator = StubTrainingPlanGenerator::new(
        call_log.clone(),
        vec![Err(TrainingPlanError::Validation(
            "recap generation failed".to_string(),
        ))],
        vec![],
        vec![],
    );
    let fixture = build_fixture_with_generator(
        date_epoch(FIRST_DAY),
        call_log,
        generator,
        NoopCalendarEntryViewRefresh,
    );
    let worker = spawn_worker(&fixture, "worker-1");

    let error = fixture
        .wrapped
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();
    let task = only_task(&fixture.scheduler).await;

    assert_eq!(
        error,
        TrainingPlanError::Validation("recap generation failed".to_string())
    );
    assert_eq!(task.status, TaskStatus::Failed);
    assert_eq!(task.attempt_count, 1);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_for_saved_workout_recovers_after_worker_restart() {
    let generator = BlockingTrainingPlanGenerator::default();
    let fixture = build_fixture_with_generator(
        date_epoch(FIRST_DAY),
        new_call_log(),
        generator.clone(),
        NoopCalendarEntryViewRefresh,
    );
    let worker = spawn_worker(&fixture, "worker-1");
    let wrapped = fixture.wrapped.clone();
    let result_future = tokio::spawn(async move {
        wrapped
            .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
            .await
    });

    wait_for_only_task_status(&fixture.scheduler, TaskStatus::Running, Some(1)).await;
    worker.shutdown().await;

    fixture.clock.set_now(date_epoch(FIRST_DAY) + 600);
    fixture
        .scheduler
        .touch_worker_heartbeat(
            "worker-1",
            false,
            vec!["training_plan.generate_for_saved_workout".to_string()],
        )
        .await
        .unwrap();
    let recovered = fixture
        .scheduler
        .sweep_timed_out_tasks(30, 100)
        .await
        .unwrap();
    let worker = spawn_worker(&fixture, "worker-1");
    generator.release_recap();

    let result = tokio::time::timeout(Duration::from_secs(2), result_future)
        .await
        .expect("scheduler-backed training plan should finish after recovery")
        .expect("training plan join should succeed")
        .expect("training plan should succeed after recovery");
    let task = wait_for_only_task_status(&fixture.scheduler, TaskStatus::Completed, Some(2)).await;

    assert_eq!(recovered, 1);
    assert_eq!(
        result.snapshot.operation_key,
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        )
    );
    assert_eq!(task.attempt_count, 2);
    assert!(generator.recap_call_count() >= 1);
    worker.shutdown().await;
}

#[tokio::test]
async fn scheduler_backed_generate_for_saved_workout_retries_after_panicked_attempt() {
    let generator = PanicOnceTrainingPlanGenerator::default();
    let fixture = build_fixture_with_generator(
        date_epoch(FIRST_DAY),
        new_call_log(),
        generator.clone(),
        NoopCalendarEntryViewRefresh,
    );
    let worker = spawn_worker(&fixture, "worker-1");
    let wrapped = fixture.wrapped.clone();
    let result_future = tokio::spawn(async move {
        wrapped
            .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
            .await
    });

    let retried_task =
        wait_for_only_task_status(&fixture.scheduler, TaskStatus::RetryScheduled, Some(1)).await;

    assert_eq!(
        retried_task.error_message.as_deref(),
        Some("scheduled task handler panicked")
    );
    assert_eq!(
        retried_task.next_attempt_at_epoch_seconds,
        date_epoch(FIRST_DAY) + 300
    );

    fixture
        .clock
        .set_now(retried_task.next_attempt_at_epoch_seconds);

    let result = tokio::time::timeout(Duration::from_secs(2), result_future)
        .await
        .expect("scheduler-backed training plan should finish after retry")
        .expect("training plan join should succeed")
        .expect("training plan should succeed after panic retry");
    let completed_task =
        wait_for_only_task_status(&fixture.scheduler, TaskStatus::Completed, Some(2)).await;

    assert!(result.was_generated);
    assert_eq!(completed_task.status, TaskStatus::Completed);
    assert_eq!(generator.recap_call_count(), 2);
    worker.shutdown().await;
}

#[tokio::test]
async fn manual_retry_of_failed_task_does_not_duplicate_projected_days() {
    let call_log = new_call_log();
    let generator = StubTrainingPlanGenerator::new(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let fixture = build_fixture_with_generator(
        date_epoch(FIRST_DAY),
        call_log,
        generator,
        FailingCalendarRefresh,
    );
    let worker = spawn_worker(&fixture, "worker-1");

    let error = fixture
        .wrapped
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();
    let failed_task =
        wait_for_only_task_status(&fixture.scheduler, TaskStatus::Failed, Some(1)).await;
    let projected_day_count_before_retry = fixture.projected_days.stored_days().len();

    assert_eq!(
        error,
        TrainingPlanError::Repository("refresh unavailable".to_string())
    );

    fixture.scheduler.retry_task(&failed_task.id).await.unwrap();

    let completed_task =
        wait_for_only_task_status(&fixture.scheduler, TaskStatus::Completed, Some(2)).await;
    let projected_day_count_after_retry = fixture.projected_days.stored_days().len();
    let operation = fixture.operations.stored_operation();

    assert_eq!(completed_task.status, TaskStatus::Completed);
    assert_eq!(projected_day_count_before_retry, 14);
    assert_eq!(
        projected_day_count_after_retry,
        projected_day_count_before_retry
    );
    assert_eq!(operation.status, WorkflowStatus::Completed);
    worker.shutdown().await;
}

fn build_fixture_with_generator<Generator, Refresh>(
    clock_now: i64,
    call_log: CallLog,
    generator: Generator,
    refresh: Refresh,
) -> SchedulerFixture<Generator, Refresh>
where
    Generator: TrainingPlanGenerator + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    let clock = SchedulerClock::new(clock_now);
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log);
    let direct = Arc::new(
        TrainingPlanGenerationService::new(
            snapshots,
            projected_days.clone(),
            operations.clone(),
            generator,
            workout_summary,
            clock.clone(),
        )
        .with_calendar_view_refresh(refresh),
    );
    let scheduler = TaskSchedulerService::new(
        InMemoryTaskRepository::default(),
        InMemoryTaskWorkerRepository::default(),
        clock.clone(),
    );
    let wrapped = SchedulerBackedTrainingPlanService::new(
        direct.clone(),
        scheduler.clone(),
        TestIdGenerator::default(),
    );

    SchedulerFixture {
        direct,
        wrapped,
        scheduler,
        clock,
        projected_days,
        operations,
    }
}

fn spawn_worker<Generator, Refresh>(
    fixture: &SchedulerFixture<Generator, Refresh>,
    worker_id: &str,
) -> aiwattcoach::BackgroundTaskHandle
where
    Generator: TrainingPlanGenerator + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    spawn_task_worker(
        fixture.scheduler.clone(),
        worker_id.to_string(),
        TaskWorkerConfig {
            is_leader: false,
            lease_duration_seconds: 30,
            heartbeat_interval: Duration::from_secs(10),
            idle_poll_interval: Duration::from_millis(10),
            max_concurrency: 2,
        },
        vec![training_plan_generate_task_handler(fixture.direct.clone())],
    )
    .unwrap()
}

async fn wait_for_only_task_status(
    scheduler: &Scheduler,
    expected_status: TaskStatus,
    expected_attempt_count: Option<u32>,
) -> ScheduledTask {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let tasks = scheduler
                .list_tasks(TaskListFilter::default())
                .await
                .unwrap();
            let Some(task) = tasks.tasks.into_iter().next() else {
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

async fn only_task(scheduler: &Scheduler) -> ScheduledTask {
    scheduler
        .list_tasks(TaskListFilter::default())
        .await
        .unwrap()
        .tasks
        .into_iter()
        .next()
        .expect("expected scheduled training plan task")
}

#[derive(Clone, Default)]
struct TestIdGenerator {
    next_id: Arc<AtomicUsize>,
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}-{next_id}")
    }
}

#[derive(Clone, Default)]
struct BlockingTrainingPlanGenerator {
    recap_calls: Arc<AtomicUsize>,
    recap_released: Arc<AtomicBool>,
    recap_notify: Arc<Notify>,
}

impl BlockingTrainingPlanGenerator {
    fn release_recap(&self) {
        self.recap_released.store(true, Ordering::SeqCst);
        self.recap_notify.notify_waiters();
    }

    fn recap_call_count(&self) -> usize {
        self.recap_calls.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Default)]
struct PanicOnceTrainingPlanGenerator {
    recap_calls: Arc<AtomicUsize>,
}

impl PanicOnceTrainingPlanGenerator {
    fn recap_call_count(&self) -> usize {
        self.recap_calls.load(Ordering::SeqCst)
    }
}

impl TrainingPlanGenerator for PanicOnceTrainingPlanGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        let recap_calls = self.recap_calls.clone();
        Box::pin(async move {
            if recap_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("panic-once training plan generator");
            }
            Ok(workout_recap())
        })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        Box::pin(async move {
            Ok(TrainingPlanPhaseOutput {
                raw_response: valid_plan_window(FIRST_DAY),
                description: None,
                tool_loop_state: aiwattcoach::domain::llm_tools::LlmToolLoopState::default(),
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _invalid_day_sections: &str,
        _issues: Vec<ValidationIssue>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        Box::pin(
            async move { unreachable!("correction should not run in panic-once generator test") },
        )
    }
}

impl TrainingPlanGenerator for BlockingTrainingPlanGenerator {
    fn generate_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<WorkoutRecap, TrainingPlanError>>
    {
        let recap_calls = self.recap_calls.clone();
        let recap_released = self.recap_released.clone();
        let recap_notify = self.recap_notify.clone();
        Box::pin(async move {
            recap_calls.fetch_add(1, Ordering::SeqCst);
            while !recap_released.load(Ordering::SeqCst) {
                recap_notify.notified().await;
            }
            Ok(workout_recap())
        })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        Box::pin(async move {
            Ok(TrainingPlanPhaseOutput {
                raw_response: valid_plan_window(FIRST_DAY),
                description: None,
                tool_loop_state: aiwattcoach::domain::llm_tools::LlmToolLoopState::default(),
            })
        })
    }

    fn correct_invalid_days_with_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: i64,
        _workout_recap: &WorkoutRecap,
        _planning_context: Option<&TrainingPlanPlanningContext>,
        _invalid_day_sections: &str,
        _issues: Vec<ValidationIssue>,
        _restored_state: Option<aiwattcoach::domain::llm_tools::LlmToolLoopState>,
        _checkpoint: Option<aiwattcoach::domain::training_plan::TrainingPlanToolLoopCheckpoint>,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanPhaseOutput, TrainingPlanError>,
    > {
        Box::pin(
            async move { unreachable!("correction should not run in blocking generator test") },
        )
    }
}

#[derive(Clone)]
struct FailingCalendarRefresh;

impl CalendarEntryViewRefreshPort for FailingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        Box::pin(async {
            Err(CalendarEntryViewError::Repository(
                "refresh unavailable".to_string(),
            ))
        })
    }
}
