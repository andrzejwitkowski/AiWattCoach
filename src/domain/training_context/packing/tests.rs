use super::*;
use crate::domain::settings::Weekday;
use crate::domain::training_context::model::{
    AthleteProfileContext, FuturePlannedEventContext, HistoricalLoadTrendPoint,
    HistoricalTrainingContext, HistoricalWorkoutContext, IntervalsStatusContext,
    PlannedWorkoutBlockContext, PlannedWorkoutReference, ProjectedDayContext,
    ProjectedWorkoutContext, RaceContext, RecentDayContext, RecentWorkoutContext,
    RecentWorkoutRecapContext, TrainingContext, WeeklyAvailabilityContext,
};

#[test]
fn compact_render_is_non_empty_and_estimates_tokens() {
    let context = rich_training_context();
    let rendered = render_training_context(&context);

    assert!(rendered
        .stable_context
        .contains("\"ap\":\"Climb-focused athlete\""));
    assert!(rendered.stable_context.contains("\"acfg\":true"));
    assert!(rendered
        .stable_context
        .contains("\"av\":{\"h\":[\"wd\",\"a\",\"mdm\"]"));
    assert!(rendered
        .stable_context
        .contains("\"lt\":{\"h\":[\"d\",\"tss\"],\"r\":[[\"2026-03-31\",42]]}"));
    assert!(rendered.stable_context.contains("480,90.0,95.0,270,285"));
    assert!(!rendered.stable_context.contains("\"ps\":"));
    assert!(!rendered.stable_context.contains("\"cs\":"));
    assert!(rendered.stable_context.contains("\"def_disc\":\"road\""));
    assert!(rendered
        .stable_context
        .contains("\"h\":[\"d\",\"n\",\"km\",\"pri\",\"id\"]"));
    assert!(rendered
        .stable_context
        .contains("\"fe\":{\"h\":[\"id\",\"sd\",\"c\""));
    assert!(rendered.volatile_context.contains("\"sick\":true"));
    assert!(rendered
        .volatile_context
        .contains("\"sickn\":\"felt unwell\""));
    assert!(rendered.volatile_context.contains("\"v\":3"));
    assert!(rendered
        .volatile_context
        .contains("[[220,220,3],[270,270,3]]"));
    assert!(rendered.volatile_context.contains("[[87,87,5]]"));
    assert!(!rendered.volatile_context.contains("\"pc\":"));
    assert!(!rendered.volatile_context.contains("\"p3\":"));
    assert!(rendered
        .volatile_context
        .contains("Held power well and finished controlled"));
    assert!(rendered
        .volatile_context
        .contains("\"wr\":{\"h\":[\"d\",\"id\",\"rpe\",\"recap\"]"));
    assert!(rendered.volatile_context.contains("\"pd\":[{"));
    assert!(rendered.volatile_context.contains("\"workout-1\""));
    assert!(!rendered.volatile_context.contains("\"p5\":"));
    assert!(rendered.approximate_tokens > 0);
}

#[test]
fn v3_payload_is_smaller_than_object_array_baseline() {
    let context = rich_training_context();
    let rendered = render_training_context(&context);
    let combined_len = rendered.stable_context.len() + rendered.volatile_context.len();
    // v2 object-array encoding for the same fixture was ~2.9k chars; v3 should save materially.
    assert!(
        combined_len < 2400,
        "expected compressed payload under 2400 chars, got {combined_len}"
    );
}

fn rich_training_context() -> TrainingContext {
    TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: Some("workout-1".to_string()),
        focus_kind: "activity".to_string(),
        intervals_status: IntervalsStatusContext {
            activities: "ok".to_string(),
            events: "ok".to_string(),
        },
        profile: AthleteProfileContext {
            athlete_prompt: Some("Climb-focused athlete".to_string()),
            availability_configured: true,
            weekly_availability: vec![WeeklyAvailabilityContext {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(90),
            }],
            ..AthleteProfileContext::default()
        },
        races: vec![RaceContext {
            race_id: "race-1".to_string(),
            date: "2026-05-10".to_string(),
            name: "Spring Classic".to_string(),
            distance_meters: 123_000,
            discipline: "road".to_string(),
            priority: "A".to_string(),
        }],
        planned_rest_days: Vec::new(),
        future_events: vec![FuturePlannedEventContext {
            event_id: 303,
            start_date_local: "2026-04-12T07:00:00".to_string(),
            category: "WORKOUT".to_string(),
            event_type: Some("Ride".to_string()),
            name: Some("Long Tempo".to_string()),
            description: Some("Endurance with tempo finish".to_string()),
            estimated_duration_seconds: Some(5400),
            estimated_training_stress_score: Some(92.5),
            estimated_intensity_factor: Some(0.81),
            estimated_normalized_power_watts: Some(243),
        }],
        history: HistoricalTrainingContext {
            window_start: "2025-10-01".to_string(),
            window_end: "2026-04-01".to_string(),
            load_trend: vec![HistoricalLoadTrendPoint {
                date: "2026-03-31".to_string(),
                sample_days: 1,
                period_tss: 42,
                rolling_tss_7d: Some(37.5),
                rolling_tss_28d: Some(51.3),
                ctl: Some(65.2),
                atl: Some(58.6),
                tsb: Some(6.6),
            }],
            workouts: vec![HistoricalWorkoutContext {
                activity_id: "ride-1".to_string(),
                power_segments: vec![[220, 220, 3], [270, 270, 3]],
                cadence_segments: vec![[87, 87, 5]],
                interval_blocks: vec![PlannedWorkoutBlockContext {
                    duration_seconds: 480,
                    min_percent_ftp: Some(90.0),
                    max_percent_ftp: Some(95.0),
                    min_target_watts: Some(270),
                    max_target_watts: Some(285),
                }],
                ..HistoricalWorkoutContext::default()
            }],
            ..HistoricalTrainingContext::default()
        },
        recent_days: vec![RecentDayContext {
            date: "2026-04-01".to_string(),
            sick_day: true,
            sick_note: Some("felt unwell".to_string()),
            workouts: vec![RecentWorkoutContext {
                activity_id: "ride-1".to_string(),
                start_date_local: "2026-04-01T08:00:00".to_string(),
                workout_recap: Some("Held power well and finished controlled".to_string()),
                power_segments: vec![[220, 220, 3], [270, 270, 3]],
                cadence_segments: vec![[87, 87, 5]],
                planned_workout: Some(PlannedWorkoutReference {
                    event_id: 101,
                    start_date_local: "2026-04-01T07:00:00".to_string(),
                    category: "WORKOUT".to_string(),
                    interval_blocks: vec![PlannedWorkoutBlockContext {
                        duration_seconds: 480,
                        min_percent_ftp: Some(90.0),
                        max_percent_ftp: Some(95.0),
                        min_target_watts: Some(270),
                        max_target_watts: Some(285),
                    }],
                    completed: true,
                    ..PlannedWorkoutReference::default()
                }),
                ..RecentWorkoutContext::default()
            }],
            ..RecentDayContext::default()
        }],
        recent_workout_recaps: vec![RecentWorkoutRecapContext {
            date: "2026-04-01".to_string(),
            workout_id: "ride-1".to_string(),
            rpe: Some(7),
            recap: "Held power well and finished controlled".to_string(),
        }],
        upcoming_days: Vec::new(),
        projected_days: vec![ProjectedDayContext {
            date: "2026-04-02".to_string(),
            workouts: vec![ProjectedWorkoutContext {
                source_workout_id: "workout-1".to_string(),
                start_date_local: "2026-04-02T07:00:00".to_string(),
                name: Some("AI Threshold".to_string()),
                interval_blocks: vec![PlannedWorkoutBlockContext {
                    duration_seconds: 600,
                    min_percent_ftp: Some(92.0),
                    max_percent_ftp: Some(97.0),
                    min_target_watts: Some(276),
                    max_target_watts: Some(291),
                }],
                raw_workout_doc: Some("Main Set\n- 10m 92-97%".to_string()),
                rest_day: false,
                rest_day_reason: None,
            }],
        }],
    }
}

#[test]
fn approximate_token_count_is_conservative() {
    assert_eq!(approximate_token_count("abcdef"), 2);
    assert_eq!(approximate_token_count("abcdefg"), 3);
}

#[test]
fn compact_render_omits_nulls_and_empty_lists() {
    let rendered = render_training_context(&TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: None,
        focus_kind: "summary".to_string(),
        intervals_status: IntervalsStatusContext {
            activities: "ok".to_string(),
            events: "ok".to_string(),
        },
        profile: AthleteProfileContext::default(),
        races: Vec::new(),
        planned_rest_days: Vec::new(),
        future_events: Vec::new(),
        history: HistoricalTrainingContext::default(),
        recent_days: Vec::new(),
        recent_workout_recaps: Vec::new(),
        upcoming_days: Vec::new(),
        projected_days: Vec::new(),
    });

    assert!(!rendered.stable_context.contains(":null"));
    assert!(!rendered.stable_context.contains("\"lt\":"));
    assert!(!rendered.volatile_context.contains("\"rd\":[]"));
    assert!(!rendered.volatile_context.contains("\"ud\":[]"));
    assert!(!rendered.volatile_context.contains("\"pd\":[]"));
    assert!(!rendered.volatile_context.contains("\"rs\":"));
}

#[test]
fn compact_render_includes_race_strategy_window_for_next_14_days() {
    let context = TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: None,
        focus_kind: "summary".to_string(),
        intervals_status: IntervalsStatusContext::default(),
        profile: AthleteProfileContext::default(),
        races: vec![
            RaceContext {
                race_id: "race-past".to_string(),
                date: "2026-06-05".to_string(),
                name: "Past Crit".to_string(),
                distance_meters: 40_000,
                discipline: "crit".to_string(),
                priority: "C".to_string(),
            },
            RaceContext {
                race_id: "race-window".to_string(),
                date: "2026-06-20".to_string(),
                name: "Target Road Race".to_string(),
                distance_meters: 120_000,
                discipline: "road".to_string(),
                priority: "B".to_string(),
            },
            RaceContext {
                race_id: "race-future".to_string(),
                date: "2026-07-01".to_string(),
                name: "Later Stage".to_string(),
                distance_meters: 150_000,
                discipline: "road".to_string(),
                priority: "A".to_string(),
            },
        ],
        planned_rest_days: Vec::new(),
        future_events: Vec::new(),
        history: HistoricalTrainingContext::default(),
        recent_days: vec![RecentDayContext {
            date: "2026-06-10".to_string(),
            ..RecentDayContext::default()
        }],
        recent_workout_recaps: Vec::new(),
        upcoming_days: Vec::new(),
        projected_days: Vec::new(),
    };

    let rendered = render_training_context(&context);

    assert!(rendered
        .volatile_context
        .contains("\"rs\":{\"h\":[\"d\",\"pri\",\"disc\",\"n\",\"days_out\"]"));
    assert!(rendered
        .volatile_context
        .contains("[\"2026-06-20\",\"B\",\"road\",\"Target Road Race\",10]"));
    assert!(!rendered.volatile_context.contains("Past Crit"));
    assert!(!rendered.volatile_context.contains("Later Stage"));
}

#[test]
fn compact_render_omits_weekly_availability_when_not_configured() {
    let rendered = render_training_context(&TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: None,
        focus_kind: "summary".to_string(),
        intervals_status: IntervalsStatusContext {
            activities: "ok".to_string(),
            events: "ok".to_string(),
        },
        profile: AthleteProfileContext {
            availability_configured: false,
            weekly_availability: Vec::new(),
            ..AthleteProfileContext::default()
        },
        races: Vec::new(),
        planned_rest_days: Vec::new(),
        future_events: Vec::new(),
        history: HistoricalTrainingContext::default(),
        recent_days: Vec::new(),
        recent_workout_recaps: Vec::new(),
        upcoming_days: Vec::new(),
        projected_days: Vec::new(),
    });

    assert!(rendered.stable_context.contains("\"acfg\":false"));
    assert!(!rendered.stable_context.contains("\"av\":"));
}

#[test]
fn segment_encoding_reduces_prompt_size_for_steady_ride() {
    use crate::domain::workout_streams;

    let steady_watts = vec![245; 7200];
    let bucket_array =
        workout_streams::average_into_buckets(&steady_watts, workout_streams::POWER_BUCKET_SECONDS);
    let segments = workout_streams::bucket_and_encode_power_segments(&steady_watts);

    let bucket_json = serde_json::to_string(&bucket_array).expect("bucket json");
    let segment_json = serde_json::to_string(&segments).expect("segment json");

    assert!(segments.len() < bucket_array.len() / 10);
    assert!(segment_json.len() < bucket_json.len() / 10);
}

#[test]
fn recent_workout_streams_omitted_from_stable_h_w() {
    let context = TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: Some("ride-recent".to_string()),
        focus_kind: "activity".to_string(),
        intervals_status: IntervalsStatusContext::default(),
        profile: AthleteProfileContext::default(),
        races: Vec::new(),
        planned_rest_days: Vec::new(),
        future_events: Vec::new(),
        history: HistoricalTrainingContext {
            workouts: vec![
                HistoricalWorkoutContext {
                    date: "2026-04-01".to_string(),
                    activity_id: "ride-recent".to_string(),
                    power_segments: vec![[220, 220, 3]],
                    cadence_segments: vec![[88, 88, 5]],
                    ..HistoricalWorkoutContext::default()
                },
                HistoricalWorkoutContext {
                    date: "2026-03-01".to_string(),
                    activity_id: "ride-old".to_string(),
                    power_segments: vec![[180, 180, 3]],
                    cadence_segments: vec![[90, 90, 5]],
                    ..HistoricalWorkoutContext::default()
                },
            ],
            ..HistoricalTrainingContext::default()
        },
        recent_days: vec![RecentDayContext {
            date: "2026-04-01".to_string(),
            workouts: vec![RecentWorkoutContext {
                activity_id: "ride-recent".to_string(),
                start_date_local: "2026-04-01T08:00:00".to_string(),
                power_segments: vec![[220, 220, 3]],
                cadence_segments: vec![[88, 88, 5]],
                ..RecentWorkoutContext::default()
            }],
            ..RecentDayContext::default()
        }],
        recent_workout_recaps: Vec::new(),
        upcoming_days: Vec::new(),
        projected_days: Vec::new(),
    };

    let rendered = render_training_context(&context);

    assert!(rendered.stable_context.contains("\"ride-recent\""));
    assert!(rendered.stable_context.contains("\"ride-old\""));
    assert!(!rendered.stable_context.contains("\"ps\":"));
    assert!(rendered
        .volatile_context
        .contains("\"w\":{\"h\":[\"id\",\"sd\",\"ps\",\"cs\"]"));
    assert!(rendered.volatile_context.contains("[[220,220,3]]"));
}

#[test]
fn selected_workout_aligned_intervals_emit_sa_and_clear_ps_cs() {
    use crate::domain::workout_alignment::{AlignedInterval, CadenceRange, PlannedStep, StepType};

    let mut context = rich_training_context();
    context.recent_days[0].workouts[0].aligned_intervals = Some(vec![AlignedInterval {
        interval_index: 0,
        planned_step: PlannedStep {
            name: "work".into(),
            step_type: StepType::Work,
            target_power_min: 270,
            target_power_max: 285,
            planned_duration_seconds: 480,
        },
        actual_duration_seconds: 480,
        avg_power: 275,
        normalized_power: 278,
        avg_cadence: 88,
        cadence_range: CadenceRange { min: 85, max: 92 },
        anomalies: Vec::new(),
    }]);
    context.recent_days[0].workouts[0].power_segments.clear();
    context.recent_days[0].workouts[0].cadence_segments.clear();

    let rendered = render_training_context(&context);

    assert!(rendered.volatile_context.contains("\"sa\""));
    assert!(rendered.volatile_context.contains("\"interval_index\""));
    assert!(rendered.volatile_context.contains(&format!(
        "\"sa\":{{\"{}\"",
        context.recent_days[0].workouts[0].activity_id
    )));
    assert!(!rendered.volatile_context.contains("[[220,220,3]]"));
}

#[test]
fn mixed_race_disciplines_keep_per_row_disc_column() {
    let context = TrainingContext {
        races: vec![
            RaceContext {
                race_id: "race-1".to_string(),
                date: "2026-06-01".to_string(),
                name: "Road Race".to_string(),
                distance_meters: 100_000,
                discipline: "road".to_string(),
                priority: "A".to_string(),
            },
            RaceContext {
                race_id: "race-2".to_string(),
                date: "2026-07-01".to_string(),
                name: "Criterium".to_string(),
                distance_meters: 50_000,
                discipline: "crit".to_string(),
                priority: "B".to_string(),
            },
        ],
        ..TrainingContext::default()
    };

    let rendered = render_training_context(&context);

    assert!(rendered
        .stable_context
        .contains("\"h\":[\"d\",\"n\",\"km\",\"pri\",\"disc\",\"id\"]"));
    assert!(rendered.stable_context.contains("\"road\""));
    assert!(rendered.stable_context.contains("\"crit\""));
    assert!(!rendered.stable_context.contains("\"def_disc\""));
}

#[test]
fn mixed_workout_activity_types_keep_per_row_ty_column() {
    let context = TrainingContext {
        history: HistoricalTrainingContext {
            workouts: vec![
                HistoricalWorkoutContext {
                    date: "2026-03-01".to_string(),
                    activity_id: "ride-1".to_string(),
                    activity_type: Some("Ride".to_string()),
                    ..HistoricalWorkoutContext::default()
                },
                HistoricalWorkoutContext {
                    date: "2026-03-02".to_string(),
                    activity_id: "run-1".to_string(),
                    activity_type: Some("Run".to_string()),
                    ..HistoricalWorkoutContext::default()
                },
            ],
            ..HistoricalTrainingContext::default()
        },
        ..TrainingContext::default()
    };

    let rendered = render_training_context(&context);

    assert!(rendered
        .stable_context
        .contains("\"h\":[\"d\",\"id\",\"ty\"]"));
    assert!(rendered.stable_context.contains("\"Ride\""));
    assert!(rendered.stable_context.contains("\"Run\""));
    assert!(!rendered.stable_context.contains("\"def_ty\""));
}
