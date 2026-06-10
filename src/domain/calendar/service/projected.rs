use sha2::{Digest, Sha256};

use crate::domain::{
    calendar::{
        CalendarEvent, CalendarEventCategory, CalendarEventSource, CalendarProjectedWorkout,
        PlannedWorkoutSyncStatus,
    },
    external_sync::{ExternalProvider, ExternalSyncState, ExternalSyncStatus},
    training_plan::TrainingPlanProjectedDay,
};

pub(super) fn build_projected_calendar_event(
    day: TrainingPlanProjectedDay,
    sync_states: &[ExternalSyncState],
    just_synced_state: Option<ExternalSyncState>,
) -> CalendarEvent {
    let payload_hash = projected_day_payload_hash(&day);
    let linked_intervals_event_id =
        linked_intervals_event_id(sync_states, just_synced_state.as_ref());
    let status = projected_day_sync_status(sync_states, just_synced_state.as_ref(), &payload_hash);
    let event_id = linked_intervals_event_id
        .unwrap_or_else(|| synthetic_event_id(&day.operation_key, &day.date));
    let projected_workout_id = projected_workout_id(&day.operation_key, &day.date);

    CalendarEvent {
        id: event_id,
        calendar_entry_id: format!("predicted:{projected_workout_id}"),
        start_date_local: day.date.clone(),
        name: projected_workout_name(&day),
        category: CalendarEventCategory::Workout,
        description: day.rest_day_reason.clone(),
        rest_day: day.rest_day,
        rest_day_reason: day.rest_day_reason.clone(),
        indoor: false,
        color: None,
        raw_workout_doc: if day.rest_day {
            None
        } else {
            day.workout.as_ref().map(serialize_projected_workout)
        },
        source: CalendarEventSource::Predicted,
        projected_workout: Some(CalendarProjectedWorkout {
            projected_workout_id,
            operation_key: day.operation_key.clone(),
            date: day.date.clone(),
            source_workout_id: day.workout_id,
            rest_day: day.rest_day,
            rest_day_reason: day.rest_day_reason,
        }),
        sync_status: Some(status),
        linked_intervals_event_id,
        actual_workout: None,
    }
}

pub(super) fn projected_workout_name(day: &TrainingPlanProjectedDay) -> Option<String> {
    if day.rest_day {
        return Some("Rest Day".to_string());
    }

    day.workout.as_ref().and_then(|workout| {
        workout
            .lines
            .iter()
            .find_map(|line| line.text().map(ToString::to_string))
    })
}

pub(super) fn serialize_projected_workout(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> String {
    crate::domain::intervals::serialize_planned_workout(workout)
}

pub(super) fn projected_day_sync_status(
    sync_states: &[ExternalSyncState],
    just_synced_state: Option<&ExternalSyncState>,
    payload_hash: &str,
) -> PlannedWorkoutSyncStatus {
    let relevant_states = sync_states_for_status(sync_states, just_synced_state);
    if relevant_states.is_empty() {
        return PlannedWorkoutSyncStatus::Unsynced;
    }

    if relevant_states.iter().any(|state| {
        state
            .last_synced_payload_hash
            .as_deref()
            .is_some_and(|hash| hash != payload_hash)
    }) {
        return PlannedWorkoutSyncStatus::Modified;
    }

    if relevant_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Synced)
    {
        return PlannedWorkoutSyncStatus::Synced;
    }

    if relevant_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Pending)
    {
        return PlannedWorkoutSyncStatus::Pending;
    }

    if relevant_states
        .iter()
        .any(|state| state.sync_status == ExternalSyncStatus::Failed)
    {
        return PlannedWorkoutSyncStatus::Failed;
    }

    PlannedWorkoutSyncStatus::Unsynced
}

pub(super) fn projected_day_payload_hash(day: &TrainingPlanProjectedDay) -> String {
    if day.rest_day {
        return crate::domain::planned_workouts::planned_workout_payload_hash_parts(
            &day.date,
            Some("Rest Day"),
            None,
        );
    }

    let Some(workout) = day.workout.as_ref() else {
        return crate::domain::planned_workouts::planned_workout_payload_hash_parts(
            &day.date, None, None,
        );
    };

    crate::domain::planned_workouts::intervals_planned_workout_payload_hash(
        &day.date,
        workout,
        projected_workout_name(day).as_deref(),
    )
}

pub(super) fn projected_event_payload_hash(
    date: &str,
    name: Option<&str>,
    workout_doc: Option<&str>,
) -> String {
    let digest = Sha256::digest(format!(
        "{date}\n{}\n{}",
        name.unwrap_or_default(),
        workout_doc.unwrap_or_default()
    ));
    format!("{digest:x}")
}

pub fn projected_workout_id(operation_key: &str, date: &str) -> String {
    format!("{operation_key}:{date}")
}

pub fn synthetic_event_id(operation_key: &str, date: &str) -> i64 {
    const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    let digest = Sha256::digest(format!("{operation_key}:{date}"));
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    ((value % MAX_JS_SAFE_INTEGER) + 1) as i64
}

pub(super) fn projected_event_sync_body(day: &TrainingPlanProjectedDay) -> Option<String> {
    projected_workout_sync_body(day)
}

pub(super) fn projected_workout_sync_body(day: &TrainingPlanProjectedDay) -> Option<String> {
    let workout = day.workout.as_ref()?;
    let serialized = crate::domain::intervals::serialize_planned_workout_for_intervals(workout);
    Ok::<_, ()>(comparable_workout_text_for_payload_hash(
        projected_workout_name(day).as_deref(),
        Some(serialized.as_str()),
    ))
    .ok()
    .flatten()
}

pub(super) fn comparable_workout_text_for_payload_hash(
    name: Option<&str>,
    workout_text: Option<&str>,
) -> Option<String> {
    let workout_text = workout_text
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let Some(name) = name.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(workout_text.to_string());
    };

    let mut lines = workout_text.lines();
    let Some(first_line) = lines.next() else {
        return Some(workout_text.to_string());
    };
    if first_line.trim() != name {
        return Some(workout_text.to_string());
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        Some(name.to_string())
    } else {
        Some(body)
    }
}

fn sync_states_for_status<'a>(
    sync_states: &'a [ExternalSyncState],
    just_synced_state: Option<&'a ExternalSyncState>,
) -> Vec<&'a ExternalSyncState> {
    let mut states = sync_states.iter().collect::<Vec<_>>();
    if let Some(just_synced_state) = just_synced_state {
        states.retain(|state| state.provider != just_synced_state.provider);
        states.push(just_synced_state);
    }
    states
}

fn linked_intervals_event_id(
    sync_states: &[ExternalSyncState],
    just_synced_state: Option<&ExternalSyncState>,
) -> Option<i64> {
    sync_states_for_status(sync_states, just_synced_state)
        .into_iter()
        .find(|state| state.provider == ExternalProvider::Intervals)
        .and_then(|state| state.external_id.as_deref())
        .and_then(|value| value.parse::<i64>().ok())
}
