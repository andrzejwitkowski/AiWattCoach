use super::{CompletedWorkout, CompletedWorkoutPowerCurve, CompletedWorkoutSeries};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerCurveError {
    InvalidResolution,
    DetailsUnavailable,
    WattsStreamMissing,
    WattsStreamUnsupportedType,
    NoValidPowerSamples,
    InsufficientData,
}

impl std::fmt::Display for PowerCurveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidResolution => write!(f, "invalid resolution"),
            Self::DetailsUnavailable => write!(f, "workout details are unavailable"),
            Self::WattsStreamMissing => write!(f, "no watts power stream in workout details"),
            Self::WattsStreamUnsupportedType => {
                write!(f, "watts stream uses an unsupported series type")
            }
            Self::NoValidPowerSamples => write!(f, "no valid power samples available"),
            Self::InsufficientData => write!(f, "not enough data for the requested resolution"),
        }
    }
}

impl std::error::Error for PowerCurveError {}

pub fn compute_power_curve(
    workout: &CompletedWorkout,
    resolution_seconds: u16,
) -> Result<CompletedWorkoutPowerCurve, PowerCurveError> {
    if resolution_seconds < 5 || resolution_seconds % 5 != 0 {
        return Err(PowerCurveError::InvalidResolution);
    }

    if workout.details_unavailable_reason.is_some() {
        return Err(PowerCurveError::DetailsUnavailable);
    }

    let watts_stream = workout
        .details
        .streams
        .iter()
        .find(|s| s.stream_type == "watts")
        .ok_or(PowerCurveError::WattsStreamMissing)?;

    let power_samples = normalize_power_series(watts_stream.primary_series.as_ref())?;

    if power_samples.is_empty() || power_samples.iter().all(Option::is_none) {
        return Err(PowerCurveError::NoValidPowerSamples);
    }

    let source_samples = power_samples.len();
    if (resolution_seconds as usize) > source_samples {
        return Err(PowerCurveError::InsufficientData);
    }
    let valid_power_samples = power_samples.iter().filter(|v| v.is_some()).count();

    let (sum_prefix, valid_prefix) = build_prefixes(&power_samples);

    let max_duration = source_samples as u32;
    let step = resolution_seconds as u32;
    let num_points = (max_duration / step) as usize;

    let mut max_average_watts: Vec<Option<i32>> = Vec::with_capacity(num_points);

    for duration_multiple in 1..=num_points {
        let duration = (duration_multiple as u32 * step) as usize;
        let mut best: Option<i32> = None;

        for start in 0..=source_samples.saturating_sub(duration) {
            let end = start + duration;
            if valid_prefix[end] - valid_prefix[start] == duration {
                let sum = sum_prefix[end] - sum_prefix[start];
                let avg = (sum / duration as i64) as i32;
                best = Some(best.map_or(avg, |b| b.max(avg)));
            }
        }

        max_average_watts.push(best);
    }

    Ok(CompletedWorkoutPowerCurve {
        resolution_seconds,
        sample_period_seconds: 1,
        source_samples,
        valid_power_samples,
        duration_start_seconds: resolution_seconds as u32,
        duration_step_seconds: resolution_seconds,
        max_average_watts,
    })
}

fn normalize_power_series(
    series: Option<&CompletedWorkoutSeries>,
) -> Result<Vec<Option<i32>>, PowerCurveError> {
    match series {
        Some(CompletedWorkoutSeries::Integers(values)) => Ok(values
            .iter()
            .map(|&v| (v >= 0).then_some(v as i32))
            .collect()),
        Some(CompletedWorkoutSeries::Floats(values)) => Ok(values
            .iter()
            .map(|&v| {
                if v.is_finite() && v >= 0.0 && v <= i32::MAX as f64 {
                    Some(v.round() as i32)
                } else {
                    None
                }
            })
            .collect()),
        _ => Err(PowerCurveError::WattsStreamUnsupportedType),
    }
}

fn build_prefixes(samples: &[Option<i32>]) -> (Vec<i64>, Vec<usize>) {
    let n = samples.len();
    let mut sum_prefix = Vec::with_capacity(n + 1);
    let mut valid_prefix = Vec::with_capacity(n + 1);
    sum_prefix.push(0);
    valid_prefix.push(0);

    let mut running_sum: i64 = 0;
    let mut running_valid: usize = 0;
    for sample in samples {
        if let Some(value) = sample {
            running_sum += *value as i64;
            running_valid += 1;
        }
        sum_prefix.push(running_sum);
        valid_prefix.push(running_valid);
    }

    (sum_prefix, valid_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
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
                streams: vec![super::super::CompletedWorkoutStream {
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
                streams: vec![super::super::CompletedWorkoutStream {
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
}
