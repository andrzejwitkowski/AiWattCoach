use std::sync::Arc;

use crate::domain::{
    training_context::TrainingContextBuilder,
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanSnapshot,
    },
};

use super::{
    super::DefaultTrainingContextBuilder,
    support::{
        sample_completed_workout_on_date_with_ftp, FixedClock, TestCompletedWorkoutRepository,
        TestPlannedWorkoutRepository, TestRaceRepository, TestSettingsService,
        TestSpecialDayRepository, TestTrainingPlanProjectionRepository,
        TestWorkoutSummaryRepository,
    },
};

#[tokio::test]
async fn builder_renders_recent_and_historical_context() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default())
    .with_race_repository(Arc::new(TestRaceRepository))
    .with_training_plan_projection_repository(Arc::new(TestTrainingPlanProjectionRepository));

    let result = builder.build("user-1", "ride-1").await.unwrap();

    assert_eq!(result.context.focus_kind, "activity");
    assert_eq!(result.context.intervals_status.activities, "ok");
    assert_eq!(result.context.intervals_status.events, "ok");
    assert_eq!(result.context.races.len(), 1);
    assert_eq!(result.context.races[0].date, "2026-05-10");
    assert_eq!(result.context.races[0].name, "Spring Classic");
    assert_eq!(result.context.races[0].discipline, "road");
    assert_eq!(result.context.races[0].priority, "A");
    assert_eq!(result.context.future_events.len(), 1);
    assert_eq!(result.context.future_events[0].event_id, 303);
    assert_eq!(result.context.future_events[0].category, "WORKOUT");
    assert_eq!(
        result.context.future_events[0].event_type.as_deref(),
        Some("Ride")
    );
    assert_eq!(
        result.context.future_events[0].name.as_deref(),
        Some("Long Tempo")
    );
    assert_eq!(
        result.context.future_events[0].description.as_deref(),
        Some("Endurance with tempo finish")
    );
    assert_eq!(
        result.context.future_events[0].estimated_duration_seconds,
        Some(5400)
    );
    assert_eq!(
        result.context.future_events[0].estimated_normalized_power_watts,
        Some(225)
    );
    assert_eq!(
        result.context.future_events[0].estimated_intensity_factor,
        Some(0.75)
    );
    assert_eq!(result.context.recent_days.len(), 14);
    assert_eq!(result.context.history.load_trend.len(), 42);
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .first()
            .map(|point| point.sample_days),
        Some(1)
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .first()
            .map(|point| point.date.as_str()),
        Some("2026-02-21")
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .map(|point| point.sample_days),
        Some(1)
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .map(|point| point.period_tss),
        Some(80)
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .and_then(|point| point.rolling_tss_7d),
        Some(11.43)
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .and_then(|point| point.rolling_tss_28d),
        Some(2.86)
    );
    assert_eq!(
        result
            .context
            .history
            .load_trend
            .last()
            .and_then(|point| point.ctl),
        Some(1.9)
    );
    let recent_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-03")
        .expect("recent day should exist");
    assert_eq!(recent_day.workouts.len(), 1);
    assert!(!recent_day.sick_day);
    assert_eq!(recent_day.workouts[0].rpe, Some(7));
    assert_eq!(
        recent_day.workouts[0].workout_recap.as_deref(),
        Some("Strong sweet spot execution with steady control")
    );
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
            .map(|planned| planned
                .interval_blocks
                .iter()
                .map(|block| block.duration_seconds)
                .sum::<i32>()),
        Some(1200)
    );
    assert_eq!(
        recent_day.workouts[0]
            .planned_workout
            .as_ref()
            .map(|planned| planned.interval_blocks.len()),
        Some(1)
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
    let sick_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-02")
        .expect("sick day should exist");
    assert!(sick_day.sick_day);
    assert_eq!(
        sick_day.sick_note.as_deref(),
        Some("Sick day Felt unwell with sore throat")
    );
    assert!(result
        .rendered
        .stable_context
        .contains("prefers concise coaching"));
    assert!(result.rendered.stable_context.contains("\"lt\":["));
    assert!(result
        .rendered
        .stable_context
        .contains("\"rc\":[{\"id\":\"race-1\",\"d\":\"2026-05-10\",\"n\":\"Spring Classic\",\"km\":123.0,\"disc\":\"road\",\"pri\":\"A\"}]"));
    assert!(result
        .rendered
        .stable_context
        .contains("\"fe\":[{\"id\":303,\"sd\":\"2026-04-25T00:00:00\",\"c\":\"WORKOUT\",\"ty\":\"Ride\",\"n\":\"Long Tempo\",\"desc\":\"Endurance with tempo finish\",\"dur\":5400"));
    assert!(result.rendered.stable_context.contains("\"ifv\":0.75"));
    assert!(result.rendered.stable_context.contains("\"np\":225"));
    assert!(result.rendered.stable_context.contains("\"days\":1"));
    assert!(result.rendered.stable_context.contains("\"bl\":["));
    assert!(result
        .rendered
        .stable_context
        .contains("\"recap\":\"Strong sweet spot execution with steady control\""));
    assert!(result
        .rendered
        .stable_context
        .contains("\"pc\":[\"36:1\",\"46:1\",\"57:1\",\"70:1\",\"84:1\"]"));
    assert!(result.rendered.volatile_context.contains("\"ride-1\""));
    assert!(result
        .rendered
        .volatile_context
        .contains("Strong sweet spot execution with steady control"));
    assert!(result.rendered.volatile_context.contains("\"pc\":["));
    assert!(result.rendered.volatile_context.contains("\"tss\":80"));
    assert!(result.rendered.volatile_context.contains("\"pd\":["));
    assert!(result
        .rendered
        .volatile_context
        .contains("\"swid\":\"ride-1\""));
    assert!(!result.rendered.volatile_context.contains("\"p5\":["));
}

#[tokio::test]
async fn build_athlete_summary_context_uses_explicit_summary_focus() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder
        .build_athlete_summary_context("user-1")
        .await
        .unwrap();

    assert_eq!(result.context.focus_kind, "summary");
    assert_eq!(result.context.focus_workout_id, None);
    assert!(result
        .rendered
        .volatile_context
        .contains("\"k\":\"summary\""));
    assert!(result
        .rendered
        .volatile_context
        .contains("\"fx\":{\"k\":\"summary\"}"));
}

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
                        workout: None,
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
                        workout: None,
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
            Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
        > {
            unreachable!()
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
    #[derive(Clone)]
    struct EventIdOnlySummaryRepository;

    impl crate::domain::workout_summary::WorkoutSummaryRepository for EventIdOnlySummaryRepository {
        fn find_by_user_id_and_workout_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<
                Option<crate::domain::workout_summary::WorkoutSummary>,
                crate::domain::workout_summary::WorkoutSummaryError,
            >,
        > {
            Box::pin(async { Ok(None) })
        }

        fn find_by_user_id_and_workout_ids(
            &self,
            _user_id: &str,
            workout_ids: Vec<String>,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<
                Vec<crate::domain::workout_summary::WorkoutSummary>,
                crate::domain::workout_summary::WorkoutSummaryError,
            >,
        > {
            Box::pin(async move {
                Ok(workout_ids
                    .into_iter()
                    .filter(|id| id == "101")
                    .map(|id| crate::domain::workout_summary::WorkoutSummary {
                        id: format!("summary-{id}"),
                        user_id: "user-1".to_string(),
                        workout_id: id,
                        rpe: Some(8),
                        messages: Vec::new(),
                        saved_at_epoch_seconds: None,
                        workout_recap_text: Some("Matched legacy event summary".to_string()),
                        workout_recap_provider: Some("openrouter".to_string()),
                        workout_recap_model: Some("test-model".to_string()),
                        workout_recap_generated_at_epoch_seconds: Some(1),
                        created_at_epoch_seconds: 1,
                        updated_at_epoch_seconds: 1,
                    })
                    .collect())
            })
        }

        fn create(
            &self,
            _summary: crate::domain::workout_summary::WorkoutSummary,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<
                crate::domain::workout_summary::WorkoutSummary,
                crate::domain::workout_summary::WorkoutSummaryError,
            >,
        > {
            unreachable!()
        }

        fn update_rpe(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _rpe: u8,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<(), crate::domain::workout_summary::WorkoutSummaryError>,
        > {
            unreachable!()
        }

        fn append_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message: crate::domain::workout_summary::ConversationMessage,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<(), crate::domain::workout_summary::WorkoutSummaryError>,
        > {
            unreachable!()
        }

        fn find_message_by_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message_id: &str,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<
                Option<crate::domain::workout_summary::ConversationMessage>,
                crate::domain::workout_summary::WorkoutSummaryError,
            >,
        > {
            unreachable!()
        }

        fn set_saved_state(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _saved_at_epoch_seconds: Option<i64>,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<(), crate::domain::workout_summary::WorkoutSummaryError>,
        > {
            unreachable!()
        }

        fn persist_workout_recap(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _recap: crate::domain::workout_summary::WorkoutRecap,
            _updated_at_epoch_seconds: i64,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<(), crate::domain::workout_summary::WorkoutSummaryError>,
        > {
            unreachable!()
        }
    }

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
