use crate::{
    adapters::rest::intervals::map_event_to_dto_with_parsed_workout_doc,
    domain::{
        calendar::CalendarEvent,
        calendar_labels::{
            CalendarLabel, CalendarLabelPayload, CalendarLabelsResponse, CalendarRaceLabel,
        },
    },
};

use super::dto::{
    CalendarActivityLabelDto, CalendarCustomLabelDto, CalendarEventDto, CalendarHealthLabelDto,
    CalendarLabelDto, CalendarLabelPayloadDto, CalendarLabelsResponseDto, CalendarRaceLabelDto,
    ProjectedWorkoutDto,
};

pub(super) fn map_calendar_event_to_dto(event: CalendarEvent) -> CalendarEventDto {
    let CalendarEvent {
        calendar_entry_id,
        event,
        source,
        projected_workout,
        sync_status,
        linked_intervals_event_id,
    } = event;
    let structured_workout_text = event.structured_workout_text().map(ToString::to_string);
    let event_dto =
        map_event_to_dto_with_parsed_workout_doc(event, structured_workout_text.as_deref(), None);

    CalendarEventDto {
        id: event_dto.id,
        calendar_entry_id,
        start_date_local: event_dto.start_date_local,
        name: event_dto.name,
        category: event_dto.category,
        description: event_dto.description,
        indoor: event_dto.indoor,
        color: event_dto.color,
        event_definition: event_dto.event_definition,
        actual_workout: None,
        planned_source: source.as_str().to_string(),
        sync_status: sync_status.map(|status| status.as_str().to_string()),
        linked_intervals_event_id,
        projected_workout: projected_workout.map(|projected| ProjectedWorkoutDto {
            projected_workout_id: projected.projected_workout_id,
            operation_key: projected.operation_key,
            date: projected.date,
            source_workout_id: projected.source_workout_id,
        }),
    }
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
