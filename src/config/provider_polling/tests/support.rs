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
    training_load::{
        BoxFuture as TrainingLoadBoxFuture, TrainingLoadError, TrainingLoadRecomputeUseCases,
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
    failure_on_call: Option<usize>,
    call_count: Arc<Mutex<usize>>,
}

#[derive(Clone, Default)]
pub(super) struct RecordingTrainingLoadRecomputeService {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingTrainingLoadRecomputeService {
    pub(super) fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl RecordingImportService {
    pub(super) fn failing(message: &str) -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            failure: Some(message.to_string()),
            failure_on_call: None,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn failing_on_call(message: &str, call_number: usize) -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            failure: Some(message.to_string()),
            failure_on_call: Some(call_number),
            call_count: Arc::new(Mutex::new(0)),
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
        let failure_on_call = self.failure_on_call;
        let call_count = self.call_count.clone();
        Box::pin(async move {
            commands.lock().unwrap().push(command.clone());
            let current_call = {
                let mut count = call_count.lock().unwrap();
                *count += 1;
                *count
            };
            if let Some(message) = failure.filter(|_| {
                failure_on_call
                    .map(|call_number| current_call >= call_number)
                    .unwrap_or(true)
            }) {
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

impl TrainingLoadRecomputeUseCases for RecordingTrainingLoadRecomputeService {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> TrainingLoadBoxFuture<Result<(), TrainingLoadError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest_date, now_epoch_seconds));
            Ok(())
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
    detailed_activities: Arc<Mutex<std::collections::HashMap<String, Activity>>>,
    detail_errors: Arc<Mutex<std::collections::HashMap<String, IntervalsError>>>,
}

#[derive(Clone, Default)]
pub(super) struct RecordingIntervalsApi {
    event_ranges: Arc<Mutex<Vec<(String, String)>>>,
    activity_ranges: Arc<Mutex<Vec<(String, String)>>>,
    activity_lookups: Arc<Mutex<Vec<String>>>,
}

impl FakeIntervalsApi {
    pub(super) fn with_events(events: Vec<Event>) -> Self {
        Self {
            events,
            activities: Vec::new(),
            detailed_activities: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detail_errors: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub(super) fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            events: Vec::new(),
            activities,
            detailed_activities: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detail_errors: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub(super) fn with_activities_and_details(
        activities: Vec<Activity>,
        detailed_activities: Vec<Activity>,
    ) -> Self {
        let detailed = detailed_activities
            .into_iter()
            .map(|activity| (activity.id.clone(), activity))
            .collect();
        Self {
            events: Vec::new(),
            activities,
            detailed_activities: Arc::new(Mutex::new(detailed)),
            detail_errors: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub(super) fn with_activities_and_detail_errors(
        activities: Vec<Activity>,
        detail_errors: Vec<(String, IntervalsError)>,
    ) -> Self {
        let detail_errors = detail_errors.into_iter().collect();
        Self {
            events: Vec::new(),
            activities,
            detailed_activities: Arc::new(Mutex::new(std::collections::HashMap::new())),
            detail_errors: Arc::new(Mutex::new(detail_errors)),
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

    pub(super) fn activity_lookups(&self) -> Vec<String> {
        self.activity_lookups.lock().unwrap().clone()
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

    fn get_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        let detailed_activities = self.detailed_activities.clone();
        let detail_errors = self.detail_errors.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(error) = detail_errors.lock().unwrap().get(&activity_id).cloned() {
                return Err(error);
            }
            detailed_activities
                .lock()
                .unwrap()
                .get(&activity_id)
                .cloned()
                .ok_or(IntervalsError::NotFound)
        })
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

    fn get_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> crate::domain::intervals::BoxFuture<Result<Activity, IntervalsError>> {
        let activity_lookups = self.activity_lookups.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            activity_lookups.lock().unwrap().push(activity_id);
            Err(IntervalsError::NotFound)
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

pub(super) fn sample_detailed_activity(activity_id: &str) -> Activity {
    let mut activity = sample_activity(activity_id);
    activity.details = ActivityDetails {
        intervals: vec![crate::domain::intervals::ActivityInterval {
            id: Some(1),
            label: Some("Hard".to_string()),
            interval_type: Some("WORK".to_string()),
            group_id: Some("g1".to_string()),
            start_index: Some(0),
            end_index: Some(59),
            start_time_seconds: Some(0),
            end_time_seconds: Some(60),
            moving_time_seconds: Some(60),
            elapsed_time_seconds: Some(60),
            distance_meters: Some(500.0),
            average_power_watts: Some(320),
            normalized_power_watts: Some(330),
            training_stress_score: Some(12.5),
            average_heart_rate_bpm: Some(165),
            average_cadence_rpm: Some(95.0),
            average_speed_mps: Some(8.1),
            average_stride_meters: None,
            zone: Some(5),
        }],
        interval_groups: vec![crate::domain::intervals::ActivityIntervalGroup {
            id: "g1".to_string(),
            count: Some(1),
            start_index: Some(0),
            moving_time_seconds: Some(60),
            elapsed_time_seconds: Some(60),
            distance_meters: Some(500.0),
            average_power_watts: Some(320),
            normalized_power_watts: Some(330),
            training_stress_score: Some(12.5),
            average_heart_rate_bpm: Some(165),
            average_cadence_rpm: Some(95.0),
            average_speed_mps: Some(8.1),
            average_stride_meters: None,
        }],
        streams: vec![crate::domain::intervals::ActivityStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            data: Some(serde_json::json!([200, 250, 300, 320])),
            data2: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        }],
        interval_summary: vec!["1x 60s 320w".to_string()],
        skyline_chart: vec!["chart".to_string()],
        power_zone_times: vec![crate::domain::intervals::ActivityZoneTime {
            zone_id: "Z5".to_string(),
            seconds: 60,
        }],
        heart_rate_zone_times: vec![60],
        pace_zone_times: Vec::new(),
        gap_zone_times: Vec::new(),
    };
    activity
}
