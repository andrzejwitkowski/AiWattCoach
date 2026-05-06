use crate::domain::completed_workouts::{
    compute_power_curve, CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
    CompletedWorkoutSeries, CompletedWorkoutStream, PowerCurveError,
};

fn workout_with_watts(name: &str, watts: Vec<Option<i32>>) -> CompletedWorkout {
    let integers: Vec<i64> = watts.iter().map(|v| v.unwrap_or(-1) as i64).collect();
    CompletedWorkout::new(
        format!("cw-{name}"),
        "u1".to_string(),
        "2026-01-01T12:00:00".to_string(),
        None,
        None,
        Some(name.to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics {
            training_stress_score: None,
            normalized_power_watts: None,
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: None,
            ftp_watts: None,
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
                primary_series: Some(CompletedWorkoutSeries::Integers(integers)),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    )
}

#[test]
fn rejects_resolution_below_5() {
    let w = workout_with_watts(
        "a",
        vec![Some(200), Some(250), Some(300), Some(275), Some(225)],
    );
    assert_eq!(
        compute_power_curve(&w, 1).unwrap_err(),
        PowerCurveError::InvalidResolution
    );
    assert_eq!(
        compute_power_curve(&w, 4).unwrap_err(),
        PowerCurveError::InvalidResolution
    );
    assert_eq!(
        compute_power_curve(&w, 7).unwrap_err(),
        PowerCurveError::InvalidResolution
    );
}

#[test]
fn rejects_details_unavailable() {
    let mut w = workout_with_watts("a", vec![Some(200)]);
    w.details_unavailable_reason = Some("no fit".to_string());
    assert_eq!(
        compute_power_curve(&w, 5).unwrap_err(),
        PowerCurveError::DetailsUnavailable
    );
}

#[test]
fn rejects_missing_watts_stream() {
    let w = CompletedWorkout::new(
        "cw-nostream".to_string(),
        "u1".to_string(),
        "2026-01-01T12:00:00".to_string(),
        None,
        None,
        Some("ride".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics {
            training_stress_score: None,
            normalized_power_watts: None,
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: None,
            ftp_watts: None,
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
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    );
    assert_eq!(
        compute_power_curve(&w, 5).unwrap_err(),
        PowerCurveError::WattsStreamMissing
    );
}

#[test]
fn rejects_no_valid_power_samples() {
    let w = workout_with_watts("all_bad", vec![None, None, None]);
    assert_eq!(
        compute_power_curve(&w, 5).unwrap_err(),
        PowerCurveError::NoValidPowerSamples
    );
}

#[test]
fn computes_simple_5s_curve() {
    let w = workout_with_watts(
        "simple",
        vec![Some(120), Some(200), Some(300), Some(250), Some(180)],
    );
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.resolution_seconds, 5);
    assert_eq!(curve.source_samples, 5);
    assert_eq!(curve.valid_power_samples, 5);
    assert_eq!(curve.duration_start_seconds, 5);
    assert_eq!(curve.duration_step_seconds, 5);
    assert_eq!(curve.max_average_watts, vec![Some(250)]);
}

#[test]
fn computes_two_point_curve() {
    let w = workout_with_watts(
        "two",
        vec![Some(200), Some(300), Some(250), Some(150), Some(350)],
    );
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.source_samples, 5);
    assert_eq!(curve.max_average_watts.len(), 1);
    assert_eq!(curve.max_average_watts[0], Some(250));
}

#[test]
fn handles_negative_samples_as_invalid() {
    let w = workout_with_watts(
        "neg",
        vec![Some(100), Some(-50), Some(200), Some(300), Some(250)],
    );
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.source_samples, 5);
    assert_eq!(curve.valid_power_samples, 4);
    assert_eq!(curve.max_average_watts[0], Some(250));
}

#[test]
fn handles_non_finite_floats_as_invalid() {
    let w = CompletedWorkout::new(
        "cw-float".to_string(),
        "u1".to_string(),
        "2026-01-01T12:00:00".to_string(),
        None,
        None,
        Some("ride".to_string()),
        None,
        Some("Ride".to_string()),
        None,
        false,
        None,
        None,
        CompletedWorkoutMetrics {
            training_stress_score: None,
            normalized_power_watts: None,
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: None,
            ftp_watts: None,
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
                primary_series: Some(CompletedWorkoutSeries::Floats(vec![
                    200.0,
                    f64::NAN,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    300.0,
                    250.0,
                    280.0,
                    150.0,
                    220.0,
                    180.0,
                ])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        None,
    );
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.source_samples, 10);
    assert_eq!(curve.valid_power_samples, 7);
    let avg_5s = curve.max_average_watts[0].unwrap();
    assert!(avg_5s > 0);
}

#[test]
fn entire_window_must_be_valid() {
    let w = workout_with_watts(
        "gap",
        vec![
            Some(100),
            None,
            None,
            None,
            None,
            Some(200),
            Some(200),
            Some(200),
            Some(200),
            Some(200),
        ],
    );
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.max_average_watts[0], Some(200));
}

#[test]
fn computes_10s_resolution_curve() {
    let w = workout_with_watts(
        "ten",
        vec![
            Some(100),
            Some(200),
            Some(300),
            Some(250),
            Some(180),
            Some(150),
            Some(220),
            Some(280),
            Some(260),
            Some(190),
        ],
    );
    let curve = compute_power_curve(&w, 10).unwrap();
    assert_eq!(curve.resolution_seconds, 10);
    assert_eq!(curve.duration_start_seconds, 10);
    assert_eq!(curve.duration_step_seconds, 10);
    assert_eq!(curve.max_average_watts.len(), 1);
    assert_eq!(curve.max_average_watts[0], Some(213));
}

#[test]
fn computes_larger_series_correctly() {
    let n = 100;
    let values: Vec<Option<i32>> = (0..n).map(|i| Some(100 + i)).collect();
    let w = workout_with_watts("hundred", values);
    let curve = compute_power_curve(&w, 5).unwrap();
    assert_eq!(curve.source_samples, 100);
    assert_eq!(curve.max_average_watts.len(), 20);
    for point in &curve.max_average_watts {
        assert!(point.is_some());
    }
}
