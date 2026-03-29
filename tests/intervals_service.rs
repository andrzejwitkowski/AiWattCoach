use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    Activity, ActivityDetails, ActivityFallbackIdentity, ActivityFileIdentityExtractorPort,
    ActivityInterval, ActivityIntervalGroup, ActivityMetrics, ActivityRepositoryPort,
    ActivityStream, CreateEvent, DateRange, Event, EventCategory, IntervalsApiPort,
    IntervalsCredentials, IntervalsError, IntervalsService, IntervalsSettingsPort,
    IntervalsUseCases, NoopActivityFileIdentityExtractor, NoopActivityRepository, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[tokio::test]
async fn list_events_returns_events_from_api() {
    let event = sample_event(42, "Workout A");
    let api = FakeIntervalsApi::with_events(vec![event.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
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
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        NoopActivityRepository::default(),
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
        NoopActivityFileIdentityExtractor,
    );

    let result = service.get_event("user-1", 99).await;

    assert_eq!(
        result,
        Err(IntervalsError::ApiError("bad gateway".to_string()))
    );
}

#[tokio::test]
async fn list_activities_persists_api_results_and_returns_fresh_api_results() {
    let activity = sample_activity("i42", "Endurance Ride");
    let api = FakeIntervalsApi::with_activities(vec![activity.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
        &[RepoCall::UpsertMany(1)]
    );
}

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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
    let service = IntervalsService::new(api, settings, repository, extractor);

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
    let service = IntervalsService::new(api, settings, repository, extractor);

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
    let service = IntervalsService::new(api, settings, repository, extractor);

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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
    let service = IntervalsService::new(api, settings, repository, extractor);

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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
async fn get_activity_persists_enriched_completed_activity() {
    let mut activity = sample_activity("i78", "Completed Workout");
    activity.details.intervals = vec![ActivityInterval {
        id: Some(1),
        label: Some("Threshold".to_string()),
        interval_type: Some("WORK".to_string()),
        group_id: Some("set-1".to_string()),
        start_index: Some(10),
        end_index: Some(20),
        start_time_seconds: Some(300),
        end_time_seconds: Some(600),
        moving_time_seconds: Some(300),
        elapsed_time_seconds: Some(300),
        distance_meters: Some(2500.0),
        average_power_watts: Some(285),
        normalized_power_watts: Some(290),
        training_stress_score: Some(18.5),
        average_heart_rate_bpm: Some(168),
        average_cadence_rpm: Some(94.0),
        average_speed_mps: Some(8.2),
        average_stride_meters: None,
        zone: Some(4),
    }];
    activity.details.interval_groups = vec![ActivityIntervalGroup {
        id: "set-1".to_string(),
        count: Some(3),
        start_index: Some(10),
        moving_time_seconds: Some(900),
        elapsed_time_seconds: Some(900),
        distance_meters: Some(7500.0),
        average_power_watts: Some(280),
        normalized_power_watts: Some(286),
        training_stress_score: Some(55.5),
        average_heart_rate_bpm: Some(165),
        average_cadence_rpm: Some(92.0),
        average_speed_mps: Some(8.0),
        average_stride_meters: None,
    }];
    activity.details.streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: Some("Power".to_string()),
        data: Some(serde_json::json!([120, 250, 310])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];
    let api = FakeIntervalsApi::with_get_activity(activity.clone());
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let stored = repository.stored.clone();
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

    let fetched = service.get_activity("user-1", "i78").await.unwrap();

    let persisted = stored
        .lock()
        .unwrap()
        .get("user-1")
        .and_then(|activities| activities.iter().find(|candidate| candidate.id == "i78"))
        .cloned()
        .expect("persisted activity");

    assert_eq!(fetched, activity);
    assert_eq!(persisted, activity);
    assert_eq!(persisted.details.intervals.len(), 1);
    assert_eq!(persisted.details.interval_groups.len(), 1);
    assert_eq!(persisted.details.streams.len(), 1);
}

#[tokio::test]
async fn get_activity_persists_fetched_activity() {
    let activity = sample_activity("i77", "Threshold Ride");
    let api = FakeIntervalsApi::with_get_activity(activity.clone());
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
            RepoCall::UpsertMany(1)
        ]
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
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

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
async fn delete_activity_removes_local_copy_only_after_upstream_delete_succeeds() {
    let api = FakeIntervalsApi::default();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let sequence = Arc::new(Mutex::new(Vec::new()));
    let repository = FakeActivityRepository::with_sequence(sequence.clone());
    let api = api.with_sequence(sequence.clone());
    let service =
        IntervalsService::new(api, settings, repository, NoopActivityFileIdentityExtractor);

    let result = service.delete_activity("user-1", "i11").await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        sequence.lock().unwrap().as_slice(),
        &["api_delete:i11".to_string(), "repo_delete:i11".to_string()]
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
    Update {
        event_id: i64,
        event: UpdateEvent,
    },
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
    FindExternalId(String),
    FindFallbackIdentity(String),
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
            fit_result: Err(error.clone()),
            list_activities_result: Err(error.clone()),
            get_activity_result: Err(error.clone()),
            upload_activity_result: Err(error.clone()),
            update_activity_result: Err(error.clone()),
            delete_activity_result: Err(error),
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
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::UploadActivity(upload));
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
    stored: Arc<Mutex<HashMap<String, Vec<Activity>>>>,
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

    fn with_existing(user_id: &str, activity: Activity) -> Self {
        let mut stored = HashMap::new();
        stored.insert(user_id.to_string(), vec![activity]);
        Self {
            stored: Arc::new(Mutex::new(stored)),
            ..Self::default()
        }
    }
}

impl ActivityRepositoryPort for FakeActivityRepository {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::Upsert(activity.id.clone()));
            let mut store = store.lock().unwrap();
            let activities = store.entry(user_id).or_default();
            activities.retain(|existing| existing.id != activity.id);
            activities.push(activity.clone());
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::UpsertMany(activities.len()));
            let mut store = store.lock().unwrap();
            let existing = store.entry(user_id).or_default();
            for activity in &activities {
                existing.retain(|current| current.id != activity.id);
                existing.push(activity.clone());
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
            let repo_user_id = user_id.clone();
            calls.lock().unwrap().push(RepoCall::FindRange {
                user_id,
                oldest: oldest.clone(),
                newest: newest.clone(),
            });
            let activities = store
                .lock()
                .unwrap()
                .get(&repo_user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .filter(|activity| activity_date(&activity.start_date_local) >= oldest.as_str())
                .filter(|activity| activity_date(&activity.start_date_local) <= newest.as_str())
                .collect())
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .find(|activity| activity.id == activity_id))
        })
    }

    fn find_by_user_id_and_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::FindExternalId(external_id.clone()));
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities.into_iter().find(|activity| {
                activity
                    .external_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    == Some(external_id.as_str())
            }))
        })
    }

    fn find_by_user_id_and_fallback_identity(
        &self,
        user_id: &str,
        identity: &str,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let store = self.stored.clone();
        let calls = self.call_log.clone();
        let user_id = user_id.to_string();
        let identity = identity.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(RepoCall::FindFallbackIdentity(identity.clone()));
            let activities = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default();
            Ok(activities
                .into_iter()
                .filter(|activity| {
                    ActivityFallbackIdentity::from_activity(activity)
                        .map(|candidate| candidate.as_fingerprint())
                        == Some(identity.clone())
                })
                .collect())
        })
    }

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let store = self.stored.clone();
        let sequence = self.sequence.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(sequence) = sequence {
                sequence
                    .lock()
                    .unwrap()
                    .push(format!("repo_delete:{activity_id}"));
            }
            if let Some(activities) = store.lock().unwrap().get_mut(&user_id) {
                activities.retain(|activity| activity.id != activity_id);
            }
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct FakeActivityIdentityExtractor {
    identity: Option<ActivityFallbackIdentity>,
}

impl FakeActivityIdentityExtractor {
    fn with_identity(identity: ActivityFallbackIdentity) -> Self {
        Self {
            identity: Some(identity),
        }
    }
}

impl ActivityFileIdentityExtractorPort for FakeActivityIdentityExtractor {
    fn extract_identity(
        &self,
        _upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>> {
        let identity = self.identity.clone();
        Box::pin(async move { Ok(identity) })
    }
}

fn activity_date(start_date_local: &str) -> &str {
    start_date_local.get(..10).unwrap_or(start_date_local)
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
