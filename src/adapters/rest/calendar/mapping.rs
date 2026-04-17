use crate::{
    adapters::rest::intervals::{
        ActualWorkoutDto, EventDefinitionDto, IntervalDefinitionDto, MatchedWorkoutIntervalDto,
        WorkoutSegmentDto, WorkoutSummaryDto,
    },
    domain::{
        calendar::{CalendarEvent, CalendarEventCategory},
        calendar_labels::{
            CalendarLabel, CalendarLabelPayload, CalendarLabelsResponse, CalendarRaceLabel,
        },
        intervals::parse_workout_doc,
    },
};

use super::dto::{
    CalendarActivityLabelDto, CalendarCustomLabelDto, CalendarEventDto, CalendarHealthLabelDto,
    CalendarLabelDto, CalendarLabelPayloadDto, CalendarLabelsResponseDto, CalendarRaceLabelDto,
    ProjectedWorkoutDto,
};

pub(super) fn map_calendar_event_to_dto(event: CalendarEvent) -> CalendarEventDto {
    let CalendarEvent {
        id,
        calendar_entry_id,
        start_date_local,
        name,
        category,
        description,
        rest_day,
        rest_day_reason,
        indoor,
        color,
        raw_workout_doc,
        source,
        projected_workout,
        sync_status,
        linked_intervals_event_id,
        actual_workout,
    } = event;
    let parsed = parse_workout_doc(
        if rest_day {
            None
        } else {
            raw_workout_doc
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .or(description.as_deref())
        },
        None,
    );

    CalendarEventDto {
        id,
        calendar_entry_id,
        start_date_local,
        name,
        category: map_category(category),
        description,
        rest_day,
        rest_day_reason,
        indoor,
        color,
        event_definition: EventDefinitionDto {
            raw_workout_doc,
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
        actual_workout: actual_workout.map(map_actual_workout_to_dto),
        planned_source: source.as_str().to_string(),
        sync_status: sync_status.map(|status| status.as_str().to_string()),
        linked_intervals_event_id,
        projected_workout: projected_workout.map(|projected| ProjectedWorkoutDto {
            projected_workout_id: projected.projected_workout_id,
            operation_key: projected.operation_key,
            date: projected.date,
            source_workout_id: projected.source_workout_id,
            rest_day: projected.rest_day,
            rest_day_reason: projected.rest_day_reason,
        }),
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
            .map(|interval| MatchedWorkoutIntervalDto {
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
            })
            .collect(),
    }
}

fn map_category(category: CalendarEventCategory) -> String {
    category.as_str().to_string()
}

pub(super) fn map_calendar_labels_to_dto(
    response: CalendarLabelsResponse,
) -> CalendarLabelsResponseDto {
    CalendarLabelsResponseDto {
        labels_by_date: response
            .labels_by_date
            .into_iter()
            .map(|(date, labels)| {
                (
                    date,
                    labels
                        .into_iter()
                        .map(|(key, label)| (key, map_calendar_label_to_dto(label)))
                        .collect(),
                )
            })
            .collect(),
    }
}

fn map_calendar_label_to_dto(label: CalendarLabel) -> CalendarLabelDto {
    CalendarLabelDto {
        kind: label.kind().to_string(),
        title: label.title,
        subtitle: label.subtitle,
        payload: map_calendar_label_payload_to_dto(label.payload),
    }
}

fn map_calendar_label_payload_to_dto(payload: CalendarLabelPayload) -> CalendarLabelPayloadDto {
    match payload {
        CalendarLabelPayload::Race(race) => {
            CalendarLabelPayloadDto::Race(map_race_label_to_dto(race))
        }
        CalendarLabelPayload::Activity(activity) => {
            CalendarLabelPayloadDto::Activity(CalendarActivityLabelDto {
                label_id: activity.label_id,
                activity_kind: activity.activity_kind,
                note: activity.note,
            })
        }
        CalendarLabelPayload::Health(health) => {
            CalendarLabelPayloadDto::Health(CalendarHealthLabelDto {
                label_id: health.label_id,
                status: health.status,
                note: health.note,
            })
        }
        CalendarLabelPayload::Custom(custom) => {
            CalendarLabelPayloadDto::Custom(CalendarCustomLabelDto {
                label_id: custom.label_id,
                value: custom.value,
            })
        }
    }
}

fn map_race_label_to_dto(race: CalendarRaceLabel) -> CalendarRaceLabelDto {
    CalendarRaceLabelDto {
        race_id: race.race_id,
        date: race.date,
        name: race.name,
        distance_meters: race.distance_meters,
        discipline: race.discipline,
        priority: race.priority,
        sync_status: race.sync_status,
        linked_intervals_event_id: race.linked_intervals_event_id,
    }
}
