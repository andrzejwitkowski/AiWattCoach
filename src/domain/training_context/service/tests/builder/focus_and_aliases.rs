use std::sync::Arc;

use crate::domain::{
    training_context::TrainingContextBuilder,
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanSnapshot,
    },
};

use super::{
    super::super::DefaultTrainingContextBuilder,
    super::support::{
        sample_completed_workout_on_date_with_ftp, AliasSummaryRepository,
        EventIdOnlySummaryRepository, FixedClock, TestCompletedWorkoutRepository,
        TestPlannedWorkoutRepository, TestSettingsService, TestSpecialDayRepository,
        TestTrainingPlanProjectionRepository, TestWorkoutSummaryRepository,
    },
};

#[tokio::test]
async fn builder_requests_longer_history_warmup_for_load_seed() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.history.window_start, "2025-06-20");
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .first()
            .map(|point| point.date.as_str()),
        Some("2026-02-21")
    );
}

#[tokio::test]
async fn builder_ignores_projected_days_on_or_before_today() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_plan_projection_repository(Arc::new(TestTrainingPlanProjectionRepository));

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert!(result
        .context
        .projected_days
        .iter()
        .all(|day| day.date.as_str() > "2026-04-03"));
}

#[tokio::test]
async fn builder_anchors_windows_to_focus_activity_date() {
    #[derive(Clone)]
    struct OlderFocusProjectionRepository;

    impl TrainingPlanProjectionRepository for OlderFocusProjectionRepository {
        fn list_active_by_user_id(
            &self,
            _user_id: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            Box::pin(async move {
                Ok(vec![
                    TrainingPlanProjectedDay {
                        user_id: "user-1".to_string(),
                        workout_id: "ride-older".to_string(),
                        operation_key: "training-plan:user-1:ride-older:1775174400".to_string(),
                        date: "2026-03-21".to_string(),
                        rest_day: false,
                        rest_day_reason: None,
                        workout: None,
                        supervisor_status: None,
                        superseded_at_epoch_seconds: None,
                        created_at_epoch_seconds: 1,
                        updated_at_epoch_seconds: 1,
                    },
                    TrainingPlanProjectedDay {
                        user_id: "user-1".to_string(),
                        workout_id: "ride-older".to_string(),
                        operation_key: "training-plan:user-1:ride-older:1775174400".to_string(),
                        date: "2026-03-24".to_string(),
                        rest_day: true,
                        rest_day_reason: Some("Scheduled recovery day".to_string()),
                        workout: None,
                        supervisor_status: None,
                        superseded_at_epoch_seconds: None,
                        created_at_epoch_seconds: 1,
                        updated_at_epoch_seconds: 1,
                    },
                ])
            })
        }

        fn find_active_by_operation_key(
            &self,
            _operation_key: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            unreachable!()
        }

        fn find_active_by_user_id_and_operation_key(
            &self,
            _user_id: &str,
            _operation_key: &str,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
        > {
            unreachable!()
        }

        fn replace_window(
            &self,
            _snapshot: TrainingPlanSnapshot,
            _projected_days: Vec<TrainingPlanProjectedDay>,
            _today: &str,
            _replaced_at_epoch_seconds: i64,
        ) -> crate::domain::training_plan::BoxFuture<
            Result<crate::domain::training_plan::TrainingPlanReplacementResult, TrainingPlanError>,
        > {
            unreachable!()
        }

        fn update_supervisor_status(
            &self,
            _user_id: &str,
            _operation_key: &str,
            _supervisor_status: Option<
                crate::domain::training_plan_supervisor::TrainingPlanSupervisorStatus,
            >,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
            Box::pin(async {
                Err(TrainingPlanError::Repository(
                    "focus_and_aliases projection repo should not receive writes".to_string(),
                ))
            })
        }
    }

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-older",
            "2026-03-20T08:00:00",
            Some(300),
            None,
        ),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_plan_projection_repository(Arc::new(OlderFocusProjectionRepository));

    let result = builder.build("user-1", "ride-older").await.unwrap();

    assert_eq!(result.context.focus_kind, "activity");
    assert_eq!(result.context.history.window_end, "2026-03-20");
    assert!(result
        .context
        .recent_days
        .iter()
        .any(|day| day.date == "2026-03-20" && !day.workouts.is_empty()));
    assert_eq!(
        result
            .context
            .projected_days
            .iter()
            .map(|day| day.date.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-03-21", "2026-03-24"]
    );
}

#[tokio::test]
async fn builder_uses_chronological_ftp_change_and_expands_projected_repeats() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-late",
            "2026-04-03T08:00:00",
            Some(320),
            Some("intervals-event:101".to_string()),
        ),
        sample_completed_workout_on_date_with_ftp(
            "ride-early",
            "2026-03-15T08:00:00",
            Some(280),
            None,
        ),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_training_plan_projection_repository(Arc::new(TestTrainingPlanProjectionRepository));

    let result = builder.build("user-1", "ride-late").await.unwrap();

    assert_eq!(result.context.history.ftp_current, Some(320));
    assert_eq!(result.context.history.ftp_change, Some(40));
    assert_eq!(
        result
            .context
            .projected_days
            .iter()
            .find(|day| day.date == "2026-04-07")
            .and_then(|day| day.workouts.first())
            .map(|workout| workout.interval_blocks.len()),
        Some(5)
    );
    assert_eq!(
        result
            .context
            .projected_days
            .iter()
            .find(|day| day.date == "2026-04-07")
            .and_then(|day| day.workouts.first())
            .map(|workout| {
                workout
                    .interval_blocks
                    .iter()
                    .map(|block| block.duration_seconds)
                    .collect::<Vec<_>>()
            }),
        Some(vec![600, 180, 600, 180, 300])
    );
    assert_eq!(
        result
            .context
            .recent_days
            .iter()
            .find(|day| day.date == "2026-04-03")
            .and_then(|day| day.workouts.first())
            .and_then(|workout| workout.planned_workout.as_ref())
            .and_then(|planned| planned.interval_blocks.first())
            .and_then(|block| block.min_target_watts),
        Some(288)
    );
}

#[tokio::test]
async fn builder_falls_back_to_event_id_summary_when_activity_id_summary_is_missing() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(EventIdOnlySummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();
    let recent_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-03")
        .expect("recent day should exist");

    assert_eq!(recent_day.workouts[0].rpe, Some(8));
    assert_eq!(
        recent_day.workouts[0].workout_recap.as_deref(),
        Some("Matched legacy event summary")
    );
}

#[tokio::test]
async fn builder_uses_alias_backed_summary_for_recent_activity_context() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(AliasSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();
    let recent_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-03")
        .expect("recent day should exist");

    assert_eq!(recent_day.workouts[0].rpe, Some(9));
    assert_eq!(
        recent_day.workouts[0].workout_recap.as_deref(),
        Some("Recovered alias-backed recap")
    );
}

#[tokio::test]
async fn builder_uses_configured_ftp_when_activity_ftp_is_missing() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-1",
            "2026-04-03T08:00:00",
            None,
            Some("intervals-event:101".to_string()),
        ),
    ]))
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();
    let recent_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-03")
        .expect("recent day should exist");

    assert_eq!(
        recent_day.workouts[0].compressed_power_levels,
        vec![
            "36:1".to_string(),
            "46:1".to_string(),
            "57:1".to_string(),
            "70:1".to_string(),
            "84:1".to_string(),
        ]
    );
    assert_eq!(
        recent_day.workouts[0]
            .planned_workout
            .as_ref()
            .and_then(|planned| planned.interval_blocks.first())
            .and_then(|block| block.min_target_watts),
        Some(270)
    );
    assert_eq!(
        recent_day.workouts[0]
            .planned_workout
            .as_ref()
            .and_then(|planned| planned.interval_blocks.first())
            .and_then(|block| block.max_target_watts),
        Some(285)
    );
}

#[tokio::test]
async fn builder_marks_event_status_when_stable_future_fetch_fails() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.intervals_status.events, "ok");
    assert_eq!(result.context.upcoming_days.len(), 14);
    assert_eq!(result.context.future_events.len(), 1);
}
