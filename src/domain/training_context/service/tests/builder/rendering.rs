use std::sync::Arc;

use crate::domain::{
    completed_workouts::AuthoritativeCompletedWorkoutRepository,
    training_context::TrainingContextBuilder,
};

use super::{
    super::super::DefaultTrainingContextBuilder,
    super::support::{
        sample_completed_workout_on_date_with_ftp, DirectCompletedWorkoutTargetService, FixedClock,
        TestCompletedWorkoutRepository, TestPlannedWorkoutRepository, TestRaceRepository,
        TestSettingsService, TestSpecialDayRepository, TestTrainingPlanProjectionRepository,
        TestWorkoutSummaryRepository,
    },
    wahoo_sync_state, TestSyncStates,
};

#[tokio::test]
async fn builder_renders_recent_and_historical_context() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
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
    assert!(recent_day.planned_workouts.is_empty());
    assert!(!recent_day.sick_day);
    assert_eq!(recent_day.workouts[0].rpe, Some(7));
    assert_eq!(
        recent_day.workouts[0].workout_recap.as_deref(),
        Some("Strong sweet spot execution with steady control")
    );
    assert_eq!(result.context.recent_workout_recaps.len(), 1);
    assert_eq!(
        result.context.recent_workout_recaps[0].recap,
        "Strong sweet spot execution with steady control"
    );
    assert_eq!(recent_day.workouts[0].power_values_3s, vec![220, 270]);
    assert_eq!(recent_day.workouts[0].cadence_values_5s, vec![84]);
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
    assert!(result.rendered.stable_context.contains("\"p3\":[220,270]"));
    assert!(result.rendered.volatile_context.contains("\"ride-1\""));
    assert!(result
        .rendered
        .volatile_context
        .contains("Strong sweet spot execution with steady control"));
    assert!(result.rendered.volatile_context.contains("\"p3\":["));
    assert!(!result.rendered.volatile_context.contains("\"pc\":"));
    assert!(result.rendered.volatile_context.contains("\"c5\":[84]"));
    assert!(result.rendered.volatile_context.contains("\"tss\":80"));
    assert!(result.rendered.volatile_context.contains("\"pd\":["));
    assert!(result
        .rendered
        .volatile_context
        .contains("\"swid\":\"ride-1\""));
    assert!(!result.rendered.volatile_context.contains("\"p5\":["));
    assert!(!result
        .rendered
        .volatile_context
        .contains("\"pw\":[{\"id\":101"));
}

#[tokio::test]
async fn builder_dedups_matched_recent_planned_workout_from_day_plans() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
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

    assert_eq!(recent_day.workouts.len(), 1);
    assert!(recent_day.planned_workouts.is_empty());
    assert_eq!(
        recent_day.workouts[0]
            .planned_workout
            .as_ref()
            .map(|planned| planned.event_id),
        Some(101)
    );
}

#[tokio::test]
async fn builder_prompt_dedupe_keeps_later_duplicate_when_authority_is_already_equal() {
    let mut wahoo = crate::domain::completed_workouts::CompletedWorkout {
        completed_workout_id: "wahoo-workout:ride-1".to_string(),
        source_activity_id: Some("ride-1".to_string()),
        ..sample_completed_workout_on_date_with_ftp(
            "ride-1",
            "2026-04-03T08:00:00",
            Some(305),
            Some("intervals-event:101".to_string()),
        )
    };
    wahoo.name = Some("Wahoo winner".to_string());

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::with_workouts(vec![
        sample_completed_workout_on_date_with_ftp(
            "ride-1",
            "2026-04-03T08:00:00",
            Some(300),
            Some("intervals-event:101".to_string()),
        ),
        wahoo,
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

    assert_eq!(recent_day.workouts.len(), 1);
    assert_eq!(recent_day.workouts[0].name.as_deref(), Some("Wahoo winner"));
    assert_eq!(result.context.history.activity_count, 1);
}

#[tokio::test]
async fn builder_keeps_intervals_completed_workout_when_wahoo_lacks_power_details() {
    let mut detailed_intervals = sample_completed_workout_on_date_with_ftp(
        "ride-1",
        "2026-04-03T08:00:00",
        Some(300),
        Some("intervals-event:101".to_string()),
    );
    let mut sparse_wahoo = crate::domain::completed_workouts::CompletedWorkout {
        completed_workout_id: "wahoo-workout:ride-1".to_string(),
        source_activity_id: Some("ride-1".to_string()),
        ..sample_completed_workout_on_date_with_ftp(
            "ride-1",
            "2026-04-03T08:00:00",
            Some(305),
            Some("intervals-event:101".to_string()),
        )
    };
    sparse_wahoo.details.streams.clear();
    detailed_intervals.name = Some("Intervals winner".to_string());
    let authoritative = AuthoritativeCompletedWorkoutRepository::new(
        TestCompletedWorkoutRepository::with_workouts(vec![detailed_intervals, sparse_wahoo]),
        TestSyncStates {
            states: vec![wahoo_sync_state("wahoo-workout:ride-1")],
        },
    );

    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(authoritative)
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder.build("user-1", "ride-1").await.unwrap();
    let recent_day = result
        .context
        .recent_days
        .iter()
        .find(|day| day.date == "2026-04-03")
        .expect("recent day should exist");

    assert_eq!(recent_day.workouts.len(), 1);
    assert_eq!(
        recent_day.workouts[0].name.as_deref(),
        Some("Intervals winner")
    );
}

#[tokio::test]
async fn build_athlete_summary_context_uses_explicit_summary_focus() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
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
async fn build_calendar_overview_context_uses_calendar_overview_focus() {
    let builder = DefaultTrainingContextBuilder::new(
        Arc::new(TestSettingsService),
        Arc::new(TestWorkoutSummaryRepository),
        Arc::new(DirectCompletedWorkoutTargetService),
        FixedClock,
    )
    .with_completed_workout_repository(TestCompletedWorkoutRepository::default())
    .with_planned_workout_repository(TestPlannedWorkoutRepository::default())
    .with_special_day_repository(TestSpecialDayRepository::default());

    let result = builder
        .build_calendar_overview_context("user-1")
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
