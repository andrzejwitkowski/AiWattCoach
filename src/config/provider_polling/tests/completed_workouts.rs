use crate::domain::external_sync::{
    ExternalImportCommand, ExternalProvider, ProviderPollState, ProviderPollStateRepository,
    ProviderPollStream,
};
use crate::domain::intervals::IntervalsError;

use super::{support::*, ProviderPollingService};

#[tokio::test]
async fn first_completed_sync_without_activities_advances_cursor_to_window_end() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]);
    let service = ProviderPollingService::new(
        RecordingIntervalsApi::default(),
        FakeIntervalsSettings,
        poll_states.clone(),
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    service.poll_due_once().await.unwrap();

    let stored = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.cursor.as_deref(), Some("2023-11-14"));
}

#[tokio::test]
async fn completed_stream_uses_independent_cursor() {
    let mut state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::CompletedWorkouts,
        1_699_999_900,
    );
    state.cursor = Some("2023-11-10".to_string());
    let api = RecordingIntervalsApi::default();
    let service = ProviderPollingService::new(
        api.clone(),
        FakeIntervalsSettings,
        RecordingProviderPollStateRepository::with_states(vec![state]),
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7)
    .with_incremental_lookback(1);

    service.poll_due_once().await.unwrap();

    assert_eq!(
        api.activity_ranges(),
        vec![("2023-11-09".to_string(), "2023-11-14".to_string())]
    );
}

#[tokio::test]
async fn poll_due_once_marks_failure_and_backoff_when_import_fails() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]);
    let service = ProviderPollingService::new(
        FakeIntervalsApi::with_activities(vec![sample_activity("activity-1")]),
        FakeIntervalsSettings,
        poll_states.clone(),
        RecordingImportService::failing("import exploded"),
        FixedClock,
        FixedIdGenerator,
    )
    .with_timing(300, 120)
    .with_windows(7, 14, 7);

    let processed = service.poll_due_once().await.unwrap();

    assert_eq!(processed, 1);
    let stored = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .expect("expected stored poll state");
    assert_eq!(stored.last_attempted_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(stored.last_successful_at_epoch_seconds, None);
    assert_eq!(stored.last_error.as_deref(), Some("import exploded"));
    assert_eq!(stored.backoff_until_epoch_seconds, Some(1_700_000_120));
    assert_eq!(stored.next_due_at_epoch_seconds, 1_700_000_120);
}

#[tokio::test]
async fn completed_stream_enriches_activity_details_before_import() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]);
    let imports = RecordingImportService::default();
    let listed = sample_activity("activity-1");
    let detailed = sample_detailed_activity("activity-1");
    let service = ProviderPollingService::new(
        FakeIntervalsApi::with_activities_and_details(vec![listed], vec![detailed]),
        FakeIntervalsSettings,
        poll_states,
        imports.clone(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    service.poll_due_once().await.unwrap();

    let commands = imports.commands();
    assert_eq!(commands.len(), 1);

    let ExternalImportCommand::UpsertCompletedWorkout(import) = &commands[0] else {
        panic!("expected completed workout import");
    };

    assert!(!import.workout.details.streams.is_empty());
    assert!(!import.workout.details.intervals.is_empty());
    assert!(!import.workout.details.interval_groups.is_empty());
}

#[tokio::test]
async fn completed_stream_imports_listed_activity_when_detail_enrichment_is_not_found() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]);
    let imports = RecordingImportService::default();
    let listed = sample_activity("activity-1");
    let api = RecordingIntervalsApi::default();
    let service = ProviderPollingService::new(
        api.clone(),
        FakeIntervalsSettings,
        poll_states,
        imports.clone(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    let listed_service = ProviderPollingService::new(
        FakeIntervalsApi::with_activities(vec![listed]),
        FakeIntervalsSettings,
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]),
        imports.clone(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    service.poll_due_once().await.unwrap();
    listed_service.poll_due_once().await.unwrap();

    assert_eq!(api.activity_lookups(), Vec::<String>::new());

    let commands = imports.commands();
    assert_eq!(commands.len(), 1);

    let ExternalImportCommand::UpsertCompletedWorkout(import) = &commands[0] else {
        panic!("expected completed workout import");
    };

    assert!(import.workout.details.streams.is_empty());
    assert!(import.workout.details.intervals.is_empty());
    assert!(import.workout.details.interval_groups.is_empty());
}

#[tokio::test]
async fn completed_stream_fails_poll_when_detail_enrichment_has_transient_error() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
            1_699_999_900,
        )]);
    let imports = RecordingImportService::default();
    let listed = sample_activity("activity-1");
    let service = ProviderPollingService::new(
        FakeIntervalsApi::with_activities_and_detail_errors(
            vec![listed],
            vec![(
                "activity-1".to_string(),
                IntervalsError::ConnectionError("timeout".to_string()),
            )],
        ),
        FakeIntervalsSettings,
        poll_states.clone(),
        imports.clone(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_timing(300, 120)
    .with_windows(7, 14, 7);

    let processed = service.poll_due_once().await.unwrap();

    assert_eq!(processed, 1);
    assert!(imports.commands().is_empty());

    let stored = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.last_successful_at_epoch_seconds, None);
    assert_eq!(stored.last_error.as_deref(), Some("completed workout enrichment failed for activity activity-1: Connection error: timeout"));
    assert_eq!(stored.backoff_until_epoch_seconds, Some(1_700_000_120));
}
