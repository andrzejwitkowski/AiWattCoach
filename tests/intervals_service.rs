use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    Activity, ActivityDetails, ActivityMetrics, ActivityRepositoryPort, CreateEvent, DateRange,
    Event, EventCategory, IntervalsApiPort, IntervalsCredentials, IntervalsError,
    IntervalsService, IntervalsSettingsPort, IntervalsUseCases, NoopActivityRepository,
    UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[tokio::test]
async fn list_events_returns_events_from_api() {
    let event = sample_event(42, "Workout A");
    let api = FakeIntervalsApi::with_events(vec![event.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let events = service
        .list_events(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(events, vec![event]);
}

#[tokio::test]
async fn list_events_fails_when_credentials_not_configured() {
    let api = FakeIntervalsApi::default();
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::without_credentials();
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let result = service
        .list_events(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await;

    assert_eq!(result, Err(IntervalsError::CredentialsNotConfigured));
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn get_event_returns_single_event() {
    let event = sample_event(7, "Threshold");
    let api = FakeIntervalsApi::with_get_event(event.clone());
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let result = service.get_event("user-1", 7).await.unwrap();

    assert_eq!(result, event);
}

#[tokio::test]
async fn create_event_passes_event_to_api() {
    let created = sample_event(10, "New Workout");
    let api = FakeIntervalsApi::with_created_event(created.clone());
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let input = CreateEvent {
        category: EventCategory::Workout,
        start_date_local: "2026-04-01".to_string(),
        name: Some("New Workout".to_string()),
        description: Some("4x8min".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc: Some("- 4x8min 95%".to_string()),
        file_upload: None,
    };

    let result = service.create_event("user-1", input.clone()).await.unwrap();

    assert_eq!(result, created);
    assert_eq!(calls.lock().unwrap().as_slice(), &[ApiCall::Create(input)]);
}

#[tokio::test]
async fn update_event_forwards_to_api() {
    let updated = sample_event(10, "Updated Workout");
    let api = FakeIntervalsApi::with_updated_event(updated.clone());
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let input = UpdateEvent {
        category: Some(EventCategory::Workout),
        start_date_local: None,
        name: Some("Updated Workout".to_string()),
        description: Some("5x5min".to_string()),
        indoor: Some(false),
        color: Some("red".to_string()),
        workout_doc: Some("- 5x5min 110%".to_string()),
        file_upload: None,
    };

    let result = service
        .update_event("user-1", 10, input.clone())
        .await
        .unwrap();

    assert_eq!(result, updated);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[ApiCall::Update {
            event_id: 10,
            event: input
        }]
    );
}

#[tokio::test]
async fn delete_event_calls_api_and_returns_ok() {
    let api = FakeIntervalsApi::default();
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let result = service.delete_event("user-1", 77).await;

    assert_eq!(result, Ok(()));
    assert_eq!(calls.lock().unwrap().as_slice(), &[ApiCall::Delete(77)]);
}

#[tokio::test]
async fn download_fit_returns_bytes() {
    let api = FakeIntervalsApi::with_fit_bytes(vec![1, 2, 3, 4]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let bytes = service.download_fit("user-1", 33).await.unwrap();

    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn api_error_propagated_to_caller() {
    let api = FakeIntervalsApi::with_error(IntervalsError::ApiError("bad gateway".to_string()));
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings, NoopActivityRepository);

    let result = service.get_event("user-1", 99).await;

    assert_eq!(
        result,
        Err(IntervalsError::ApiError("bad gateway".to_string()))
    );
}

#[tokio::test]
async fn list_activities_persists_api_results_and_returns_repository_view() {
    let activity = sample_activity("i42", "Endurance Ride");
    let api = FakeIntervalsApi::with_activities(vec![activity.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(api, settings, repository);

    let activities = service
        .list_activities(
            "user-1",
            &DateRange {
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(activities, vec![activity]);
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[
            RepoCall::UpsertMany(1),
            RepoCall::FindRange {
                user_id: "user-1".to_string(),
                oldest: "2026-03-01".to_string(),
                newest: "2026-03-31".to_string()
            }
        ]
    );
}

#[tokio::test]
async fn get_activity_persists_fetched_activity() {
    let activity = sample_activity("i77", "Threshold Ride");
    let api = FakeIntervalsApi::with_get_activity(activity.clone());
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(api, settings, repository);

    let fetched = service.get_activity("user-1", "i77").await.unwrap();

    assert_eq!(fetched, activity);
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[RepoCall::Upsert("i77".to_string())]
    );
}

#[tokio::test]
async fn upload_activity_persists_uploaded_activities() {
    let uploaded_activity = sample_activity("i91", "Uploaded Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i91".to_string()],
        activities: vec![uploaded_activity.clone()],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(api, settings, repository);

    let upload = UploadActivity {
        filename: "ride.fit".to_string(),
        file_bytes: vec![1, 2, 3],
        name: Some("Uploaded Ride".to_string()),
        description: Some("fresh from head unit".to_string()),
        device_name: Some("Garmin Edge".to_string()),
        external_id: Some("garmin-1".to_string()),
        paired_event_id: Some(7),
    };

    let result = service.upload_activity("user-1", upload.clone()).await.unwrap();

    assert_eq!(result.activity_ids, vec!["i91".to_string()]);
    assert_eq!(result.activities, vec![uploaded_activity]);
    assert_eq!(
        api_calls.lock().unwrap().as_slice(),
        &[ApiCall::UploadActivity(upload)]
    );
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[RepoCall::UpsertMany(1)]
    );
}

#[tokio::test]
async fn update_activity_persists_updated_activity() {
    let updated_activity = sample_activity("i55", "Updated Ride");
    let api = FakeIntervalsApi::with_updated_activity(updated_activity.clone());
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(api, settings, repository);

    let update = UpdateActivity {
        name: Some("Updated Ride".to_string()),
        description: Some("more details".to_string()),
        activity_type: Some("VirtualRide".to_string()),
        trainer: Some(true),
        commute: Some(false),
        race: Some(false),
    };

    let result = service
        .update_activity("user-1", "i55", update.clone())
        .await
        .unwrap();

    assert_eq!(result, updated_activity);
    assert_eq!(
        api_calls.lock().unwrap().as_slice(),
        &[ApiCall::UpdateActivity {
            activity_id: "i55".to_string(),
            activity: update
        }]
    );
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[RepoCall::Upsert("i55".to_string())]
    );
}

#[tokio::test]
async fn delete_activity_removes_local_copy_before_calling_api() {
    let api = FakeIntervalsApi::default();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let sequence = Arc::new(Mutex::new(Vec::new()));
    let repository = FakeActivityRepository::with_sequence(sequence.clone());
    let api = api.with_sequence(sequence.clone());
    let service = IntervalsService::new(api, settings, repository);

    let result = service.delete_activity("user-1", "i11").await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        sequence.lock().unwrap().as_slice(),
        &["repo_delete:i11".to_string(), "api_delete:i11".to_string()]
    );
}

fn valid_credentials() -> IntervalsCredentials {
    IntervalsCredentials {
        api_key: "api-key-123".to_string(),
        athlete_id: "athlete-42".to_string(),
    }
}

fn sample_event(id: i64, name: &str) -> Event {
    Event {
        id,
        start_date_local: "2026-03-22".to_string(),
        name: Some(name.to_string()),
        category: EventCategory::Workout,
        description: Some("structured workout".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc: Some("- 5min 55%".to_string()),
    }
}

fn sample_activity(id: &str, name: &str) -> Activity {
    Activity {
        id: id.to_string(),
        athlete_id: Some("athlete-42".to_string()),
        start_date_local: "2026-03-22T08:00:00".to_string(),
        start_date: Some("2026-03-22T07:00:00Z".to_string()),
        name: Some(name.to_string()),
        description: Some("structured ride".to_string()),
        activity_type: Some("Ride".to_string()),
        source: Some("UPLOAD".to_string()),
        external_id: Some(format!("external-{id}")),
        device_name: Some("Garmin Edge".to_string()),
        distance_meters: Some(40200.0),
        moving_time_seconds: Some(3600),
        elapsed_time_seconds: Some(3720),
        total_elevation_gain_meters: Some(510.0),
        total_elevation_loss_meters: Some(505.0),
        average_speed_mps: Some(11.1),
        max_speed_mps: Some(16.4),
        average_heart_rate_bpm: Some(148),
        max_heart_rate_bpm: Some(175),
        average_cadence_rpm: Some(89.5),
        trainer: false,
        commute: false,
        race: false,
        has_heart_rate: true,
        stream_types: vec!["watts".to_string(), "heartrate".to_string()],
        tags: vec!["tempo".to_string()],
        metrics: ActivityMetrics {
            training_stress_score: Some(72),
            normalized_power_watts: Some(238),
            intensity_factor: Some(0.84),
            efficiency_factor: Some(1.28),
            variability_index: Some(1.04),
            average_power_watts: Some(228),
            ftp_watts: Some(283),
            total_work_joules: Some(820),
            calories: Some(690),
            trimp: Some(92.0),
            power_load: Some(72),
            heart_rate_load: Some(66),
            pace_load: None,
            strain_score: Some(13.7),
        },
        details: ActivityDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: Vec::new(),
            interval_summary: vec!["tempo".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: Vec::new(),
            heart_rate_zone_times: Vec::new(),
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ApiCall {
    Create(CreateEvent),
    Update { event_id: i64, event: UpdateEvent },
    Delete(i64),
    UploadActivity(UploadActivity),
    UpdateActivity {
        activity_id: String,
        activity: UpdateActivity,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum RepoCall {
    Upsert(String),
    UpsertMany(usize),
    FindRange {
        user_id: String,
        oldest: String,
        newest: String,
    },
}

#[derive(Clone)]
struct FakeIntervalsApi {
    list_events_result: Result<Vec<Event>, IntervalsError>,
    get_event_result: Result<Event, IntervalsError>,
    create_event_result: Result<Event, IntervalsError>,
    update_event_result: Result<Event, IntervalsError>,
    delete_event_result: Result<(), IntervalsError>,
    fit_result: Result<Vec<u8>, IntervalsError>,
    list_activities_result: Result<Vec<Activity>, IntervalsError>,
    get_activity_result: Result<Activity, IntervalsError>,
    upload_activity_result: Result<UploadedActivities, IntervalsError>,
    update_activity_result: Result<Activity, IntervalsError>,
    delete_activity_result: Result<(), IntervalsError>,
    call_log: Arc<Mutex<Vec<ApiCall>>>,
    sequence: Option<Arc<Mutex<Vec<String>>>>,
}

impl Default for FakeIntervalsApi {
    fn default() -> Self {
        Self {
            list_events_result: Ok(Vec::new()),
            get_event_result: Err(IntervalsError::NotFound),
            create_event_result: Err(IntervalsError::NotFound),
            update_event_result: Err(IntervalsError::NotFound),
            delete_event_result: Ok(()),
            fit_result: Ok(Vec::new()),
            list_activities_result: Ok(Vec::new()),
            get_activity_result: Err(IntervalsError::NotFound),
            upload_activity_result: Err(IntervalsError::NotFound),
            update_activity_result: Err(IntervalsError::NotFound),
            delete_activity_result: Ok(()),
            call_log: Arc::new(Mutex::new(Vec::new())),
            sequence: None,
        }
    }
}

impl FakeIntervalsApi {
    fn with_events(events: Vec<Event>) -> Self {
        Self {
            list_events_result: Ok(events),
            ..Self::default()
        }
    }

    fn with_get_event(event: Event) -> Self {
        Self {
            get_event_result: Ok(event),
            ..Self::default()
        }
    }

    fn with_created_event(event: Event) -> Self {
        Self {
            create_event_result: Ok(event),
            ..Self::default()
        }
    }

    fn with_updated_event(event: Event) -> Self {
        Self {
            update_event_result: Ok(event),
            ..Self::default()
        }
    }

    fn with_fit_bytes(bytes: Vec<u8>) -> Self {
        Self {
            fit_result: Ok(bytes),
            ..Self::default()
        }
    }

    fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            list_activities_result: Ok(activities),
            ..Self::default()
        }
    }

    fn with_get_activity(activity: Activity) -> Self {
        Self {
            get_activity_result: Ok(activity),
            ..Self::default()
        }
    }

    fn with_uploaded_activities(result: UploadedActivities) -> Self {
        Self {
            upload_activity_result: Ok(result),
            ..Self::default()
        }
    }

    fn with_updated_activity(activity: Activity) -> Self {
        Self {
            update_activity_result: Ok(activity),
            ..Self::default()
        }
    }

    fn with_sequence(mut self, sequence: Arc<Mutex<Vec<String>>>) -> Self {
        self.sequence = Some(sequence);
        self
    }

    fn with_error(error: IntervalsError) -> Self {
        Self {
            list_events_result: Err(error.clone()),
            get_event_result: Err(error.clone()),
            create_event_result: Err(error.clone()),
            update_event_result: Err(error.clone()),
            delete_event_result: Err(error.clone()),
            fit_result: Err(error),
            list_activities_result: Err(IntervalsError::ApiError("bad gateway".to_string())),
            get_activity_result: Err(IntervalsError::ApiError("bad gateway".to_string())),
            upload_activity_result: Err(IntervalsError::ApiError("bad gateway".to_string())),
            update_activity_result: Err(IntervalsError::ApiError("bad gateway".to_string())),
            delete_activity_result: Err(IntervalsError::ApiError("bad gateway".to_string())),
            ..Self::default()
        }
    }
}

impl IntervalsApiPort for FakeIntervalsApi {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let result = self.list_events_result.clone();
        Box::pin(async move { result })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let result = self.get_event_result.clone();
        Box::pin(async move { result })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        self.call_log.lock().unwrap().push(ApiCall::Create(event));
        let result = self.create_event_result.clone();
        Box::pin(async move { result })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::Update { event_id, event });
        let result = self.update_event_result.clone();
        Box::pin(async move { result })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::Delete(event_id));
        let result = self.delete_event_result.clone();
        Box::pin(async move { result })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let result = self.fit_result.clone();
        Box::pin(async move { result })
    }

    fn list_activities(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let result = self.list_activities_result.clone();
        Box::pin(async move { result })
    }

    fn get_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let result = self.get_activity_result.clone();
        Box::pin(async move { result })
    }

    fn upload_activity(
        &self,
        _credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        self.call_log.lock().unwrap().push(ApiCall::UploadActivity(upload));
        let result = self.upload_activity_result.clone();
        Box::pin(async move { result })
    }

    fn update_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        self.call_log.lock().unwrap().push(ApiCall::UpdateActivity {
            activity_id: activity_id.to_string(),
            activity,
        });
        let result = self.update_activity_result.clone();
        Box::pin(async move { result })
    }

    fn delete_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        if let Some(sequence) = self.sequence.as_ref() {
            sequence
                .lock()
                .unwrap()
                .push(format!("api_delete:{activity_id}"));
        }
        let result = self.delete_activity_result.clone();
        Box::pin(async move { result })
    }
}

#[derive(Clone, Default)]
struct FakeActivityRepository {
    stored: Arc<Mutex<Vec<Activity>>>,
    call_log: Arc<Mutex<Vec<RepoCall>>>,
    sequence: Option<Arc<Mutex<Vec<String>>>>,
}

impl FakeActivityRepository {
    fn with_sequence(sequence: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            sequence: Some(sequence),
            ..Self::default()
        }
    }
}

impl ActivityRepositoryPort for FakeActivityRepository {
    fn upsert(&self, _user_id: &str, activity: Activity) -> BoxFuture<Result<Activity, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(RepoCall::Upsert(activity.id.clone()));
            let mut store = store.lock().unwrap();
            store.retain(|existing| existing.id != activity.id);
            store.push(activity.clone());
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        _user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(RepoCall::UpsertMany(activities.len()));
            let mut store = store.lock().unwrap();
            for activity in &activities {
                store.retain(|existing| existing.id != activity.id);
                store.push(activity.clone());
            }
            Ok(activities)
        })
    }

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(RepoCall::FindRange {
                user_id,
                oldest: oldest.clone(),
                newest: newest.clone(),
            });
            Ok(store
                .lock()
                .unwrap()
                .iter()
                .filter(|activity| activity.start_date_local.as_str() >= oldest.as_str())
                .filter(|activity| activity.start_date_local.as_str() <= newest.as_str())
                .cloned()
                .collect())
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        _user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            Ok(store
                .lock()
                .unwrap()
                .iter()
                .find(|activity| activity.id == activity_id)
                .cloned())
        })
    }

    fn delete(&self, _user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let store = self.stored.clone();
        let sequence = self.sequence.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(sequence) = sequence {
                sequence
                    .lock()
                    .unwrap()
                    .push(format!("repo_delete:{activity_id}"));
            }
            store.lock().unwrap().retain(|activity| activity.id != activity_id);
            Ok(())
        })
    }
}

#[derive(Clone)]
struct FakeSettingsPort {
    credentials: Option<IntervalsCredentials>,
}

impl FakeSettingsPort {
    fn with_credentials(credentials: IntervalsCredentials) -> Self {
        Self {
            credentials: Some(credentials),
        }
    }

    fn without_credentials() -> Self {
        Self { credentials: None }
    }
}

impl IntervalsSettingsPort for FakeSettingsPort {
    fn get_credentials(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
        let credentials = self.credentials.clone();
        Box::pin(async move { credentials.ok_or(IntervalsError::CredentialsNotConfigured) })
    }
}
