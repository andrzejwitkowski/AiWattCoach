use aiwattcoach::domain::intervals::{
    CreateEvent, DateRange, EventCategory, IntervalsError, IntervalsService, IntervalsUseCases,
    NoopActivityFileIdentityExtractor, NoopActivityRepository,
    NoopActivityUploadOperationRepository, UpdateEvent,
};

use crate::{
    common::{sample_event, valid_credentials},
    fakes::{ApiCall, FakeIntervalsApi, FakeSettingsPort},
};

#[tokio::test]
async fn list_events_returns_events_from_api() {
    let event = sample_event(42, "Workout A");
    let api = FakeIntervalsApi::with_events(vec![event.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.get_event("user-1", 7).await.unwrap();

    assert_eq!(result, event);
}

#[tokio::test]
async fn create_event_passes_event_to_api() {
    let created = sample_event(10, "New Workout");
    let api = FakeIntervalsApi::with_created_event(created.clone());
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let input = CreateEvent {
        category: EventCategory::Workout,
        start_date_local: "2026-04-01".to_string(),
        event_type: Some("Ride".to_string()),
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
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let input = UpdateEvent {
        category: Some(EventCategory::Workout),
        start_date_local: None,
        event_type: Some("Ride".to_string()),
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
            event: input,
        }]
    );
}

#[tokio::test]
async fn delete_event_calls_api_and_returns_ok() {
    let api = FakeIntervalsApi::default();
    let calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.delete_event("user-1", 77).await;

    assert_eq!(result, Ok(()));
    assert_eq!(calls.lock().unwrap().as_slice(), &[ApiCall::Delete(77)]);
}

#[tokio::test]
async fn download_fit_returns_bytes() {
    let api = FakeIntervalsApi::with_fit_bytes(vec![1, 2, 3, 4]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let bytes = service.download_fit("user-1", 33).await.unwrap();

    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn api_error_propagated_to_caller() {
    let api = FakeIntervalsApi::with_error(IntervalsError::ApiError("bad gateway".to_string()));
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.get_event("user-1", 99).await;

    assert_eq!(
        result,
        Err(IntervalsError::ApiError("bad gateway".to_string()))
    );
}

#[tokio::test]
async fn get_enriched_event_propagates_activity_lookup_failure() {
    let event = sample_event(77, "Threshold");
    let api = FakeIntervalsApi::with_get_event_and_activities_error(
        event,
        IntervalsError::ConnectionError("upstream down".to_string()),
    );
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.get_enriched_event("user-1", 77).await;

    assert_eq!(
        result,
        Err(IntervalsError::ConnectionError("upstream down".to_string()))
    );
}
