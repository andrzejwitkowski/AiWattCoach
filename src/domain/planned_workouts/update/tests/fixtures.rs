use crate::domain::{
    external_sync::{CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState},
    intervals::{Event, EventCategory},
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutText,
        UpdatePlannedWorkoutCommand,
    },
};

pub fn existing_workout() -> PlannedWorkout {
    PlannedWorkout::new(
        "training-plan:user-1:w1:2026-05-10".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                text: "Existing Session".to_string(),
            })],
        },
    )
}

pub fn update_command() -> UpdatePlannedWorkoutCommand {
    UpdatePlannedWorkoutCommand {
        user_id: "user-1".to_string(),
        planned_workout_id: "training-plan:user-1:w1:2026-05-10".to_string(),
        date: "2026-05-10".to_string(),
        workout_doc: "Warmup\n- 5m 60%".to_string(),
    }
}

pub fn intervals_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "training-plan:user-1:w1:2026-05-10".to_string(),
        ),
    )
    .mark_synced("77".to_string(), "old-hash".to_string(), 1_700_000_001)
}

pub fn existing_intervals_event() -> Event {
    Event {
        id: 77,
        start_date_local: "2026-05-10T00:00:00".to_string(),
        event_type: Some("Ride".to_string()),
        name: Some("Existing Session".to_string()),
        category: EventCategory::Workout,
        description: Some("manual note".to_string()),
        indoor: false,
        color: Some("blue".to_string()),
        workout_doc: None,
    }
}

pub fn wahoo_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Wahoo,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "training-plan:user-1:w1:2026-05-10".to_string(),
        ),
    )
    .mark_wahoo_pending("training-plan:user-1:w1:2026-05-10".to_string())
    .mark_wahoo_synced(
        "old-hash".to_string(),
        1_700_000_001,
        "training-plan:user-1:w1:2026-05-10".to_string(),
        5001,
        6001,
        "[AIWATTCOACH:pw=ABC123EF45]".to_string(),
    )
}

pub fn wahoo_sync_state_without_token() -> ExternalSyncState {
    let mut state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Wahoo,
        CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            "training-plan:user-1:w1:2026-05-10".to_string(),
        ),
    )
    .mark_wahoo_pending("training-plan:user-1:w1:2026-05-10".to_string())
    .mark_wahoo_synced(
        "old-hash".to_string(),
        1_700_000_001,
        "training-plan:user-1:w1:2026-05-10".to_string(),
        5001,
        6001,
        "[AIWATTCOACH:pw=ABC123EF45]".to_string(),
    );
    state.wahoo_workout_token = None;
    state
}
