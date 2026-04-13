use crate::domain::{
    completed_workouts::CompletedWorkout,
    external_sync::ExternalSyncState,
    planned_workouts::PlannedWorkout,
    races::Race,
    special_days::{SpecialDay, SpecialDayKind},
};

use super::{CalendarEntryKind, CalendarEntrySummary, CalendarEntrySync, CalendarEntryView};

pub fn project_planned_workout_entry(
    workout: &PlannedWorkout,
    sync_state: Option<&ExternalSyncState>,
) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("planned:{}", workout.planned_workout_id),
        user_id: workout.user_id.clone(),
        entry_kind: CalendarEntryKind::PlannedWorkout,
        date: workout.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", workout.date)),
        title: planned_workout_title(workout),
        subtitle: Some(format!("{} lines", workout.workout.lines.len())),
        description: None,
        planned_workout_id: Some(workout.planned_workout_id.clone()),
        completed_workout_id: None,
        race_id: None,
        special_day_id: None,
        summary: None,
        sync: map_sync_state(sync_state),
    }
}

pub fn project_completed_workout_entry(workout: &CompletedWorkout) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("completed:{}", workout.completed_workout_id),
        user_id: workout.user_id.clone(),
        entry_kind: CalendarEntryKind::CompletedWorkout,
        date: date_prefix(&workout.start_date_local).to_string(),
        start_date_local: Some(workout.start_date_local.clone()),
        title: "Completed workout".to_string(),
        subtitle: workout
            .metrics
            .training_stress_score
            .map(|tss| format!("TSS {tss}")),
        description: workout.details.interval_summary.first().cloned(),
        planned_workout_id: None,
        completed_workout_id: Some(workout.completed_workout_id.clone()),
        race_id: None,
        special_day_id: None,
        summary: Some(CalendarEntrySummary {
            training_stress_score: workout.metrics.training_stress_score,
            intensity_factor: workout.metrics.intensity_factor,
            normalized_power_watts: workout.metrics.normalized_power_watts,
        }),
        sync: None,
    }
}

pub fn project_race_entry(
    race: &Race,
    sync_state: Option<&ExternalSyncState>,
) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("race:{}", race.race_id),
        user_id: race.user_id.clone(),
        entry_kind: CalendarEntryKind::Race,
        date: race.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", race.date)),
        title: race.label_title(),
        subtitle: Some(race.label_subtitle()),
        description: Some(format!(
            "distance_meters={}\ndiscipline={}\npriority={}",
            race.distance_meters,
            race.discipline.as_str(),
            race.priority.as_str()
        )),
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: Some(race.race_id.clone()),
        special_day_id: None,
        summary: None,
        sync: map_sync_state(sync_state),
    }
}

pub fn project_special_day_entry(special_day: &SpecialDay) -> CalendarEntryView {
    CalendarEntryView {
        entry_id: format!("special:{}", special_day.special_day_id),
        user_id: special_day.user_id.clone(),
        entry_kind: CalendarEntryKind::SpecialDay,
        date: special_day.date.clone(),
        start_date_local: Some(format!("{}T00:00:00", special_day.date)),
        title: special_day_title(&special_day.kind),
        subtitle: None,
        description: None,
        planned_workout_id: None,
        completed_workout_id: None,
        race_id: None,
        special_day_id: Some(special_day.special_day_id.clone()),
        summary: None,
        sync: None,
    }
}

fn planned_workout_title(workout: &PlannedWorkout) -> String {
    workout
        .workout
        .lines
        .iter()
        .find_map(|line| match line {
            crate::domain::planned_workouts::PlannedWorkoutLine::Text(text) => {
                Some(text.text.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| "Planned workout".to_string())
}

fn special_day_title(kind: &SpecialDayKind) -> String {
    match kind {
        SpecialDayKind::Illness => "Illness".to_string(),
        SpecialDayKind::Travel => "Travel".to_string(),
        SpecialDayKind::Blocked => "Blocked day".to_string(),
        SpecialDayKind::Note => "Note".to_string(),
        SpecialDayKind::Other => "Special day".to_string(),
    }
}

fn map_sync_state(sync_state: Option<&ExternalSyncState>) -> Option<CalendarEntrySync> {
    sync_state.map(|state| CalendarEntrySync {
        linked_intervals_event_id: state
            .external_id
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok()),
        sync_status: Some(state.sync_status.as_str().to_string()),
    })
}

fn date_prefix(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}
