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
    let mut entries = merge_workout_entries(
        planned_workouts
            .iter()
            .map(|workout| project_planned_workout_entry(workout, None))
            .collect(),
        completed_workouts,
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

pub fn merge_workout_entries(
    mut planned_entries: Vec<CalendarEntryView>,
    completed_workouts: &[CompletedWorkout],
) -> Vec<CalendarEntryView> {
    let planned_positions = index_planned_entry_positions(&planned_entries);
    let completed_by_planned = group_completed_workouts_by_planned_id(completed_workouts);

    merge_single_completed_match_into_planned_entries(
        &mut planned_entries,
        &planned_positions,
        &completed_by_planned,
    );

    let mut entries = planned_entries;
    entries.extend(build_standalone_completed_entries(
        completed_workouts,
        &planned_positions,
        &completed_by_planned,
    ));
    entries
}

fn index_planned_entry_positions(
    planned_entries: &[CalendarEntryView],
) -> std::collections::HashMap<String, usize> {
    planned_entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            entry
                .planned_workout_id
                .as_ref()
                .map(|planned_workout_id| (planned_workout_id.clone(), index))
        })
        .collect()
}

fn group_completed_workouts_by_planned_id(
    completed_workouts: &[CompletedWorkout],
) -> std::collections::HashMap<String, Vec<&CompletedWorkout>> {
    completed_workouts.iter().fold(
        std::collections::HashMap::<String, Vec<&CompletedWorkout>>::new(),
        |mut grouped, workout| {
            if let Some(planned_workout_id) = &workout.planned_workout_id {
                grouped
                    .entry(planned_workout_id.clone())
                    .or_default()
                    .push(workout);
            }
            grouped
        },
    )
}

fn merge_single_completed_match_into_planned_entries(
    planned_entries: &mut [CalendarEntryView],
    planned_positions: &std::collections::HashMap<String, usize>,
    completed_by_planned: &std::collections::HashMap<String, Vec<&CompletedWorkout>>,
) {
    for (planned_workout_id, completed_matches) in completed_by_planned {
        let Some(completed_workout) = single_completed_match(completed_matches) else {
            continue;
        };
        let Some(position) = planned_positions.get(planned_workout_id) else {
            continue;
        };

        merge_completed_workout_into_planned_entry(
            &mut planned_entries[*position],
            completed_workout,
        );
    }
}

fn merge_completed_workout_into_planned_entry(
    planned_entry: &mut CalendarEntryView,
    completed_workout: &CompletedWorkout,
) {
    let completed_entry = project_completed_workout_entry(completed_workout);
    planned_entry.completed_workout_id = completed_entry.completed_workout_id;
    planned_entry.summary = completed_entry.summary;
    if planned_entry.description.is_none() {
        planned_entry.description = completed_entry.description;
    }
}

fn build_standalone_completed_entries<'a>(
    completed_workouts: &'a [CompletedWorkout],
    planned_positions: &'a std::collections::HashMap<String, usize>,
    completed_by_planned: &'a std::collections::HashMap<String, Vec<&'a CompletedWorkout>>,
) -> impl Iterator<Item = CalendarEntryView> + 'a {
    completed_workouts
        .iter()
        .filter(|workout| {
            should_keep_completed_workout_as_standalone_entry(
                workout,
                planned_positions,
                completed_by_planned,
            )
        })
        .map(project_completed_workout_entry)
}

fn should_keep_completed_workout_as_standalone_entry(
    workout: &CompletedWorkout,
    planned_positions: &std::collections::HashMap<String, usize>,
    completed_by_planned: &std::collections::HashMap<String, Vec<&CompletedWorkout>>,
) -> bool {
    let Some(planned_workout_id) = workout.planned_workout_id.as_deref() else {
        return true;
    };

    let Some(completed_matches) = completed_by_planned.get(planned_workout_id) else {
        return true;
    };

    completed_matches.len() != 1 || !planned_positions.contains_key(planned_workout_id)
}

fn single_completed_match<'a>(
    completed_matches: &'a [&CompletedWorkout],
) -> Option<&'a CompletedWorkout> {
    (completed_matches.len() == 1).then_some(completed_matches[0])
}
