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
        Activity, ActivityDetails, ActivityMetrics, DateRange, Event, IntervalsApiPort,
        IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
    },
};

#[derive(Clone)]
pub(super) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(super) struct FixedIdGenerator;

impl IdGenerator for FixedIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-generated-1")
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingImportService {
    commands: Arc<Mutex<Vec<ExternalImportCommand>>>,
    failure: Option<String>,
}

impl RecordingImportService {
    pub(super) fn failing(message: &str) -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            failure: Some(message.to_string()),
        }
    }

    pub(super) fn commands(&self) -> Vec<ExternalImportCommand> {
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
pub(super) struct FakeIntervalsSettings;

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
pub(super) struct FakeIntervalsApi {
    events: Vec<Event>,
    activities: Vec<Activity>,
}

#[derive(Clone, Default)]
pub(super) struct RecordingIntervalsApi {
    event_ranges: Arc<Mutex<Vec<(String, String)>>>,
    activity_ranges: Arc<Mutex<Vec<(String, String)>>>,
}

impl FakeIntervalsApi {
    pub(super) fn with_events(events: Vec<Event>) -> Self {
        Self {
            events,
            activities: Vec::new(),
        }
    }

    pub(super) fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            events: Vec::new(),
            activities,
        }
    }
}

impl RecordingIntervalsApi {
    pub(super) fn event_ranges(&self) -> Vec<(String, String)> {
        self.event_ranges.lock().unwrap().clone()
    }

    pub(super) fn activity_ranges(&self) -> Vec<(String, String)> {
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
pub(super) struct AssertingIntervalsApi {
    states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl AssertingIntervalsApi {
    pub(super) fn new(states: Arc<Mutex<Vec<ProviderPollState>>>) -> Self {
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
pub(super) struct RecordingProviderPollStateRepository {
    pub(super) states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl RecordingProviderPollStateRepository {
    pub(super) fn with_states(states: Vec<ProviderPollState>) -> Self {
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
pub(super) struct RecordingCalendarRefresh {
    ranges: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    pub(super) fn ranges(&self) -> Vec<(String, String, String)> {
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

pub(super) fn sample_activity(activity_id: &str) -> Activity {
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
