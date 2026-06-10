use crate::domain::planned_rest_days::PlannedRestDay;

use super::dto::PlannedRestDayDto;

pub(super) fn map_planned_rest_day_to_dto(entry: PlannedRestDay) -> PlannedRestDayDto {
    PlannedRestDayDto {
        planned_rest_day_id: entry.planned_rest_day_id,
        start_date: entry.start_date,
        end_date: entry.end_date,
        title: entry.title,
        note: entry.note,
        created_at_epoch_seconds: entry.created_at_epoch_seconds,
        updated_at_epoch_seconds: entry.updated_at_epoch_seconds,
    }
}
