use crate::{
    adapters::rest::intervals::map_event_to_dto_with_parsed_workout_doc,
    domain::calendar::CalendarEvent,
};

use super::dto::{CalendarEventDto, ProjectedWorkoutDto};

pub(super) fn map_calendar_event_to_dto(event: CalendarEvent) -> CalendarEventDto {
    let CalendarEvent {
        calendar_entry_id,
        event,
        source,
        projected_workout,
        sync_status,
        linked_intervals_event_id,
    } = event;
    let workout_doc = event.workout_doc.clone();
    let event_dto = map_event_to_dto_with_parsed_workout_doc(event, workout_doc.as_deref(), None);

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
