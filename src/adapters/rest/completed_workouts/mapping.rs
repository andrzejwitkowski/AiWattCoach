use serde_json::Value;

use crate::{
    adapters::rest::intervals::{map_activity_to_dto, ActivityDto},
    domain::{
        completed_workouts::{CompletedWorkout, CompletedWorkoutSeries, CompletedWorkoutStream},
        intervals::{
            Activity, ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
            ActivityStream, ActivityZoneTime,
        },
    },
};

pub(super) fn map_completed_workout_to_dto(workout: CompletedWorkout) -> ActivityDto {
    map_activity_to_dto(map_completed_workout_to_activity(workout))
}

fn map_completed_workout_to_activity(workout: CompletedWorkout) -> Activity {
    let stream_types = workout
        .details
        .streams
        .iter()
        .map(|stream| stream.stream_type.clone())
        .collect::<Vec<_>>();
    let has_heart_rate = workout
        .details
        .intervals
        .iter()
        .any(|interval| interval.average_heart_rate_bpm.is_some())
        || stream_types
            .iter()
            .any(|stream_type| matches!(stream_type.as_str(), "heartrate" | "heart_rate"));

    Activity {
        id: canonical_activity_id(&workout),
        athlete_id: None,
        start_date_local: workout.start_date_local,
        start_date: None,
        name: workout.name,
        description: workout.description,
        activity_type: workout.activity_type,
        source: Some("canonical_completed_workout".to_string()),
        external_id: workout.external_id,
        device_name: None,
        distance_meters: workout.distance_meters,
        moving_time_seconds: workout.duration_seconds,
        elapsed_time_seconds: workout.duration_seconds,
        total_elevation_gain_meters: None,
        total_elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        trainer: workout.trainer,
        commute: false,
        race: false,
        has_heart_rate,
        stream_types,
        tags: Vec::new(),
        metrics: ActivityMetrics {
            training_stress_score: workout.metrics.training_stress_score,
            normalized_power_watts: workout.metrics.normalized_power_watts,
            intensity_factor: workout.metrics.intensity_factor,
            efficiency_factor: workout.metrics.efficiency_factor,
            variability_index: workout.metrics.variability_index,
            average_power_watts: workout.metrics.average_power_watts,
            ftp_watts: workout.metrics.ftp_watts,
            total_work_joules: workout.metrics.total_work_joules,
            calories: workout.metrics.calories,
            trimp: workout.metrics.trimp,
            power_load: workout.metrics.power_load,
            heart_rate_load: workout.metrics.heart_rate_load,
            pace_load: workout.metrics.pace_load,
            strain_score: workout.metrics.strain_score,
        },
        details: ActivityDetails {
            intervals: workout
                .details
                .intervals
                .into_iter()
                .map(|interval| ActivityInterval {
                    id: interval.id,
                    label: interval.label,
                    interval_type: interval.interval_type,
                    group_id: interval.group_id,
                    start_index: interval.start_index,
                    end_index: interval.end_index,
                    start_time_seconds: interval.start_time_seconds,
                    end_time_seconds: interval.end_time_seconds,
                    moving_time_seconds: interval.moving_time_seconds,
                    elapsed_time_seconds: interval.elapsed_time_seconds,
                    distance_meters: interval.distance_meters,
                    average_power_watts: interval.average_power_watts,
                    normalized_power_watts: interval.normalized_power_watts,
                    training_stress_score: interval.training_stress_score,
                    average_heart_rate_bpm: interval.average_heart_rate_bpm,
                    average_cadence_rpm: interval.average_cadence_rpm,
                    average_speed_mps: interval.average_speed_mps,
                    average_stride_meters: interval.average_stride_meters,
                    zone: interval.zone,
                })
                .collect(),
            interval_groups: workout
                .details
                .interval_groups
                .into_iter()
                .map(|group| ActivityIntervalGroup {
                    id: group.id,
                    count: group.count,
                    start_index: group.start_index,
                    moving_time_seconds: group.moving_time_seconds,
                    elapsed_time_seconds: group.elapsed_time_seconds,
                    distance_meters: group.distance_meters,
                    average_power_watts: group.average_power_watts,
                    normalized_power_watts: group.normalized_power_watts,
                    training_stress_score: group.training_stress_score,
                    average_heart_rate_bpm: group.average_heart_rate_bpm,
                    average_cadence_rpm: group.average_cadence_rpm,
                    average_speed_mps: group.average_speed_mps,
                    average_stride_meters: group.average_stride_meters,
                })
                .collect(),
            streams: workout
                .details
                .streams
                .into_iter()
                .map(map_stream_to_activity_stream)
                .collect(),
            interval_summary: workout.details.interval_summary,
            skyline_chart: workout.details.skyline_chart,
            power_zone_times: workout
                .details
                .power_zone_times
                .into_iter()
                .map(|zone| ActivityZoneTime {
                    zone_id: zone.zone_id,
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: workout.details.heart_rate_zone_times,
            pace_zone_times: workout.details.pace_zone_times,
            gap_zone_times: workout.details.gap_zone_times,
        },
        details_unavailable_reason: workout.details_unavailable_reason,
    }
}

fn canonical_activity_id(workout: &CompletedWorkout) -> String {
    workout
        .source_activity_id
        .clone()
        .unwrap_or_else(|| legacy_activity_id(&workout.completed_workout_id).to_string())
}

fn legacy_activity_id(completed_workout_id: &str) -> &str {
    completed_workout_id
        .strip_prefix("intervals-activity:")
        .unwrap_or(completed_workout_id)
}

fn map_stream_to_activity_stream(stream: CompletedWorkoutStream) -> ActivityStream {
    ActivityStream {
        stream_type: stream.stream_type,
        name: stream.name,
        data: map_series_to_json(stream.primary_series),
        data2: map_series_to_json(stream.secondary_series),
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn map_series_to_json(series: Option<CompletedWorkoutSeries>) -> Option<Value> {
    match series? {
        CompletedWorkoutSeries::Integers(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Floats(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Bools(values) => Some(serde_json::json!(values)),
        CompletedWorkoutSeries::Strings(values) => Some(serde_json::json!(values)),
    }
}
