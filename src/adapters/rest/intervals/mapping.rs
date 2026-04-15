use crate::domain::intervals::{
    parse_workout_doc, Activity, ActivityStream, EnrichedEvent, Event, EventCategory,
    MatchedWorkoutInterval, ParsedWorkoutDoc,
};

use super::dto::{
    ActivityDetailsDto, ActivityDto, ActivityIntervalDto, ActivityIntervalGroupDto,
    ActivityMetricsDto, ActivityStreamDto, ActivityZoneTimeDto, ActualWorkoutDto,
    EventDefinitionDto, EventDto, IntervalDefinitionDto, MatchedWorkoutIntervalDto,
    WorkoutSegmentDto, WorkoutSummaryDto,
};

pub(super) fn map_event_to_dto(event: Event) -> EventDto {
    map_event_to_dto_with_parsed(event, None, None)
}

pub(super) fn map_enriched_event_to_dto(enriched_event: EnrichedEvent) -> EventDto {
    map_event_to_dto_with_parsed(
        enriched_event.event,
        Some(enriched_event.parsed_workout),
        enriched_event.actual_workout.map(map_actual_workout_to_dto),
    )
}

fn map_event_to_dto_with_parsed(
    event: Event,
    parsed: Option<ParsedWorkoutDoc>,
    actual_workout: Option<ActualWorkoutDto>,
) -> EventDto {
    let parsed = parsed.unwrap_or_else(|| parse_workout_doc(event.structured_workout_text(), None));

    EventDto {
        id: event.id,
        start_date_local: event.start_date_local,
        name: event.name,
        category: category_to_string(&event.category),
        description: event.description,
        indoor: event.indoor,
        color: event.color,
        event_definition: EventDefinitionDto {
            raw_workout_doc: event.workout_doc.clone(),
            intervals: parsed
                .intervals
                .iter()
                .map(|interval| IntervalDefinitionDto {
                    definition: interval.definition.clone(),
                    repeat_count: interval.repeat_count,
                    duration_seconds: interval.duration_seconds,
                    target_percent_ftp: interval.target_percent_ftp,
                    zone_id: interval.zone_id,
                })
                .collect(),
            segments: parsed
                .segments
                .iter()
                .map(|segment| WorkoutSegmentDto {
                    order: segment.order,
                    label: segment.label.clone(),
                    duration_seconds: segment.duration_seconds,
                    start_offset_seconds: segment.start_offset_seconds,
                    end_offset_seconds: segment.end_offset_seconds,
                    target_percent_ftp: segment.target_percent_ftp,
                    zone_id: segment.zone_id,
                })
                .collect(),
            summary: WorkoutSummaryDto {
                total_segments: parsed.summary.total_segments,
                total_duration_seconds: parsed.summary.total_duration_seconds,
                estimated_normalized_power_watts: parsed.summary.estimated_normalized_power_watts,
                estimated_average_power_watts: parsed.summary.estimated_average_power_watts,
                estimated_intensity_factor: parsed.summary.estimated_intensity_factor,
                estimated_training_stress_score: parsed.summary.estimated_training_stress_score,
            },
        },
        actual_workout,
    }
}

fn map_actual_workout_to_dto(
    actual_workout: crate::domain::intervals::ActualWorkoutMatch,
) -> ActualWorkoutDto {
    ActualWorkoutDto {
        activity_id: actual_workout.activity_id,
        activity_name: actual_workout.activity_name,
        start_date_local: actual_workout.start_date_local,
        power_values: actual_workout.power_values,
        cadence_values: actual_workout.cadence_values,
        heart_rate_values: actual_workout.heart_rate_values,
        speed_values: actual_workout.speed_values,
        average_power_watts: actual_workout.average_power_watts,
        normalized_power_watts: actual_workout.normalized_power_watts,
        training_stress_score: actual_workout.training_stress_score,
        intensity_factor: actual_workout.intensity_factor,
        compliance_score: actual_workout.compliance_score,
        matched_intervals: actual_workout
            .matched_intervals
            .into_iter()
            .map(map_matched_interval_to_dto)
            .collect(),
    }
}

fn map_matched_interval_to_dto(interval: MatchedWorkoutInterval) -> MatchedWorkoutIntervalDto {
    MatchedWorkoutIntervalDto {
        planned_segment_order: interval.planned_segment_order,
        planned_label: interval.planned_label,
        planned_duration_seconds: interval.planned_duration_seconds,
        target_percent_ftp: interval.target_percent_ftp,
        zone_id: interval.zone_id,
        actual_interval_id: interval.actual_interval_id,
        actual_start_time_seconds: interval.actual_start_time_seconds,
        actual_end_time_seconds: interval.actual_end_time_seconds,
        average_power_watts: interval.average_power_watts,
        normalized_power_watts: interval.normalized_power_watts,
        average_heart_rate_bpm: interval.average_heart_rate_bpm,
        average_cadence_rpm: interval.average_cadence_rpm,
        average_speed_mps: interval.average_speed_mps,
        compliance_score: interval.compliance_score,
    }
}

pub(super) fn map_activity_to_dto(activity: Activity) -> ActivityDto {
    ActivityDto {
        id: activity.id,
        start_date_local: activity.start_date_local,
        start_date: activity.start_date,
        name: activity.name,
        description: activity.description,
        activity_type: activity.activity_type,
        source: activity.source,
        external_id: activity.external_id,
        device_name: activity.device_name,
        distance_meters: activity.distance_meters,
        moving_time_seconds: activity.moving_time_seconds,
        elapsed_time_seconds: activity.elapsed_time_seconds,
        total_elevation_gain_meters: activity.total_elevation_gain_meters,
        average_speed_mps: activity.average_speed_mps,
        average_heart_rate_bpm: activity.average_heart_rate_bpm,
        average_cadence_rpm: activity.average_cadence_rpm,
        trainer: activity.trainer,
        commute: activity.commute,
        race: activity.race,
        has_heart_rate: activity.has_heart_rate,
        stream_types: activity.stream_types,
        tags: activity.tags,
        metrics: ActivityMetricsDto {
            training_stress_score: activity.metrics.training_stress_score,
            normalized_power_watts: activity.metrics.normalized_power_watts,
            intensity_factor: activity.metrics.intensity_factor,
            efficiency_factor: activity.metrics.efficiency_factor,
            variability_index: activity.metrics.variability_index,
            average_power_watts: activity.metrics.average_power_watts,
            ftp_watts: activity.metrics.ftp_watts,
            total_work_joules: activity.metrics.total_work_joules,
            calories: activity.metrics.calories,
            trimp: activity.metrics.trimp,
            power_load: activity.metrics.power_load,
            heart_rate_load: activity.metrics.heart_rate_load,
            pace_load: activity.metrics.pace_load,
            strain_score: activity.metrics.strain_score,
        },
        details: ActivityDetailsDto {
            intervals: activity
                .details
                .intervals
                .into_iter()
                .map(|interval| ActivityIntervalDto {
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
            interval_groups: activity
                .details
                .interval_groups
                .into_iter()
                .map(|group| ActivityIntervalGroupDto {
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
            streams: activity
                .details
                .streams
                .into_iter()
                .map(map_activity_stream_to_dto)
                .collect(),
            interval_summary: activity.details.interval_summary,
            skyline_chart: activity.details.skyline_chart,
            power_zone_times: activity
                .details
                .power_zone_times
                .into_iter()
                .map(|zone| ActivityZoneTimeDto {
                    zone_id: zone.zone_id,
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: activity.details.heart_rate_zone_times,
            pace_zone_times: activity.details.pace_zone_times,
            gap_zone_times: activity.details.gap_zone_times,
        },
        details_unavailable_reason: activity.details_unavailable_reason,
    }
}

fn map_activity_stream_to_dto(stream: ActivityStream) -> ActivityStreamDto {
    ActivityStreamDto {
        stream_type: stream.stream_type,
        name: stream.name,
        data: stream.data,
        data2: stream.data2,
        value_type_is_array: stream.value_type_is_array,
        custom: stream.custom,
        all_null: stream.all_null,
    }
}

fn category_to_string(category: &EventCategory) -> String {
    match category {
        EventCategory::RaceA | EventCategory::RaceB | EventCategory::RaceC => {
            EventCategory::Race.as_str().to_string()
        }
        _ => category.as_str().to_string(),
    }
}
