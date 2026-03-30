use aiwattcoach::domain::intervals::{
    ActivityFallbackIdentity, IntervalsService, IntervalsUseCases,
    NoopActivityUploadOperationRepository, UploadActivity, UploadedActivities,
};

use crate::{
    common::{sample_activity, valid_credentials},
    fakes::{
        ApiCall, FakeActivityIdentityExtractor, FakeActivityRepository, FakeIntervalsApi,
        FakeSettingsPort, RepoCall,
    },
};

#[tokio::test]
async fn upload_activity_returns_existing_activity_when_external_id_matches() {
    let existing = sample_activity("i200", "Existing Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i201".to_string()],
        activities: vec![sample_activity("i201", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        FakeActivityIdentityExtractor::default(),
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Existing Ride".to_string()),
                description: None,
                device_name: None,
                external_id: existing.external_id.clone(),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![existing.id.clone()]);
    assert_eq!(result.activities, vec![existing]);
    assert!(api_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upload_activity_returns_existing_activity_when_fallback_identity_matches() {
    let existing = sample_activity("i300", "Existing Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i301".to_string()],
        activities: vec![sample_activity("i301", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let extractor = FakeActivityIdentityExtractor::with_identity(ActivityFallbackIdentity {
        start_bucket: "2026-03-22T07:00".to_string(),
        activity_type_bucket: "ride".to_string(),
        duration_bucket_seconds: 3720,
        distance_bucket_meters: Some(40200),
        trainer: false,
    });
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        extractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Imported Ride".to_string()),
                description: None,
                device_name: None,
                external_id: None,
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![existing.id.clone()]);
    assert_eq!(result.activities, vec![existing]);
    assert!(api_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upload_activity_does_not_dedupe_when_external_ids_conflict_even_if_fallback_matches() {
    let existing = sample_activity("i350", "Existing Ride");
    let uploaded = sample_activity("i351", "Conflicting External Id Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec![uploaded.id.clone()],
        activities: vec![uploaded.clone()],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing);
    let extractor = FakeActivityIdentityExtractor::with_identity(ActivityFallbackIdentity {
        start_bucket: "2026-03-22T07:00".to_string(),
        activity_type_bucket: "ride".to_string(),
        duration_bucket_seconds: 3720,
        distance_bucket_meters: Some(40200),
        trainer: false,
    });
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        extractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Imported Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("different-external-id".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.activities, vec![uploaded]);
    assert_eq!(api_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn upload_activity_does_not_dedupe_ride_and_virtualride() {
    let mut existing = sample_activity("i400", "Trainer Ride");
    existing.activity_type = Some("VirtualRide".to_string());
    let uploaded = sample_activity("i401", "Outdoor Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec![uploaded.id.clone()],
        activities: vec![uploaded.clone()],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing);
    let extractor = FakeActivityIdentityExtractor::with_identity(ActivityFallbackIdentity {
        start_bucket: "2026-03-22T07:00".to_string(),
        activity_type_bucket: "ride".to_string(),
        duration_bucket_seconds: 3720,
        distance_bucket_meters: Some(40200),
        trainer: false,
    });
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        extractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Outdoor Ride".to_string()),
                description: None,
                device_name: None,
                external_id: None,
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.activities, vec![uploaded]);
    assert_eq!(api_calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn upload_activity_returns_existing_activity_when_external_id_matches_after_trim() {
    let existing = sample_activity("i500", "Trimmed Match Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i501".to_string()],
        activities: vec![sample_activity("i501", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        FakeActivityIdentityExtractor::default(),
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Trimmed Match Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("  external-i500  ".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![existing.id.clone()]);
    assert_eq!(result.activities, vec![existing]);
    assert!(api_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upload_activity_normalizes_external_id_before_forwarding_to_api() {
    let uploaded_activity = sample_activity("i601", "Uploaded Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec![uploaded_activity.id.clone()],
        activities: vec![uploaded_activity.clone()],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        FakeActivityIdentityExtractor::default(),
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Uploaded Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("  garmin-601  ".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.activities, vec![uploaded_activity]);
    assert_eq!(
        api_calls.lock().unwrap().as_slice(),
        &[ApiCall::UploadActivity(UploadActivity {
            filename: "ride.fit".to_string(),
            file_bytes: vec![1, 2, 3],
            name: Some("Uploaded Ride".to_string()),
            description: None,
            device_name: None,
            external_id: Some("garmin-601".to_string()),
            paired_event_id: None,
        })]
    );
}

#[tokio::test]
async fn upload_activity_uses_positive_timer_time_when_elapsed_time_is_zero() {
    let mut existing = sample_activity("i610", "Elapsed Zero Ride");
    existing.elapsed_time_seconds = Some(0);
    existing.moving_time_seconds = Some(3600);
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i611".to_string()],
        activities: vec![sample_activity("i611", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let extractor = FakeActivityIdentityExtractor::with_identity(ActivityFallbackIdentity {
        start_bucket: "2026-03-22T07:00".to_string(),
        activity_type_bucket: "ride".to_string(),
        duration_bucket_seconds: 3600,
        distance_bucket_meters: Some(40200),
        trainer: false,
    });
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        extractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Elapsed Zero Ride".to_string()),
                description: None,
                device_name: None,
                external_id: None,
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![existing.id.clone()]);
    assert_eq!(result.activities, vec![existing]);
    assert!(api_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upload_activity_returns_cached_duplicate_without_credentials() {
    let existing = sample_activity("i620", "Cached Duplicate Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i621".to_string()],
        activities: vec![sample_activity("i621", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::without_credentials();
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        FakeActivityIdentityExtractor::default(),
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Cached Duplicate Ride".to_string()),
                description: None,
                device_name: None,
                external_id: existing.external_id.clone(),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![existing.id.clone()]);
    assert_eq!(result.activities, vec![existing]);
    assert!(api_calls.lock().unwrap().is_empty());
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
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        FakeActivityIdentityExtractor::default(),
    );

    let upload = UploadActivity {
        filename: "ride.fit".to_string(),
        file_bytes: vec![1, 2, 3],
        name: Some("Uploaded Ride".to_string()),
        description: Some("fresh from head unit".to_string()),
        device_name: Some("Garmin Edge".to_string()),
        external_id: Some("garmin-1".to_string()),
        paired_event_id: Some(7),
    };

    let result = service
        .upload_activity("user-1", upload.clone())
        .await
        .unwrap();

    assert_eq!(result.activity_ids, vec!["i91".to_string()]);
    assert_eq!(result.activities, vec![uploaded_activity]);
    assert_eq!(
        api_calls.lock().unwrap().as_slice(),
        &[ApiCall::UploadActivity(upload)]
    );
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[
            RepoCall::FindExternalId("garmin-1".to_string()),
            RepoCall::UpsertMany(1),
        ]
    );
}
