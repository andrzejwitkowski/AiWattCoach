use super::super::*;
use super::support::{
    InMemoryProviderPollStateRepository, InMemoryUserSettingsRepository, TestClock,
};
use crate::domain::external_sync::{
    ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
};

#[tokio::test]
async fn update_intervals_preserves_requested_connection_state_and_seeds_due_poll_states() {
    let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let poll_states = InMemoryProviderPollStateRepository::default();
    let service = UserSettingsService::new(repository, TestClock)
        .with_provider_poll_states(poll_states.clone());

    let updated = service
        .update_intervals(
            "user-1",
            IntervalsConfig {
                api_key: Some("api-key".to_string()),
                athlete_id: Some("athlete-1".to_string()),
                connected: false,
            },
        )
        .await
        .unwrap();

    assert!(!updated.intervals.connected);
    assert_eq!(updated.intervals.api_key.as_deref(), Some("api-key"));
    assert_eq!(updated.intervals.athlete_id.as_deref(), Some("athlete-1"));

    let stored = poll_states.stored();
    assert_eq!(stored.len(), 1);
    assert!(stored
        .iter()
        .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
}

#[tokio::test]
async fn update_intervals_trims_credentials_and_keeps_empty_values_disconnected() {
    let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let poll_states = InMemoryProviderPollStateRepository::default();
    let service = UserSettingsService::new(repository, TestClock)
        .with_provider_poll_states(poll_states.clone());

    let updated = service
        .update_intervals(
            "user-1",
            IntervalsConfig {
                api_key: Some("  ".to_string()),
                athlete_id: Some(" athlete-1 ".to_string()),
                connected: true,
            },
        )
        .await
        .unwrap();

    assert!(!updated.intervals.connected);
    assert_eq!(updated.intervals.api_key, None);
    assert_eq!(updated.intervals.athlete_id.as_deref(), Some("athlete-1"));
    assert!(poll_states
        .stored()
        .iter()
        .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
}

#[tokio::test]
async fn update_intervals_disconnect_disables_existing_poll_states() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.intervals = IntervalsConfig {
        api_key: Some("old-key".to_string()),
        athlete_id: Some("old-athlete".to_string()),
        connected: true,
    };
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let poll_states = InMemoryProviderPollStateRepository::default();
    poll_states
        .upsert(ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::CompletedWorkouts,
            cursor: Some("2026-05-01".to_string()),
            next_due_at_epoch_seconds: 1_700_000_000,
            last_attempted_at_epoch_seconds: Some(1_699_999_000),
            last_successful_at_epoch_seconds: Some(1_699_999_100),
            last_error: Some("bad auth".to_string()),
            backoff_until_epoch_seconds: Some(1_700_000_300),
        })
        .await
        .unwrap();
    poll_states
        .upsert(ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::Calendar,
            cursor: Some("2026-05-02".to_string()),
            next_due_at_epoch_seconds: 1_700_000_050,
            last_attempted_at_epoch_seconds: Some(1_699_999_200),
            last_successful_at_epoch_seconds: Some(1_699_999_300),
            last_error: Some("calendar stale".to_string()),
            backoff_until_epoch_seconds: Some(1_700_000_400),
        })
        .await
        .unwrap();
    let service = UserSettingsService::new(repository, TestClock)
        .with_provider_poll_states(poll_states.clone());

    let updated = service
        .update_intervals(
            "user-1",
            IntervalsConfig {
                api_key: None,
                athlete_id: None,
                connected: false,
            },
        )
        .await
        .unwrap();

    assert!(!updated.intervals.connected);
    let stored = poll_states.stored();
    assert_eq!(stored.len(), 2);
    assert!(stored
        .iter()
        .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
    assert!(stored.iter().all(|state| state.cursor.is_none()));
    assert!(stored
        .iter()
        .all(|state| state.backoff_until_epoch_seconds.is_none()));
    assert!(stored.iter().all(|state| state.last_error.is_none()));
}

#[tokio::test]
async fn update_intervals_credential_change_resets_cursor_for_fresh_backfill() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.intervals = IntervalsConfig {
        api_key: Some("old-key".to_string()),
        athlete_id: Some("old-athlete".to_string()),
        connected: true,
    };
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let poll_states = InMemoryProviderPollStateRepository::default();
    poll_states
        .upsert(ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::CompletedWorkouts,
            cursor: Some("2099-01-01".to_string()),
            next_due_at_epoch_seconds: 1_700_099_999,
            last_attempted_at_epoch_seconds: Some(1_699_999_000),
            last_successful_at_epoch_seconds: Some(1_699_999_100),
            last_error: Some("stale".to_string()),
            backoff_until_epoch_seconds: Some(1_700_000_300),
        })
        .await
        .unwrap();
    let service = UserSettingsService::new(repository, TestClock)
        .with_provider_poll_states(poll_states.clone());

    service
        .update_intervals(
            "user-1",
            IntervalsConfig {
                api_key: Some("new-key".to_string()),
                athlete_id: Some("new-athlete".to_string()),
                connected: false,
            },
        )
        .await
        .unwrap();

    let stored = poll_states.stored();
    assert!(stored
        .iter()
        .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
    assert!(stored.iter().all(|state| state.cursor.is_none()));
    assert!(stored
        .iter()
        .all(|state| state.backoff_until_epoch_seconds.is_none()));
    assert!(stored.iter().all(|state| state.last_error.is_none()));
}

#[tokio::test]
async fn update_intervals_without_credential_change_keeps_future_poll_schedule() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.intervals = IntervalsConfig {
        api_key: Some("same-key".to_string()),
        athlete_id: Some("same-athlete".to_string()),
        connected: true,
    };
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let poll_states = InMemoryProviderPollStateRepository::default();
    poll_states
        .upsert(ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::CompletedWorkouts,
            cursor: Some("2026-05-01".to_string()),
            next_due_at_epoch_seconds: 1_700_099_999,
            last_attempted_at_epoch_seconds: Some(1_699_999_000),
            last_successful_at_epoch_seconds: Some(1_699_999_100),
            last_error: Some("transient".to_string()),
            backoff_until_epoch_seconds: Some(1_700_100_100),
        })
        .await
        .unwrap();
    let service = UserSettingsService::new(repository, TestClock)
        .with_provider_poll_states(poll_states.clone());

    service
        .update_intervals(
            "user-1",
            IntervalsConfig {
                api_key: Some("same-key".to_string()),
                athlete_id: Some("same-athlete".to_string()),
                connected: true,
            },
        )
        .await
        .unwrap();

    let stored = poll_states.stored();
    assert_eq!(stored.len(), 1);
    assert!(stored.iter().any(|state| {
        state.stream == ProviderPollStream::CompletedWorkouts
            && state.next_due_at_epoch_seconds == 1_700_099_999
            && state.backoff_until_epoch_seconds == Some(1_700_100_100)
            && state.last_error.as_deref() == Some("transient")
    }));
}
