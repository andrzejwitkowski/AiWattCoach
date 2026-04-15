use super::date_keys::start_minute_bucket;

use crate::domain::{
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutStream,
    },
    intervals::{round_distance_bucket, round_duration_bucket},
};

pub(super) fn completed_workout_dedup_key(workout: &CompletedWorkout) -> Option<String> {
    let start_bucket = start_minute_bucket(&workout.start_date_local)?;
    let duration_seconds = completed_workout_duration_seconds(workout)?;
    let distance_bucket_meters = completed_workout_distance_bucket(workout);
    let stream_bucket = completed_workout_stream_bucket(workout);
    Some(format!(
        "v1:{}|{}|{}|{}",
        start_bucket,
        round_duration_bucket(duration_seconds),
        distance_bucket_meters
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        stream_bucket,
    ))
}

fn completed_workout_duration_seconds(workout: &CompletedWorkout) -> Option<i32> {
    let group_duration = workout
        .details
        .interval_groups
        .iter()
        .filter_map(|group| group.elapsed_time_seconds.or(group.moving_time_seconds))
        .max();
    let interval_duration = workout
        .details
        .intervals
        .iter()
        .filter_map(|interval| {
            interval
                .elapsed_time_seconds
                .or(interval.moving_time_seconds)
        })
        .max();
    let stream_duration = workout
        .details
        .streams
        .iter()
        .filter_map(completed_workout_stream_sample_count)
        .max();

    group_duration.or(interval_duration).or(stream_duration)
}

fn completed_workout_distance_bucket(workout: &CompletedWorkout) -> Option<i32> {
    let group_distance = workout
        .details
        .interval_groups
        .iter()
        .filter_map(|group| group.distance_meters)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let interval_distance = workout
        .details
        .intervals
        .iter()
        .filter_map(|interval| interval.distance_meters)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let stream_distance = completed_workout_stream_distance_bucket(workout);

    round_distance_bucket(group_distance.or(interval_distance).or(stream_distance))
}

fn completed_workout_stream_bucket(workout: &CompletedWorkout) -> String {
    let mut stream_types = workout
        .details
        .streams
        .iter()
        .map(|stream| stream.stream_type.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    stream_types.sort();
    stream_types.dedup();

    if stream_types.is_empty() {
        "none".to_string()
    } else {
        stream_types.join(",")
    }
}

fn completed_workout_stream_sample_count(stream: &CompletedWorkoutStream) -> Option<i32> {
    let primary = json_array_len(stream.primary_series.as_ref());
    let secondary = json_array_len(stream.secondary_series.as_ref());
    primary.or(secondary)
}

fn completed_workout_stream_distance_bucket(workout: &CompletedWorkout) -> Option<f64> {
    workout
        .details
        .streams
        .iter()
        .find(|stream| {
            stream.stream_type.eq_ignore_ascii_case("distance")
                || stream.stream_type.eq_ignore_ascii_case("dist")
        })
        .and_then(|stream| last_numeric_value(stream.primary_series.as_ref()))
        .or_else(|| {
            workout
                .details
                .streams
                .iter()
                .find(|stream| {
                    stream.stream_type.eq_ignore_ascii_case("distance")
                        || stream.stream_type.eq_ignore_ascii_case("dist")
                })
                .and_then(|stream| last_numeric_value(stream.secondary_series.as_ref()))
        })
}

fn json_array_len(value: Option<&serde_json::Value>) -> Option<i32> {
    let len = value?.as_array()?.len();
    i32::try_from(len).ok().filter(|value| *value > 0)
}

fn last_numeric_value(value: Option<&serde_json::Value>) -> Option<f64> {
    value?
        .as_array()?
        .iter()
        .rev()
        .find_map(serde_json::Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(super) fn merge_completed_workout(
    existing: CompletedWorkout,
    incoming: CompletedWorkout,
) -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: existing.completed_workout_id,
        user_id: existing.user_id,
        start_date_local: incoming.start_date_local,
        metrics: merge_completed_workout_metrics(existing.metrics, incoming.metrics),
        details: merge_completed_workout_details(existing.details, incoming.details),
    }
}

fn merge_completed_workout_metrics(
    existing: CompletedWorkoutMetrics,
    incoming: CompletedWorkoutMetrics,
) -> CompletedWorkoutMetrics {
    CompletedWorkoutMetrics {
        training_stress_score: incoming
            .training_stress_score
            .or(existing.training_stress_score),
        normalized_power_watts: incoming
            .normalized_power_watts
            .or(existing.normalized_power_watts),
        intensity_factor: incoming.intensity_factor.or(existing.intensity_factor),
        efficiency_factor: incoming.efficiency_factor.or(existing.efficiency_factor),
        variability_index: incoming.variability_index.or(existing.variability_index),
        average_power_watts: incoming
            .average_power_watts
            .or(existing.average_power_watts),
        ftp_watts: incoming.ftp_watts.or(existing.ftp_watts),
        total_work_joules: incoming.total_work_joules.or(existing.total_work_joules),
        calories: incoming.calories.or(existing.calories),
        trimp: incoming.trimp.or(existing.trimp),
        power_load: incoming.power_load.or(existing.power_load),
        heart_rate_load: incoming.heart_rate_load.or(existing.heart_rate_load),
        pace_load: incoming.pace_load.or(existing.pace_load),
        strain_score: incoming.strain_score.or(existing.strain_score),
    }
}

fn merge_completed_workout_details(
    existing: CompletedWorkoutDetails,
    incoming: CompletedWorkoutDetails,
) -> CompletedWorkoutDetails {
    CompletedWorkoutDetails {
        intervals: prefer_non_empty(incoming.intervals, existing.intervals),
        interval_groups: prefer_non_empty(incoming.interval_groups, existing.interval_groups),
        streams: prefer_non_empty(incoming.streams, existing.streams),
        interval_summary: prefer_non_empty(incoming.interval_summary, existing.interval_summary),
        skyline_chart: prefer_non_empty(incoming.skyline_chart, existing.skyline_chart),
        power_zone_times: prefer_non_empty(incoming.power_zone_times, existing.power_zone_times),
        heart_rate_zone_times: prefer_non_empty(
            incoming.heart_rate_zone_times,
            existing.heart_rate_zone_times,
        ),
        pace_zone_times: prefer_non_empty(incoming.pace_zone_times, existing.pace_zone_times),
        gap_zone_times: prefer_non_empty(incoming.gap_zone_times, existing.gap_zone_times),
    }
}

fn prefer_non_empty<T>(incoming: Vec<T>, existing: Vec<T>) -> Vec<T> {
    if incoming.is_empty() {
        existing
    } else {
        incoming
    }
}
