use super::{
    CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutRepository,
    CompletedWorkoutStream, CompletedWorkoutZoneTime,
};

#[test]
fn completed_workout_uses_local_canonical_id() {
    let workout = CompletedWorkout::new(
        "completed-1".to_string(),
        "user-1".to_string(),
        "2026-05-01T08:00:00".to_string(),
        CompletedWorkoutMetrics {
            training_stress_score: Some(78),
            normalized_power_watts: Some(245),
            intensity_factor: Some(0.83),
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: Some(221),
            ftp_watts: Some(295),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(serde_json::json!([180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z3".to_string(),
                seconds: 1200,
            }],
            heart_rate_zone_times: vec![600],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    );

    assert_eq!(workout.completed_workout_id, "completed-1");
    assert_eq!(workout.user_id, "user-1");
    assert_eq!(workout.start_date_local, "2026-05-01T08:00:00");
    assert_eq!(workout.metrics.training_stress_score, Some(78));
    assert_eq!(workout.details.streams.len(), 1);
}

fn assert_completed_workout_repository<T: CompletedWorkoutRepository>() {}

#[test]
fn completed_workout_repository_trait_is_usable() {
    assert_completed_workout_repository::<super::ports::NoopCompletedWorkoutRepository>();
}
