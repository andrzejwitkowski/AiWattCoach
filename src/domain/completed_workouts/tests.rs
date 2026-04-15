use super::{
    CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutRepository,
    CompletedWorkoutSeries, CompletedWorkoutStream, CompletedWorkoutZoneTime,
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
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
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

#[tokio::test]
async fn completed_workout_repository_lists_by_user_and_date_range() {
    let repository = super::ports::NoopCompletedWorkoutRepository::default();
    repository
        .upsert(sample_workout(
            "completed-2",
            "user-1",
            "2026-05-02T08:00:00",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_workout(
            "completed-1",
            "user-1",
            "2026-05-01T08:00:00",
        ))
        .await
        .unwrap();
    repository
        .upsert(sample_workout(
            "completed-3",
            "user-2",
            "2026-05-01T08:00:00",
        ))
        .await
        .unwrap();

    let workouts = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(workouts.len(), 2);
    assert_eq!(workouts[0].completed_workout_id, "completed-1");
    assert_eq!(workouts[1].completed_workout_id, "completed-2");
}

fn sample_workout(
    completed_workout_id: &str,
    user_id: &str,
    start_date_local: &str,
) -> CompletedWorkout {
    CompletedWorkout::new(
        completed_workout_id.to_string(),
        user_id.to_string(),
        start_date_local.to_string(),
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
                primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
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
    )
}
