use crate::domain::{
    completed_workouts::CompletedWorkout, planned_workouts::PlannedWorkout, races::Race,
    special_days::SpecialDay,
};

use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, CalendarEntryView,
};

pub fn rebuild_calendar_entries(
    planned_workouts: &[PlannedWorkout],
    completed_workouts: &[CompletedWorkout],
    races: &[Race],
    special_days: &[SpecialDay],
) -> Vec<CalendarEntryView> {
    let mut entries = planned_workouts
        .iter()
        .map(|workout| project_planned_workout_entry(workout, None))
        .collect::<Vec<_>>();
    entries.extend(
        completed_workouts
            .iter()
            .map(project_completed_workout_entry),
    );
    entries.extend(races.iter().map(|race| project_race_entry(race, None)));
    entries.extend(special_days.iter().map(project_special_day_entry));
    entries.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.entry_kind.as_str().cmp(right.entry_kind.as_str()))
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    entries
}
