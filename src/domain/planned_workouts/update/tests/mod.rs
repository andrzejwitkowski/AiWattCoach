use std::sync::{Arc, Mutex};

mod fixtures;
mod support;

use crate::domain::{
    calendar::NoopWahooUseCases,
    external_sync::{ExternalProvider, ExternalSyncState, ExternalSyncStatus},
    intervals::IntervalsUseCases,
    planned_workout_tokens::{NoopPlannedWorkoutTokenRepository, PlannedWorkoutTokenRepository},
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutRepeat,
        PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    settings::{NoopUserSettingsRepository, UserSettingsRepository},
    wahoo::WahooUseCases,
};

use super::{
    comparable_workout_text_for_payload_hash, map_planned_workout_to_syncable,
    preserve_event_description, PlannedWorkoutUpdateService,
};

use fixtures::{
    existing_intervals_event, existing_workout, intervals_sync_state, update_command,
    wahoo_sync_state, wahoo_sync_state_without_token,
};
use support::{
    FixedClock, InMemoryExternalSyncStateRepository, InMemoryUserSettingsRepository,
    RecordingCalendarRefresh, RecordingIntervalsService, RecordingPlannedWorkoutRepository,
    RecordingWahooService,
};

// ---------------------------------------------------------------------------
// Unit tests for pure helper functions
// ---------------------------------------------------------------------------

#[test]
fn map_planned_workout_to_syncable_preserves_name_and_hashable_body() {
    let workout = PlannedWorkout::new(
        "training-plan:user-1:w1:2026-05-10".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Threshold Builder".to_string(),
                }),
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Warmup".to_string(),
                }),
            ],
        },
    );

    let syncable = map_planned_workout_to_syncable(&workout).expect("syncable workout");

    assert_eq!(syncable.planned_workout_id, workout.planned_workout_id);
    assert_eq!(syncable.date, workout.date);
    assert_eq!(syncable.name.as_deref(), Some("Threshold Builder"));
    assert!(!syncable.payload_hash().is_empty());
}

#[test]
fn syncable_minutes_expand_repeat_blocks() {
    let workout = PlannedWorkout::new(
        "training-plan:user-1:w1:2026-05-10".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Repeat Session".to_string(),
                }),
                PlannedWorkoutLine::Repeat(PlannedWorkoutRepeat {
                    title: Some("Main Set".to_string()),
                    count: 3,
                }),
                PlannedWorkoutLine::Step(PlannedWorkoutStep {
                    duration_seconds: 120,
                    kind: PlannedWorkoutStepKind::Steady,
                    target: PlannedWorkoutTarget::PercentFtp {
                        min: 80.0,
                        max: 80.0,
                    },
                }),
            ],
        },
    );

    let syncable = map_planned_workout_to_syncable(&workout).expect("syncable workout");

    assert_eq!(syncable.minutes().expect("duration"), 6);
}

#[test]
fn preserve_event_description_returns_none_when_both_absent() {
    assert_eq!(preserve_event_description(None, None, None), None);
}

#[test]
fn preserve_event_description_returns_projected_when_existing_is_absent() {
    assert_eq!(
        preserve_event_description(None, Some("workout body"), None),
        Some("workout body".to_string())
    );
}

#[test]
fn preserve_event_description_returns_existing_when_projected_is_absent() {
    assert_eq!(
        preserve_event_description(Some("coach note"), None, None),
        Some("coach note".to_string())
    );
}

#[test]
fn preserve_event_description_returns_existing_when_it_already_contains_projected() {
    assert_eq!(
        preserve_event_description(
            Some("coach note\n\nworkout body"),
            Some("workout body"),
            None
        ),
        Some("coach note\n\nworkout body".to_string())
    );
}

#[test]
fn preserve_event_description_appends_projected_when_existing_lacks_it() {
    assert_eq!(
        preserve_event_description(Some("coach note"), Some("workout body"), None),
        Some("coach note\n\nworkout body".to_string())
    );
}

#[test]
fn preserve_event_description_replaces_previous_generated_workout_body() {
    assert_eq!(
        preserve_event_description(
            Some("coach note\n\nOld workout\n- 10m 60%"),
            Some("New workout\n- 20m 70%"),
            Some("Old workout\n- 10m 60%"),
        ),
        Some("coach note\n\nNew workout\n- 20m 70%".to_string())
    );
}

#[test]
fn preserve_event_description_replaces_legacy_generated_body_when_workout_doc_missing() {
    assert_eq!(
        preserve_event_description(
            Some("coach note\n\nNew workout\n- 10m 60%"),
            Some("New workout\n- 20m 70%"),
            None,
        ),
        Some("coach note\n\nNew workout\n- 20m 70%".to_string())
    );
}

#[test]
fn comparable_workout_text_returns_none_when_workout_text_absent() {
    assert_eq!(
        comparable_workout_text_for_payload_hash(Some("Warmup"), None),
        None
    );
}

#[test]
fn comparable_workout_text_returns_full_text_when_name_absent() {
    assert_eq!(
        comparable_workout_text_for_payload_hash(None, Some("Warmup\n- 5m 60%")),
        Some("Warmup\n- 5m 60%".to_string())
    );
}

#[test]
fn comparable_workout_text_strips_leading_name_line_when_name_matches_first_line() {
    assert_eq!(
        comparable_workout_text_for_payload_hash(Some("Warmup"), Some("Warmup\n- 5m 60%")),
        Some("- 5m 60%".to_string())
    );
}

#[test]
fn comparable_workout_text_returns_full_text_when_first_line_differs_from_name() {
    assert_eq!(
        comparable_workout_text_for_payload_hash(
            Some("Threshold Builder"),
            Some("Warmup\n- 5m 60%")
        ),
        Some("Warmup\n- 5m 60%".to_string())
    );
}

#[test]
fn comparable_workout_text_returns_name_when_body_is_only_name_line() {
    assert_eq!(
        comparable_workout_text_for_payload_hash(Some("Warmup"), Some("Warmup")),
        Some("Warmup".to_string())
    );
}

// ---------------------------------------------------------------------------
// Behaviour tests for PlannedWorkoutUpdateService
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_planned_workout_persists_local_change_and_refreshes_when_no_sync_state_exists() {
    let planned_workouts =
        RecordingPlannedWorkoutRepository::with_workouts(vec![existing_workout()]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts.clone(),
        sync_states.clone(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        NoopUserSettingsRepository,
        NoopPlannedWorkoutTokenRepository::default(),
        refresh.clone(),
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("local update should succeed");

    assert_eq!(outcome.synced_providers, Vec::<ExternalProvider>::new());
    assert_eq!(
        outcome.planned_workout.planned_workout_id,
        existing_workout().planned_workout_id
    );
    assert_eq!(planned_workouts.upserted().len(), 1);
    assert_eq!(planned_workouts.stored().len(), 1);
    assert_eq!(sync_states.stored(), Vec::<ExternalSyncState>::new());
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
    assert_eq!(
        planned_workouts.operation_log(),
        vec!["planned_workouts.upsert".to_string()]
    );
}

#[tokio::test]
async fn update_planned_workout_marks_intervals_state_modified_then_synced_after_remote_update() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned_workouts = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![intervals_sync_state()],
        shared_log.clone(),
    );
    let intervals = RecordingIntervalsService::with_existing_event_and_shared_log(
        existing_intervals_event(),
        shared_log.clone(),
    );
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts.clone(),
        sync_states.clone(),
        intervals.clone(),
        NoopWahooUseCases,
        NoopUserSettingsRepository,
        NoopPlannedWorkoutTokenRepository::default(),
        refresh.clone(),
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("intervals-backed update should succeed");

    assert_eq!(outcome.synced_providers, vec![ExternalProvider::Intervals]);
    assert_eq!(intervals.updated_events().len(), 1);
    assert_eq!(intervals.updated_events()[0].0, 77);
    assert_eq!(
        intervals.updated_events()[0].1.start_date_local.as_deref(),
        Some("2026-05-10T00:00:00")
    );
    assert_eq!(
        intervals.updated_events()[0].1.name.as_deref(),
        Some("Warmup")
    );
    assert_eq!(
        intervals.updated_events()[0].1.description.as_deref(),
        Some("manual note\n\n- 5m 60%")
    );
    assert_eq!(intervals.updated_events()[0].1.workout_doc, None);
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(stored_states[0].sync_status, ExternalSyncStatus::Synced);
    assert_eq!(stored_states[0].external_id.as_deref(), Some("77"));
    assert!(stored_states[0].last_error.is_none());
    assert_eq!(
        shared_log
            .lock()
            .expect("shared log mutex poisoned")
            .clone(),
        vec![
            "planned_workouts.upsert".to_string(),
            "sync_states.upsert:modified".to_string(),
            "intervals.get_event".to_string(),
            "intervals.update_event".to_string(),
            "sync_states.upsert:synced".to_string(),
        ]
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
}

#[tokio::test]
async fn update_planned_workout_keeps_local_change_when_intervals_update_fails() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned_workouts = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![intervals_sync_state()],
        shared_log.clone(),
    );
    let intervals = RecordingIntervalsService::with_failed_update_and_shared_log(
        existing_intervals_event(),
        shared_log.clone(),
    );
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts.clone(),
        sync_states.clone(),
        intervals,
        NoopWahooUseCases,
        NoopUserSettingsRepository,
        NoopPlannedWorkoutTokenRepository::default(),
        refresh.clone(),
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("local update should survive remote failure");

    assert_eq!(outcome.synced_providers, Vec::<ExternalProvider>::new());
    let stored_workouts = planned_workouts.stored();
    assert_eq!(stored_workouts.len(), 1);
    assert_eq!(
        stored_workouts[0].planned_workout_id,
        existing_workout().planned_workout_id
    );
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(stored_states[0].sync_status, ExternalSyncStatus::Failed);
    assert_eq!(stored_states[0].last_error.as_deref(), Some("boom"));
    assert_eq!(
        stored_states[0].last_seen_remote_payload_hash,
        intervals_sync_state().last_seen_remote_payload_hash
    );
    assert_eq!(
        shared_log
            .lock()
            .expect("shared log mutex poisoned")
            .clone(),
        vec![
            "planned_workouts.upsert".to_string(),
            "sync_states.upsert:modified".to_string(),
            "intervals.get_event".to_string(),
            "intervals.update_event".to_string(),
            "sync_states.upsert:failed".to_string(),
        ]
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
}

#[tokio::test]
async fn update_planned_workout_updates_existing_wahoo_plan_and_workout() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned_workouts = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![wahoo_sync_state()],
        shared_log.clone(),
    );
    let wahoo = RecordingWahooService::successful(shared_log.clone());
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts.clone(),
        sync_states.clone(),
        RecordingIntervalsService::default(),
        wahoo.clone(),
        InMemoryUserSettingsRepository::with_ftp(295),
        NoopPlannedWorkoutTokenRepository::default(),
        refresh.clone(),
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("wahoo-backed update should succeed");

    assert_eq!(outcome.synced_providers, vec![ExternalProvider::Wahoo]);
    assert_eq!(wahoo.updated_plans().len(), 1);
    assert_eq!(wahoo.updated_plans()[0].0, 5001);
    assert_eq!(
        wahoo.updated_plans()[0].1.filename.as_deref(),
        Some("training-plan:user-1:w1:2026-05-10.plan.json")
    );
    assert_eq!(wahoo.updated_workouts().len(), 1);
    assert_eq!(wahoo.updated_workouts()[0].0, 6001);
    assert_eq!(wahoo.updated_workouts()[0].1.minutes, Some(5));
    assert_eq!(
        wahoo.updated_workouts()[0].1.name.as_deref(),
        Some("Warmup")
    );
    assert_eq!(
        wahoo.updated_workouts()[0].1.starts.as_deref(),
        Some("2026-05-10T00:00:00.000Z")
    );
    assert_eq!(wahoo.updated_workouts()[0].1.plan_id, Some(5001));
    assert_eq!(
        wahoo.updated_workouts()[0].1.workout_token.as_deref(),
        Some("[AIWATTCOACH:pw=ABC123EF45]")
    );
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(stored_states[0].sync_status, ExternalSyncStatus::Synced);
    assert_eq!(stored_states[0].wahoo_plan_id, Some(5001));
    assert_eq!(stored_states[0].wahoo_workout_id, Some(6001));
    assert!(stored_states[0].last_error.is_none());
    assert_eq!(
        shared_log
            .lock()
            .expect("shared log mutex poisoned")
            .clone(),
        vec![
            "planned_workouts.upsert".to_string(),
            "sync_states.upsert:modified".to_string(),
            "wahoo.update_plan".to_string(),
            "wahoo.update_workout".to_string(),
            "sync_states.upsert:synced".to_string(),
        ]
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
}

#[tokio::test]
async fn update_planned_workout_keeps_local_change_when_wahoo_update_fails() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned_workouts = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![wahoo_sync_state()],
        shared_log.clone(),
    );
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts.clone(),
        sync_states.clone(),
        RecordingIntervalsService::default(),
        RecordingWahooService::failing("wahoo unavailable", shared_log.clone()),
        InMemoryUserSettingsRepository::with_ftp(295),
        NoopPlannedWorkoutTokenRepository::default(),
        refresh.clone(),
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("local update should survive wahoo failure");

    assert_eq!(outcome.synced_providers, Vec::<ExternalProvider>::new());
    let stored_workouts = planned_workouts.stored();
    assert_eq!(stored_workouts.len(), 1);
    assert_eq!(
        stored_workouts[0].planned_workout_id,
        existing_workout().planned_workout_id
    );
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(stored_states[0].sync_status, ExternalSyncStatus::Failed);
    assert_eq!(
        stored_states[0].last_error.as_deref(),
        Some("wahoo unavailable")
    );
    assert_eq!(
        shared_log
            .lock()
            .expect("shared log mutex poisoned")
            .clone(),
        vec![
            "planned_workouts.upsert".to_string(),
            "sync_states.upsert:modified".to_string(),
            "wahoo.update_plan".to_string(),
            "sync_states.upsert:failed".to_string(),
        ]
    );
    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-05-10".to_string(),
            "2026-05-10".to_string(),
        )]
    );
}

#[tokio::test]
async fn update_planned_workout_generates_missing_wahoo_token_before_updating_workout() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned_workouts = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![wahoo_sync_state_without_token()],
        shared_log.clone(),
    );
    let wahoo = RecordingWahooService::successful(shared_log.clone());
    let refresh = RecordingCalendarRefresh::default();
    let service = build_service(
        planned_workouts,
        sync_states.clone(),
        RecordingIntervalsService::default(),
        wahoo.clone(),
        InMemoryUserSettingsRepository::with_ftp(295),
        NoopPlannedWorkoutTokenRepository::default(),
        refresh,
    );

    let outcome = service
        .update_planned_workout(update_command())
        .await
        .expect("wahoo update should create missing token");

    assert_eq!(outcome.synced_providers, vec![ExternalProvider::Wahoo]);
    assert_eq!(wahoo.updated_workouts().len(), 1);
    let generated_token = wahoo.updated_workouts()[0]
        .1
        .workout_token
        .clone()
        .expect("generated workout token");
    assert!(generated_token.starts_with("[AIWATTCOACH:pw="));
    let stored_states = sync_states.stored();
    assert_eq!(stored_states.len(), 1);
    assert_eq!(stored_states[0].sync_status, ExternalSyncStatus::Synced);
    assert_eq!(
        stored_states[0].wahoo_workout_token.as_deref(),
        Some(generated_token.as_str())
    );
}

// ---------------------------------------------------------------------------
// Test helper
// ---------------------------------------------------------------------------

fn build_service<Intervals, Wahoo, Settings, Tokens>(
    planned_workouts: RecordingPlannedWorkoutRepository,
    sync_states: InMemoryExternalSyncStateRepository,
    intervals: Intervals,
    wahoo: Wahoo,
    settings: Settings,
    planned_workout_tokens: Tokens,
    refresh: RecordingCalendarRefresh,
) -> PlannedWorkoutUpdateService<
    RecordingPlannedWorkoutRepository,
    InMemoryExternalSyncStateRepository,
    Intervals,
    Wahoo,
    Settings,
    Tokens,
    RecordingCalendarRefresh,
    FixedClock,
>
where
    Intervals: IntervalsUseCases + Clone,
    Wahoo: WahooUseCases + Clone,
    Settings: UserSettingsRepository,
    Tokens: PlannedWorkoutTokenRepository,
{
    PlannedWorkoutUpdateService::new(
        planned_workouts,
        sync_states,
        intervals,
        wahoo,
        settings,
        planned_workout_tokens,
        refresh,
        FixedClock,
    )
}
