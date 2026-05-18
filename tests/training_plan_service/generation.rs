use super::support::*;
use aiwattcoach::domain::calendar_view::{
    CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort,
};
use aiwattcoach::domain::settings::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    UserSettings, WahooConfig,
};
use aiwattcoach::domain::training_plan_supervisor::{
    TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
    TrainingPlanSupervisorService, TrainingPlanSupervisorStatus,
};

#[derive(Clone, Default)]
struct InMemorySupervisorOperationRepository {
    operations: std::sync::Arc<
        std::sync::Mutex<
            Vec<aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation>,
        >,
    >,
}

impl InMemorySupervisorOperationRepository {
    fn stored_operations(
        &self,
    ) -> Vec<aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation> {
        self.operations.lock().unwrap().clone()
    }

    fn seed_operation(
        &self,
        operation: aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation,
    ) {
        self.operations.lock().unwrap().push(operation);
    }
}

impl TrainingPlanSupervisorOperationRepository for InMemorySupervisorOperationRepository {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> aiwattcoach::domain::training_plan_supervisor::BoxFuture<
        Result<
            Option<aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation>,
            TrainingPlanError,
        >,
    > {
        let operations = self.operations.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            Ok(operations
                .lock()
                .unwrap()
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned())
        })
    }

    fn upsert(
        &self,
        operation: aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation,
    ) -> aiwattcoach::domain::training_plan_supervisor::BoxFuture<
        Result<
            aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation,
            TrainingPlanError,
        >,
    > {
        let operations = self.operations.clone();
        Box::pin(async move {
            let mut operations = operations.lock().unwrap();
            operations
                .retain(|existing| existing.worker_operation_key != operation.worker_operation_key);
            operations.push(operation.clone());
            Ok(operation)
        })
    }

    fn complete_review_if_pending(
        &self,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan_supervisor::BoxFuture<
        Result<
            aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation,
            TrainingPlanError,
        >,
    > {
        let operations = self.operations.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            let mut operations = operations.lock().unwrap();
            let existing = operations
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned()
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;
            let completed = existing.complete_review(review, now_epoch_seconds)?;
            operations
                .retain(|existing| existing.worker_operation_key != completed.worker_operation_key);
            operations.push(completed.clone());
            Ok(completed)
        })
    }
}

#[derive(Clone)]
struct EnabledSettingsService;

impl aiwattcoach::domain::settings::UserSettingsUseCases for EnabledSettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<Option<UserSettings>, aiwattcoach::domain::settings::SettingsError>,
    > {
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(Some(UserSettings {
                user_id,
                ai_agents: AiAgentsConfig {
                    training_plan_supervisor_enabled: true,
                    training_plan_supervisor_model: Some("gemini-2.5-pro".to_string()),
                    ..AiAgentsConfig::default()
                },
                intervals: IntervalsConfig::default(),
                wahoo: WahooConfig::default(),
                options: AnalysisOptions::default(),
                availability: AvailabilitySettings::default(),
                cycling: CyclingSettings::default(),
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }))
        })
    }

    fn get_settings(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("get_settings is not used in this test")
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("update_ai_agents is not used in this test")
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("update_intervals is not used in this test")
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("update_options is not used in this test")
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("update_availability is not used in this test")
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<UserSettings, aiwattcoach::domain::settings::SettingsError>,
    > {
        unreachable!("update_cycling is not used in this test")
    }
}

#[tokio::test]
async fn generates_snapshot_and_projected_days_for_saved_workout() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator.clone(),
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result.was_generated);
    assert_eq!(result.snapshot.days.len(), 14);
    assert_eq!(result.active_projected_days.len(), 13);
    assert_eq!(snapshots.stored_snapshots().len(), 1);
    assert_eq!(
        projected_days
            .stored_days()
            .iter()
            .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > FIRST_DAY)
            .count(),
        13
    );
    assert!(!result
        .active_projected_days
        .iter()
        .any(|day| day.date == FIRST_DAY));

    let operation = operations.stored_operation();
    assert_eq!(operation.status, WorkflowStatus::Completed);
    assert_eq!(
        operation.operation_key,
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        )
    );
}

#[tokio::test]
async fn generation_marks_active_projected_days_pending_when_supervisor_enabled() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let supervisor_operations = InMemorySupervisorOperationRepository::default();
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator,
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    )
    .with_training_plan_supervisor(TrainingPlanSupervisorService::new(
        supervisor_operations.clone(),
        EnabledSettingsService,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    ));

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result
        .active_projected_days
        .iter()
        .all(|day| day.supervisor_status == Some(TrainingPlanSupervisorStatus::Pending)));
    assert_eq!(supervisor_operations.stored_operations().len(), 1);
    assert!(projected_days
        .stored_days()
        .iter()
        .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > FIRST_DAY)
        .all(|day| day.supervisor_status == Some(TrainingPlanSupervisorStatus::Pending)));
}

#[tokio::test]
async fn generation_reuses_existing_supervisor_status_for_same_operation() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let supervisor_operations = InMemorySupervisorOperationRepository::default();
    supervisor_operations.seed_operation(
        aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorOperation {
            worker_operation_key: format!(
                "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
                date_epoch(FIRST_DAY)
            ),
            user_id: USER_ID.to_string(),
            worker_saved_at_epoch_seconds: date_epoch(FIRST_DAY),
            model: "gemini-2.5-pro".to_string(),
            status: TrainingPlanSupervisorStatus::Accepted,
            review: Some(TrainingPlanSupervisorReview {
                decision: aiwattcoach::domain::training_plan_supervisor::TrainingPlanSupervisorDecision::Accept,
                reason: "looks good".to_string(),
                plan: None,
            }),
            created_at_epoch_seconds: date_epoch(FIRST_DAY),
            updated_at_epoch_seconds: date_epoch(FIRST_DAY),
        },
    );
    let service = TrainingPlanGenerationService::new(
        snapshots.clone(),
        projected_days.clone(),
        operations.clone(),
        generator,
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    )
    .with_training_plan_supervisor(TrainingPlanSupervisorService::new(
        supervisor_operations,
        EnabledSettingsService,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    ));

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(result
        .active_projected_days
        .iter()
        .all(|day| day.supervisor_status == Some(TrainingPlanSupervisorStatus::Accepted)));
    assert!(projected_days
        .stored_days()
        .iter()
        .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > FIRST_DAY)
        .all(|day| day.supervisor_status == Some(TrainingPlanSupervisorStatus::Accepted)));
}

#[tokio::test]
async fn persists_workout_recap_before_generating_training_plan_window() {
    let call_log = new_call_log();
    let service = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    service
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "workout_summary.persist_workout_recap",
        "generator.generate_initial_plan_window",
    );
}

#[tokio::test]
async fn passes_planning_conversation_context_to_initial_plan_generation() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );
    built
        .workout_summary
        .set_planning_context(Some(sample_planning_context()));

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let planning_contexts = built.generator.initial_planning_contexts();
    assert_eq!(planning_contexts.len(), 1);
    let planning_context = planning_contexts[0]
        .as_ref()
        .expect("expected planning context for initial generation");
    assert_eq!(planning_context.rpe, Some(6));
    assert_eq!(planning_context.messages.len(), 2);
    assert_eq!(
        planning_context.messages[0].role,
        TrainingPlanConversationRole::Coach
    );
}

#[tokio::test]
async fn checkpoints_recap_in_operation_before_persisting_to_workout_summary() {
    let call_log = new_call_log();
    let built = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_event_order(
        &recorded_calls(&call_log),
        "operation.upsert",
        "workout_summary.persist_workout_recap",
    );
}

#[tokio::test]
async fn replay_of_same_saved_workout_generation_is_idempotent() {
    let call_log = new_call_log();
    let built = build_service(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );

    let first = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();
    let replay = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert!(first.was_generated);
    assert!(!replay.was_generated);
    assert_eq!(first.snapshot.operation_key, replay.snapshot.operation_key);
    assert_eq!(built.generator.recap_call_count(), 1);
    assert_eq!(built.generator.initial_plan_call_count(), 1);
    assert_eq!(built.snapshots.stored_snapshots().len(), 1);
    assert_eq!(built.projected_days.stored_days().len(), 14);
}

#[tokio::test]
async fn existing_pending_operation_returns_unavailable_without_calling_generator() {
    let operation = TrainingPlanGenerationOperation::pending(
        format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        USER_ID.to_string(),
        WORKOUT_ID.to_string(),
        date_epoch(FIRST_DAY),
        date_epoch(SECOND_DAY),
    );
    let built = build_service_with_operation(
        new_call_log(),
        operation,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![Ok(single_rest_day("2026-04-10"))],
        SECOND_DAY,
    );

    let error = built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        TrainingPlanError::Unavailable("training plan generation already in progress".to_string())
    );
    assert_eq!(built.generator.recap_call_count(), 0);
    assert_eq!(built.generator.initial_plan_call_count(), 0);
    assert_eq!(built.generator.correction_call_count(), 0);
}

#[tokio::test]
async fn next_day_generation_supersedes_only_overlapping_future_projected_days() {
    let first = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
        FIRST_DAY,
    );
    first
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let second_generator = StubTrainingPlanGenerator::new(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(SECOND_DAY))],
        vec![],
    );
    let second_service = TrainingPlanGenerationService::new(
        first.snapshots.clone(),
        first.projected_days.clone(),
        first.operations.clone(),
        second_generator,
        first.workout_summary.clone(),
        FixedClock {
            now_epoch_seconds: date_epoch(SECOND_DAY),
        },
    );

    second_service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(SECOND_DAY))
        .await
        .unwrap();

    let stored_days = first.projected_days.stored_days();
    let active_days = stored_days
        .iter()
        .filter(|day| day.superseded_at_epoch_seconds.is_none() && day.date.as_str() > SECOND_DAY)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(active_days.len(), 13);
    assert!(!active_days.iter().any(|day| day.date == FIRST_DAY));
    assert!(!active_days.iter().any(|day| day.date == SECOND_DAY));
    assert!(stored_days.iter().any(|day| {
        day.date == SECOND_DAY
            && day.operation_key
                == format!(
                    "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
                    date_epoch(FIRST_DAY)
                )
            && day.superseded_at_epoch_seconds.is_some()
    }));
}

#[tokio::test]
async fn successful_generation_records_real_workflow_attempts() {
    let built = build_service(
        new_call_log(),
        vec![Ok(workout_recap())],
        vec![Ok(plan_with_invalid_day(FIRST_DAY, "2026-04-10"))],
        vec![Ok(single_rest_day("2026-04-10"))],
        FIRST_DAY,
    );

    built
        .service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    let operation = built.operations.stored_operation();
    let phases = operation
        .attempts
        .iter()
        .map(|attempt| attempt.phase.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            WorkflowPhase::WorkoutRecap,
            WorkflowPhase::InitialGeneration,
            WorkflowPhase::Correction,
            WorkflowPhase::ProjectionUpdate,
        ]
    );
}

#[tokio::test]
async fn generation_refreshes_calendar_view_for_generated_window() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let refresh = RecordingCalendarRefresh::default();
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations,
        generator,
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    )
    .with_calendar_view_refresh(refresh.clone());

    service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_eq!(
        refresh.calls(),
        vec![(
            USER_ID.to_string(),
            FIRST_DAY.to_string(),
            "2026-04-19".to_string(),
        )]
    );
}

#[tokio::test]
async fn generation_fails_when_refresh_fails_after_projection_persistence() {
    let call_log = new_call_log();
    let snapshots = InMemoryTrainingPlanSnapshotRepository::new();
    let projected_days =
        InMemoryTrainingPlanProjectedDayRepository::new(snapshots.snapshots.clone());
    let operations = InMemoryTrainingPlanOperationRepository::new(call_log.clone());
    let workout_summary = StubWorkoutSummaryPort::new(call_log.clone());
    let generator = StubTrainingPlanGenerator::new(
        call_log,
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY))],
        vec![],
    );
    let service = TrainingPlanGenerationService::new(
        snapshots,
        projected_days,
        operations,
        generator,
        workout_summary,
        FixedClock {
            now_epoch_seconds: date_epoch(FIRST_DAY),
        },
    )
    .with_calendar_view_refresh(FailingCalendarRefresh);

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap_err();

    assert_eq!(
        result,
        TrainingPlanError::Repository("refresh unavailable".to_string())
    );
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> aiwattcoach::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push((user_id, oldest, newest));
            Ok(Vec::new())
        })
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
