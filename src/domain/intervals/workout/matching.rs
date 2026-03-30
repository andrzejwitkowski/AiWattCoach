use super::{
    super::{Activity, ActivityInterval},
    ActualWorkoutMatch, MatchedWorkoutInterval, ParsedWorkoutDoc, WorkoutSegment,
};

pub fn find_best_activity_match(
    parsed: &ParsedWorkoutDoc,
    activities: &[Activity],
    ftp_watts: Option<i32>,
) -> Option<ActualWorkoutMatch> {
    let planned_segments = matching_segments(parsed);
    if planned_segments.is_empty() {
        return None;
    }

    activities
        .iter()
        .filter_map(|activity| evaluate_activity_match(activity, &planned_segments, ftp_watts))
        .max_by(|left, right| left.compliance_score.total_cmp(&right.compliance_score))
        .filter(|matched| matched.compliance_score >= 0.45)
}

fn matching_segments(parsed: &ParsedWorkoutDoc) -> Vec<WorkoutSegment> {
    let work_segments = parsed
        .segments
        .iter()
        .filter(|segment| segment.target_percent_ftp.unwrap_or_default() >= 75.0)
        .cloned()
        .collect::<Vec<_>>();

    if work_segments.is_empty() {
        parsed
            .segments
            .iter()
            .filter(|segment| segment.target_percent_ftp.is_some())
            .cloned()
            .collect()
    } else {
        work_segments
    }
}

fn evaluate_activity_match(
    activity: &Activity,
    planned_segments: &[WorkoutSegment],
    ftp_watts: Option<i32>,
) -> Option<ActualWorkoutMatch> {
    let power_values = extract_integer_stream(activity, &["watts"]);
    let cadence_values = extract_integer_stream(activity, &["cadence"]);
    let heart_rate_values = extract_integer_stream(activity, &["heartrate", "heart_rate"]);
    let speed_values = extract_float_stream(activity, &["velocity_smooth", "speed"]);

    let detected_intervals = detected_interval_candidates(activity);
    let fallback_intervals = if detected_intervals.is_empty() {
        detect_intervals_from_power_stream(planned_segments, &power_values, ftp_watts)
    } else {
        Vec::new()
    };
    let actual_intervals = if detected_intervals.is_empty() {
        fallback_intervals
    } else {
        detected_intervals
    };

    let pair_count = planned_segments.len().min(actual_intervals.len());
    if pair_count == 0 {
        return None;
    }

    let matched_intervals = planned_segments
        .iter()
        .zip(actual_intervals.iter())
        .map(|(planned, actual)| {
            let expected_watts = ftp_watts.and_then(|ftp| {
                planned
                    .target_percent_ftp
                    .map(|percent| (ftp as f64 * percent / 100.0).round() as i32)
            });
            let duration_score = similarity_score(
                actual.duration_seconds as f64,
                planned.duration_seconds as f64,
            );
            let power_score = match (expected_watts, actual.average_power_watts) {
                (Some(expected), Some(actual_power)) => {
                    similarity_score(actual_power as f64, expected as f64)
                }
                _ => 0.5,
            };
            let zone_score = match (planned.zone_id, actual.zone_id) {
                (Some(planned_zone), Some(actual_zone)) => {
                    (1.0 - ((planned_zone - actual_zone).abs() as f64 / 6.0)).clamp(0.0, 1.0)
                }
                _ => 0.5,
            };
            let compliance_score = super::round_to(
                (power_score * 0.55) + (duration_score * 0.35) + (zone_score * 0.10),
                3,
            );

            MatchedWorkoutInterval {
                planned_segment_order: planned.order,
                planned_label: planned.label.clone(),
                planned_duration_seconds: planned.duration_seconds,
                target_percent_ftp: planned.target_percent_ftp,
                zone_id: planned.zone_id,
                actual_interval_id: actual.id,
                actual_start_time_seconds: Some(actual.start_time_seconds),
                actual_end_time_seconds: Some(actual.end_time_seconds),
                average_power_watts: actual.average_power_watts,
                normalized_power_watts: actual.normalized_power_watts,
                average_heart_rate_bpm: actual.average_heart_rate_bpm,
                average_cadence_rpm: actual.average_cadence_rpm,
                average_speed_mps: actual.average_speed_mps,
                compliance_score,
            }
        })
        .collect::<Vec<_>>();

    let pair_score = matched_intervals
        .iter()
        .map(|interval| interval.compliance_score)
        .sum::<f64>()
        / matched_intervals.len() as f64;
    let count_score = pair_count as f64 / planned_segments.len() as f64;
    let compliance_score = super::round_to((pair_score * 0.8) + (count_score * 0.2), 3);

    Some(ActualWorkoutMatch {
        activity_id: activity.id.clone(),
        activity_name: activity.name.clone(),
        start_date_local: activity.start_date_local.clone(),
        power_values,
        cadence_values,
        heart_rate_values,
        speed_values,
        average_power_watts: activity.metrics.average_power_watts,
        normalized_power_watts: activity.metrics.normalized_power_watts,
        training_stress_score: activity.metrics.training_stress_score,
        intensity_factor: activity
            .metrics
            .intensity_factor
            .map(|value| super::round_to(value, 3)),
        compliance_score,
        matched_intervals,
    })
}

fn detected_interval_candidates(activity: &Activity) -> Vec<IntervalCandidate> {
    let preferred = activity
        .details
        .intervals
        .iter()
        .filter_map(IntervalCandidate::from_interval)
        .filter(|interval| {
            interval
                .interval_type
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("WORK"))
                .unwrap_or(false)
                || interval.zone_id.unwrap_or_default() >= 3
        })
        .collect::<Vec<_>>();

    if preferred.is_empty() {
        activity
            .details
            .intervals
            .iter()
            .filter_map(IntervalCandidate::from_interval)
            .collect()
    } else {
        preferred
    }
}

fn detect_intervals_from_power_stream(
    planned_segments: &[WorkoutSegment],
    power_values: &[i32],
    ftp_watts: Option<i32>,
) -> Vec<IntervalCandidate> {
    let Some(ftp_watts) = ftp_watts.filter(|value| *value > 0) else {
        return Vec::new();
    };
    if power_values.is_empty() {
        return Vec::new();
    }

    let prefix_sums = power_values.iter().fold(vec![0_i64], |mut sums, value| {
        let next = sums.last().copied().unwrap_or_default() + i64::from(*value);
        sums.push(next);
        sums
    });
    let mut search_start = 0usize;
    let mut detected = Vec::new();

    for planned in planned_segments {
        let Some(target_percent) = planned.target_percent_ftp else {
            continue;
        };
        let window = planned.duration_seconds.max(30) as usize;
        if search_start + window > power_values.len() {
            break;
        }

        let expected_watts = (ftp_watts as f64 * target_percent / 100.0).round();
        let mut best: Option<(usize, f64)> = None;
        for start in search_start..=(power_values.len() - window) {
            let end = start + window;
            let average = (prefix_sums[end] - prefix_sums[start]) as f64 / window as f64;
            let score = similarity_score(average, expected_watts);
            if best
                .as_ref()
                .is_none_or(|(_, best_score)| score > *best_score)
            {
                best = Some((start, score));
            }
        }

        let Some((start_index, _)) = best else {
            break;
        };
        let end_index = start_index + window;
        let average_power_watts = ((prefix_sums[end_index] - prefix_sums[start_index]) as f64
            / window as f64)
            .round() as i32;
        detected.push(IntervalCandidate {
            id: None,
            interval_type: Some("POWER_STREAM".to_string()),
            zone_id: planned.zone_id,
            start_time_seconds: start_index as i32,
            end_time_seconds: end_index as i32,
            duration_seconds: window as i32,
            average_power_watts: Some(average_power_watts),
            normalized_power_watts: Some(average_power_watts),
            average_heart_rate_bpm: None,
            average_cadence_rpm: None,
            average_speed_mps: None,
        });
        search_start = end_index;
    }

    detected
}

fn extract_integer_stream(activity: &Activity, stream_types: &[&str]) -> Vec<i32> {
    extract_float_stream(activity, stream_types)
        .into_iter()
        .map(|value| value.round() as i32)
        .collect()
}

fn extract_float_stream(activity: &Activity, stream_types: &[&str]) -> Vec<f64> {
    activity
        .details
        .streams
        .iter()
        .find(|stream| {
            stream_types
                .iter()
                .any(|stream_type| stream.stream_type.eq_ignore_ascii_case(stream_type))
        })
        .and_then(|stream| stream.data.as_ref())
        .and_then(|value| value.as_array().cloned())
        .map(|values| {
            values
                .into_iter()
                .filter_map(|value| value.as_f64())
                .collect()
        })
        .unwrap_or_default()
}

fn similarity_score(actual: f64, expected: f64) -> f64 {
    if expected <= 0.0 {
        return 0.0;
    }

    (1.0 - ((actual - expected).abs() / expected)).clamp(0.0, 1.0)
}

#[derive(Clone, Debug)]
struct IntervalCandidate {
    id: Option<i32>,
    interval_type: Option<String>,
    zone_id: Option<i32>,
    start_time_seconds: i32,
    end_time_seconds: i32,
    duration_seconds: i32,
    average_power_watts: Option<i32>,
    normalized_power_watts: Option<i32>,
    average_heart_rate_bpm: Option<i32>,
    average_cadence_rpm: Option<f64>,
    average_speed_mps: Option<f64>,
}

impl IntervalCandidate {
    fn from_interval(interval: &ActivityInterval) -> Option<Self> {
        let start_time_seconds = interval.start_time_seconds?;
        let end_time_seconds = interval.end_time_seconds?;
        let duration_seconds = (end_time_seconds - start_time_seconds).max(0);
        if duration_seconds <= 0 {
            return None;
        }

        Some(Self {
            id: interval.id,
            interval_type: interval.interval_type.clone(),
            zone_id: interval.zone,
            start_time_seconds,
            end_time_seconds,
            duration_seconds,
            average_power_watts: interval.average_power_watts,
            normalized_power_watts: interval.normalized_power_watts,
            average_heart_rate_bpm: interval.average_heart_rate_bpm,
            average_cadence_rpm: interval.average_cadence_rpm,
            average_speed_mps: interval.average_speed_mps,
        })
    }
}
