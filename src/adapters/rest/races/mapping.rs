use crate::domain::races::Race;

use super::dto::RaceDto;

pub(super) fn map_race_to_dto(race: Race) -> RaceDto {
    RaceDto {
        race_id: race.race_id,
        date: race.date,
        name: race.name,
        distance_meters: race.distance_meters,
        discipline: race.discipline.as_str().to_string(),
        priority: race.priority.as_str().to_string(),
        sync_status: race.sync_status.as_str().to_string(),
        linked_intervals_event_id: race.linked_intervals_event_id,
        last_error: race.last_error,
    }
}
