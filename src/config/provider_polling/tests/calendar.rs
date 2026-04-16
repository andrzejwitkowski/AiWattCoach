use crate::domain::{
    external_sync::{
        ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
    },
    intervals::{Event, EventCategory},
};

use super::{support::*, ProviderPollingService};

#[tokio::test]
async fn poll_due_once_imports_calendar_events_and_marks_success() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
            1_699_999_900,
        )]);
    let imports = RecordingImportService::default();
    let service = ProviderPollingService::new(
        FakeIntervalsApi::with_events(vec![Event {
            id: 144,
            start_date_local: "2026-05-10T00:00:00".to_string(),
            event_type: Some("Ride".to_string()),
            name: Some("Threshold Builder".to_string()),
            category: EventCategory::Workout,
            description: Some("Threshold Builder\n- 10m 90-95%".to_string()),
            indoor: false,
            color: None,
            workout_doc: None,
        }]),
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
    assert_eq!(imports.commands().len(), 1);
    let stored = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .expect("expected stored poll state");
    assert_eq!(stored.last_attempted_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(stored.last_successful_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(stored.last_error, None);
    assert_eq!(stored.backoff_until_epoch_seconds, None);
    assert_eq!(stored.cursor.as_deref(), Some("2023-11-28"));
    assert_eq!(stored.next_due_at_epoch_seconds, 1_700_000_300);
}

#[tokio::test]
async fn first_calendar_sync_uses_backfill_window_and_refreshes_full_range() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
            1_699_999_900,
        )]);
    let refresh = RecordingCalendarRefresh::default();
    let api = RecordingIntervalsApi::default();
    let service = ProviderPollingService::new(
        api.clone(),
        FakeIntervalsSettings,
        poll_states,
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7)
    .with_calendar_view_refresh(refresh.clone());

    service.poll_due_once().await.unwrap();

    assert_eq!(
        api.event_ranges(),
        vec![("2023-11-07".to_string(), "2023-11-28".to_string())]
    );
    assert_eq!(
        refresh.ranges(),
        vec![(
            "user-1".to_string(),
            "2023-11-07".to_string(),
            "2023-11-28".to_string(),
        )]
    );
}

#[tokio::test]
async fn later_calendar_sync_uses_cursor_and_skips_full_range_refresh() {
    let mut state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_699_999_900,
    );
    state.cursor = Some("2023-11-20".to_string());
    let api = RecordingIntervalsApi::default();
    let refresh = RecordingCalendarRefresh::default();
    let service = ProviderPollingService::new(
        api.clone(),
        FakeIntervalsSettings,
        RecordingProviderPollStateRepository::with_states(vec![state]),
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7)
    .with_incremental_lookback(2)
    .with_calendar_view_refresh(refresh.clone());

    service.poll_due_once().await.unwrap();

    assert_eq!(
        api.event_ranges(),
        vec![("2023-11-18".to_string(), "2023-11-28".to_string())]
    );
    assert!(refresh.ranges().is_empty());
}

#[tokio::test]
async fn poll_due_once_keeps_cursor_when_provider_returns_no_new_events() {
    let mut state = ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_699_999_900,
    );
    state.cursor = Some("2026-05-10".to_string());
    let poll_states = RecordingProviderPollStateRepository::with_states(vec![state]);
    let service = ProviderPollingService::new(
        RecordingIntervalsApi::default(),
        FakeIntervalsSettings,
        poll_states.clone(),
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7)
    .with_incremental_lookback(2);

    service.poll_due_once().await.unwrap();

    let stored = poll_states
        .find_by_provider_and_stream(
            "user-1",
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.cursor.as_deref(), Some("2026-05-10"));
}

#[tokio::test]
async fn first_calendar_sync_without_events_advances_cursor_to_window_end() {
    let poll_states =
        RecordingProviderPollStateRepository::with_states(vec![ProviderPollState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
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
            ProviderPollStream::Calendar,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.cursor.as_deref(), Some("2023-11-28"));
}
