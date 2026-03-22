use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    CreateEvent, DateRange, Event, EventCategory, IntervalsApiPort, IntervalsCredentials,
    IntervalsError, IntervalsService, IntervalsSettingsPort, IntervalsUseCases, UpdateEvent,
};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[tokio::test]
async fn list_events_returns_events_from_api() {
    let event = sample_event(42, "Workout A");
    let api = FakeIntervalsApi::with_events(vec![event.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings);

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
    let service = IntervalsService::new(api, settings);

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
    let service = IntervalsService::new(api, settings);

    let result = service.get_event("user-1", 7).await.unwrap();

    assert_eq!(result, event);
}

#[tokio::test]
async fn create_event_passes_event_to_api() {
    let created = sample_event(10, "New Workout");
    let api = FakeIntervalsApi::with_created_event(created.clone());
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings);

    let input = CreateEvent {
        category: EventCategory::Workout,
        start_date_local: "2026-04-01".to_string(),
        name: Some("New Workout".to_string()),
        description: Some("4x8min".to_string()),
        indoor: true,
        color: Some("blue".to_string()),
        workout_doc: Some("- 4x8min 95%".to_string()),
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
    let service = IntervalsService::new(api, settings);

    let input = UpdateEvent {
        category: Some(EventCategory::Workout),
        start_date_local: None,
        name: Some("Updated Workout".to_string()),
        description: Some("5x5min".to_string()),
        indoor: Some(false),
        color: Some("red".to_string()),
        workout_doc: Some("- 5x5min 110%".to_string()),
    };

    let result = service
        .update_event("user-1", 10, input.clone())
        .await
        .unwrap();

    assert_eq!(result, updated);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[ApiCall::Update { event_id: 10, event: input }]
    );
}

#[tokio::test]
async fn delete_event_calls_api_and_returns_ok() {
    let api = FakeIntervalsApi::default();
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings);

    let result = service.delete_event("user-1", 77).await;

    assert_eq!(result, Ok(()));
    assert_eq!(calls.lock().unwrap().as_slice(), &[ApiCall::Delete(77)]);
}

#[tokio::test]
async fn download_fit_returns_bytes() {
    let api = FakeIntervalsApi::with_fit_bytes(vec![1, 2, 3, 4]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings);

    let bytes = service.download_fit("user-1", 33).await.unwrap();

    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn api_error_propagated_to_caller() {
    let api = FakeIntervalsApi::with_error(IntervalsError::ApiError("bad gateway".to_string()));
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(api, settings);

    let result = service.get_event("user-1", 99).await;

    assert_eq!(result, Err(IntervalsError::ApiError("bad gateway".to_string())));
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

#[derive(Clone, Debug, PartialEq)]
enum ApiCall {
    Create(CreateEvent),
    Update { event_id: i64, event: UpdateEvent },
    Delete(i64),
}

#[derive(Clone)]
struct FakeIntervalsApi {
    list_events_result: Result<Vec<Event>, IntervalsError>,
    get_event_result: Result<Event, IntervalsError>,
    create_event_result: Result<Event, IntervalsError>,
    update_event_result: Result<Event, IntervalsError>,
    delete_event_result: Result<(), IntervalsError>,
    fit_result: Result<Vec<u8>, IntervalsError>,
    call_log: Arc<Mutex<Vec<ApiCall>>>,
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
            call_log: Arc::new(Mutex::new(Vec::new())),
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

    fn with_error(error: IntervalsError) -> Self {
        Self {
            list_events_result: Err(error.clone()),
            get_event_result: Err(error.clone()),
            create_event_result: Err(error.clone()),
            update_event_result: Err(error.clone()),
            delete_event_result: Err(error.clone()),
            fit_result: Err(error),
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
        self.call_log.lock().unwrap().push(ApiCall::Delete(event_id));
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
        Box::pin(async move {
            credentials.ok_or(IntervalsError::CredentialsNotConfigured)
        })
    }
}
