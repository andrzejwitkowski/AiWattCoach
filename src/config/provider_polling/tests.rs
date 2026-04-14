use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar_view::{
        BoxFuture as CalendarViewBoxFuture, CalendarEntryView, CalendarEntryViewError,
        CalendarEntryViewRefreshPort,
    },
    external_sync::{
        BoxFuture as SyncBoxFuture, ExternalImportCommand, ExternalImportError,
        ExternalImportOutcome, ExternalImportUseCases, ExternalProvider,
        ExternalSyncRepositoryError, ProviderPollState, ProviderPollStateRepository,
        ProviderPollStream,
    },
    identity::{Clock, IdGenerator},
    intervals::{
        Activity, ActivityDetails, ActivityMetrics, DateRange, Event, EventCategory,
        IntervalsApiPort, IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
    },
};

use super::ProviderPollingService;

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
    assert_eq!(stored.cursor.as_deref(), Some("2026-05-10"));
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
async fn poll_due_once_persists_attempt_before_calling_intervals() {
    let shared_states = Arc::new(Mutex::new(vec![ProviderPollState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        ProviderPollStream::Calendar,
        1_699_999_900,
    )]));
    let poll_states = RecordingProviderPollStateRepository {
        states: shared_states.clone(),
    };
    let service = ProviderPollingService::new(
        AssertingIntervalsApi::new(shared_states),
        FakeIntervalsSettings,
        poll_states,
        RecordingImportService::default(),
        FixedClock,
        FixedIdGenerator,
    )
    .with_windows(7, 14, 7);

    service.poll_due_once().await.unwrap();
}

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
struct FixedIdGenerator;

impl IdGenerator for FixedIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-generated-1")
    }
}

#[derive(Clone, Default)]
struct RecordingImportService {
    commands: Arc<Mutex<Vec<ExternalImportCommand>>>,
    failure: Option<String>,
}

impl RecordingImportService {
    fn failing(message: &str) -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            failure: Some(message.to_string()),
        }
    }

    fn commands(&self) -> Vec<ExternalImportCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl ExternalImportUseCases for RecordingImportService {
    fn import(
        &self,
        command: ExternalImportCommand,
    ) -> SyncBoxFuture<Result<ExternalImportOutcome, ExternalImportError>> {
        let commands = self.commands.clone();
        let failure = self.failure.clone();
        Box::pin(async move {
            commands.lock().unwrap().push(command.clone());
            if let Some(message) = failure {
                return Err(ExternalImportError::Repository(message));
            }

            let (canonical_entity, provider, external_id) = match command {
                ExternalImportCommand::UpsertPlannedWorkout(import) => (
                    crate::domain::external_sync::CanonicalEntityRef::new(
                        crate::domain::external_sync::CanonicalEntityKind::PlannedWorkout,
                        import.workout.planned_workout_id,
                    ),
                    import.provider,
                    import.external_id,
                ),
                ExternalImportCommand::UpsertCompletedWorkout(import) => (
                    crate::domain::external_sync::CanonicalEntityRef::new(
                        crate::domain::external_sync::CanonicalEntityKind::CompletedWorkout,
                        import.workout.completed_workout_id,
                    ),
                    import.provider,
                    import.external_id,
                ),
                ExternalImportCommand::UpsertRace(import) => (
                    crate::domain::external_sync::CanonicalEntityRef::new(
                        crate::domain::external_sync::CanonicalEntityKind::Race,
                        import.race.race_id,
                    ),
                    import.provider,
                    import.external_id,
                ),
                ExternalImportCommand::UpsertSpecialDay(import) => (
                    crate::domain::external_sync::CanonicalEntityRef::new(
                        crate::domain::external_sync::CanonicalEntityKind::SpecialDay,
                        import.special_day.special_day_id,
                    ),
                    import.provider,
                    import.external_id,
                ),
            };

            Ok(ExternalImportOutcome {
                canonical_entity,
                provider,
                external_id,
            })
        })
    }
}

#[derive(Clone)]
struct FakeIntervalsSettings;

impl IntervalsSettingsPort for FakeIntervalsSettings {
    fn get_credentials(
        &self,
        _user_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
        Box::pin(async {
            Ok(IntervalsCredentials {
                api_key: "test-key".to_string(),
                athlete_id: "athlete-1".to_string(),
            })
        })
    }
}

#[derive(Clone, Default)]
struct FakeIntervalsApi {
    events: Vec<Event>,
    activities: Vec<Activity>,
}

#[derive(Clone, Default)]
struct RecordingIntervalsApi {
    event_ranges: Arc<Mutex<Vec<(String, String)>>>,
    activity_ranges: Arc<Mutex<Vec<(String, String)>>>,
}

impl FakeIntervalsApi {
    fn with_events(events: Vec<Event>) -> Self {
        Self {
            events,
            activities: Vec::new(),
        }
    }

    fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            events: Vec::new(),
            activities,
        }
    }
}

impl RecordingIntervalsApi {
    fn event_ranges(&self) -> Vec<(String, String)> {
        self.event_ranges.lock().unwrap().clone()
    }

    fn activity_ranges(&self) -> Vec<(String, String)> {
        self.activity_ranges.lock().unwrap().clone()
    }
}

impl IntervalsApiPort for FakeIntervalsApi {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let events = self.events.clone();
        Box::pin(async move { Ok(events) })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event: crate::domain::intervals::CreateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
        _event: crate::domain::intervals::UpdateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn list_activities(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let activities = self.activities.clone();
        Box::pin(async move { Ok(activities) })
    }
}

impl IntervalsApiPort for RecordingIntervalsApi {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let event_ranges = self.event_ranges.clone();
        let range = range.clone();
        Box::pin(async move {
            event_ranges
                .lock()
                .unwrap()
                .push((range.oldest, range.newest));
            Ok(Vec::new())
        })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event: crate::domain::intervals::CreateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
        _event: crate::domain::intervals::UpdateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn list_activities(
        &self,
        _credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let activity_ranges = self.activity_ranges.clone();
        let range = range.clone();
        Box::pin(async move {
            activity_ranges
                .lock()
                .unwrap()
                .push((range.oldest, range.newest));
            Ok(Vec::new())
        })
    }
}

#[derive(Clone)]
struct AssertingIntervalsApi {
    states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl AssertingIntervalsApi {
    fn new(states: Arc<Mutex<Vec<ProviderPollState>>>) -> Self {
        Self { states }
    }
}

impl IntervalsApiPort for AssertingIntervalsApi {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let states = self.states.clone();
        Box::pin(async move {
            let attempted = states
                .lock()
                .unwrap()
                .iter()
                .find(|state| state.stream == ProviderPollStream::Calendar)
                .and_then(|state| state.last_attempted_at_epoch_seconds);
            assert_eq!(attempted, Some(1_700_000_000));
            Ok(Vec::new())
        })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event: crate::domain::intervals::CreateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
        _event: crate::domain::intervals::UpdateEvent,
    ) -> crate::domain::intervals::BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> crate::domain::intervals::BoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { unreachable!("not used in test") })
    }
}

#[derive(Clone)]
struct RecordingProviderPollStateRepository {
    states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl RecordingProviderPollStateRepository {
    fn with_states(states: Vec<ProviderPollState>) -> Self {
        Self {
            states: Arc::new(Mutex::new(states)),
        }
    }
}

impl ProviderPollStateRepository for RecordingProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> SyncBoxFuture<Result<ProviderPollState, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        Box::pin(async move {
            let mut states = states.lock().unwrap();
            states.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.stream == state.stream)
            });
            states.push(state.clone());
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> SyncBoxFuture<Result<Vec<ProviderPollState>, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.is_due(now_epoch_seconds))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> SyncBoxFuture<Result<Option<ProviderPollState>, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id && state.provider == provider && state.stream == stream
                })
                .cloned())
        })
    }
}

#[derive(Clone, Default)]
struct RecordingCalendarRefresh {
    ranges: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    fn ranges(&self) -> Vec<(String, String, String)> {
        self.ranges.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> CalendarViewBoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let ranges = self.ranges.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            ranges.lock().unwrap().push((user_id, oldest, newest));
            Ok(Vec::new())
        })
    }
}

fn sample_activity(activity_id: &str) -> Activity {
    Activity {
        id: activity_id.to_string(),
        athlete_id: None,
        start_date_local: "2023-11-14T08:00:00".to_string(),
        start_date: None,
        name: Some("Threshold Ride".to_string()),
        description: Some("Strong day".to_string()),
        activity_type: Some("Ride".to_string()),
        source: Some("intervals".to_string()),
        external_id: Some("external-1".to_string()),
        device_name: Some("Trainer".to_string()),
        distance_meters: Some(35_000.0),
        moving_time_seconds: Some(3600),
        elapsed_time_seconds: Some(3700),
        total_elevation_gain_meters: Some(400.0),
        total_elevation_loss_meters: Some(400.0),
        average_speed_mps: Some(9.7),
        max_speed_mps: Some(15.0),
        average_heart_rate_bpm: Some(150),
        max_heart_rate_bpm: Some(175),
        average_cadence_rpm: Some(88.0),
        trainer: true,
        commute: false,
        race: false,
        has_heart_rate: true,
        stream_types: vec!["watts".to_string()],
        tags: vec!["quality".to_string()],
        metrics: ActivityMetrics {
            training_stress_score: None,
            normalized_power_watts: None,
            intensity_factor: None,
            efficiency_factor: None,
            variability_index: None,
            average_power_watts: None,
            ftp_watts: None,
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        details: ActivityDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: Vec::new(),
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
        details_unavailable_reason: None,
    }
}
