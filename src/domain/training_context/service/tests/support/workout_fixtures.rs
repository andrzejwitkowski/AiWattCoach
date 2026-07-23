use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream, CompletedWorkoutZoneTime,
    },
    intervals::{
        PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
        PlannedWorkoutTarget, PlannedWorkoutText,
    },
    planned_workouts::{
        PlannedWorkout as CanonicalPlannedWorkout,
        PlannedWorkoutContent as CanonicalPlannedWorkoutContent,
        PlannedWorkoutLine as CanonicalPlannedWorkoutLine,
        PlannedWorkoutStep as CanonicalPlannedWorkoutStep,
        PlannedWorkoutStepKind as CanonicalPlannedWorkoutStepKind,
        PlannedWorkoutTarget as CanonicalPlannedWorkoutTarget,
        PlannedWorkoutText as CanonicalPlannedWorkoutText,
    },
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanSnapshot,
    },
};

#[derive(Clone)]
pub(crate) struct TestTrainingPlanProjectionRepository;

impl TrainingPlanProjectionRepository for TestTrainingPlanProjectionRepository {
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
                    workout_id: "ride-1".to_string(),
                    operation_key: "training-plan:user-1:ride-1:1775174400".to_string(),
                    date: "2026-04-04".to_string(),
                    rest_day: false,
                    rest_day_reason: None,
                    workout: Some(PlannedWorkout {
                        lines: vec![
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "Past AI Threshold".to_string(),
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 600,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 92.0,
                                    max: 97.0,
                                },
                            }),
                        ],
                    }),
                    superseded_at_epoch_seconds: None,
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                },
                TrainingPlanProjectedDay {
                    user_id: "user-1".to_string(),
                    workout_id: "ride-1".to_string(),
                    operation_key: "training-plan:user-1:ride-1:1775174400".to_string(),
                    date: "2026-04-07".to_string(),
                    rest_day: false,
                    rest_day_reason: None,
                    workout: Some(PlannedWorkout {
                        lines: vec![
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "AI Threshold".to_string(),
                            }),
                            PlannedWorkoutLine::Repeat(
                                crate::domain::intervals::PlannedWorkoutRepeat {
                                    title: Some("Main Set".to_string()),
                                    count: 2,
                                },
                            ),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 600,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 92.0,
                                    max: 97.0,
                                },
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 180,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 55.0,
                                    max: 55.0,
                                },
                            }),
                            PlannedWorkoutLine::Text(PlannedWorkoutText {
                                text: "Cooldown".to_string(),
                            }),
                            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                                duration_seconds: 300,
                                kind: PlannedWorkoutStepKind::Steady,
                                target: PlannedWorkoutTarget::PercentFtp {
                                    min: 50.0,
                                    max: 50.0,
                                },
                            }),
                        ],
                    }),
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
}

pub(crate) fn sample_completed_workout_on_date_with_ftp(
    id: &str,
    start_date_local: &str,
    ftp_watts: Option<i32>,
    planned_workout_id: Option<String>,
) -> CompletedWorkout {
    CompletedWorkout::new(
        format!("intervals-activity:{id}"),
        "user-1".to_string(),
        start_date_local.to_string(),
        Some(id.to_string()),
        planned_workout_id,
        Some("Sweet Spot".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        Some(3600),
        None,
        CompletedWorkoutMetrics {
            training_stress_score: Some(80),
            normalized_power_watts: Some(250),
            intensity_factor: Some(0.83),
            efficiency_factor: Some(1.2),
            variability_index: Some(1.05),
            average_power_watts: Some(238),
            ftp_watts,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: vec![
                crate::domain::completed_workouts::CompletedWorkoutInterval {
                    id: Some(1),
                    label: Some("Work 1".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(600),
                    end_index: Some(1200),
                    start_time_seconds: Some(600),
                    end_time_seconds: Some(1200),
                    moving_time_seconds: Some(600),
                    elapsed_time_seconds: Some(600),
                    distance_meters: None,
                    average_power_watts: Some(278),
                    normalized_power_watts: Some(280),
                    training_stress_score: Some(20.0),
                    average_heart_rate_bpm: None,
                    average_cadence_rpm: None,
                    average_speed_mps: None,
                    average_stride_meters: None,
                    zone: Some(4),
                },
                crate::domain::completed_workouts::CompletedWorkoutInterval {
                    id: Some(2),
                    label: Some("Work 2".to_string()),
                    interval_type: Some("WORK".to_string()),
                    group_id: Some("g1".to_string()),
                    start_index: Some(1500),
                    end_index: Some(2100),
                    start_time_seconds: Some(1500),
                    end_time_seconds: Some(2100),
                    moving_time_seconds: Some(600),
                    elapsed_time_seconds: Some(600),
                    distance_meters: None,
                    average_power_watts: Some(279),
                    normalized_power_watts: Some(281),
                    training_stress_score: Some(20.0),
                    average_heart_rate_bpm: None,
                    average_cadence_rpm: None,
                    average_speed_mps: None,
                    average_stride_meters: None,
                    zone: Some(4),
                },
            ],
            interval_groups: Vec::new(),
            streams: vec![
                CompletedWorkoutStream {
                    stream_type: "watts".to_string(),
                    name: None,
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                        200, 220, 240, 260, 280,
                    ])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
                CompletedWorkoutStream {
                    stream_type: "cadence".to_string(),
                    name: None,
                    primary_series: Some(CompletedWorkoutSeries::Integers(vec![
                        80, 82, 84, 86, 88,
                    ])),
                    secondary_series: None,
                    value_type_is_array: false,
                    custom: false,
                    all_null: false,
                },
            ],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z4".to_string(),
                seconds: 1200,
            }],
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

pub(crate) fn sample_planned_workout(event_id: i64, date: &str) -> CanonicalPlannedWorkout {
    let (name, description, workout_doc) = if event_id == 303 {
        (
            "Long Tempo",
            Some("Endurance with tempo finish".to_string()),
            "- 90m 75%",
        )
    } else {
        ("Sweet Spot", None, "- 2x10min 90-95%")
    };

    CanonicalPlannedWorkout::new(
        format!("intervals-event:{event_id}"),
        "user-1".to_string(),
        date.to_string(),
        CanonicalPlannedWorkoutContent {
            lines: vec![
                CanonicalPlannedWorkoutLine::Text(CanonicalPlannedWorkoutText {
                    text: name.to_string(),
                }),
                CanonicalPlannedWorkoutLine::Step(CanonicalPlannedWorkoutStep {
                    duration_seconds: if event_id == 303 { 5400 } else { 1200 },
                    kind: CanonicalPlannedWorkoutStepKind::Steady,
                    target: CanonicalPlannedWorkoutTarget::PercentFtp {
                        min: if event_id == 303 { 75.0 } else { 90.0 },
                        max: if event_id == 303 { 75.0 } else { 95.0 },
                    },
                }),
            ],
        },
    )
    .with_event_metadata(
        Some(name.to_string()),
        description.or_else(|| (event_id != 101).then(|| workout_doc.to_string())),
        Some("Ride".to_string()),
    )
}
