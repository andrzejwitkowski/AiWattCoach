use std::sync::{Arc, Mutex};

use aiwattcoach::domain::intervals::{
    ActivityDetails, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
    ActivityRepositoryPort, ActivityStream, DateRange, IntervalsError, IntervalsService,
    IntervalsUseCases, NoopActivityFileIdentityExtractor, NoopActivityUploadOperationRepository,
    UpdateActivity,
};

use crate::{
    common::{sample_activity, valid_credentials},
    fakes::{ApiCall, FakeActivityRepository, FakeIntervalsApi, FakeSettingsPort, RepoCall},
};

#[tokio::test]
async fn list_activities_persists_api_results_and_returns_fresh_api_results() {
    let activity = sample_activity("i42", "Endurance Ride");
    let api = FakeIntervalsApi::with_activities(vec![activity.clone()]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let fetched = service.get_activity("user-1", "i77").await.unwrap();

    assert_eq!(fetched, activity);
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[RepoCall::Upsert("i77".to_string())]
    );
}

#[tokio::test]
async fn list_activities_does_not_clobber_existing_enriched_completed_activity() {
    let mut enriched = sample_activity("i79", "Completed Workout");
    enriched.details.intervals = vec![ActivityInterval {
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
    enriched.details.interval_groups = vec![ActivityIntervalGroup {
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
    enriched.details.streams = vec![ActivityStream {
        stream_type: "watts".to_string(),
        name: Some("Power".to_string()),
        data: Some(serde_json::json!([120, 250, 310])),
        data2: None,
        value_type_is_array: false,
        custom: false,
        all_null: false,
    }];

    let mut sparse = sample_activity("i79", "Completed Workout");
    sparse.athlete_id = None;
    sparse.start_date = None;
    sparse.name = None;
    sparse.description = None;
    sparse.activity_type = None;
    sparse.external_id = None;
    sparse.device_name = None;
    sparse.distance_meters = None;
    sparse.moving_time_seconds = None;
    sparse.elapsed_time_seconds = None;
    sparse.total_elevation_gain_meters = None;
    sparse.total_elevation_loss_meters = None;
    sparse.average_speed_mps = None;
    sparse.max_speed_mps = None;
    sparse.average_heart_rate_bpm = None;
    sparse.max_heart_rate_bpm = None;
    sparse.average_cadence_rpm = None;
    sparse.has_heart_rate = false;
    sparse.stream_types = Vec::new();
    sparse.tags = Vec::new();
    sparse.metrics = ActivityMetrics {
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
    };
    sparse.details = ActivityDetails {
        intervals: Vec::new(),
        interval_groups: Vec::new(),
        streams: Vec::new(),
        interval_summary: Vec::new(),
        skyline_chart: Vec::new(),
        power_zone_times: Vec::new(),
        heart_rate_zone_times: Vec::new(),
        pace_zone_times: Vec::new(),
        gap_zone_times: Vec::new(),
    };

    let api = FakeIntervalsApi::with_activities(vec![sparse]);
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", enriched.clone());
    let stored = repository.stored.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let listed = service
        .list_activities(
            "user-1",
            &DateRange {
                oldest: "2026-03-22".to_string(),
                newest: "2026-03-22".to_string(),
            },
        )
        .await
        .unwrap();

    let persisted = stored
        .lock()
        .unwrap()
        .get("user-1")
        .and_then(|activities| activities.iter().find(|candidate| candidate.id == "i79"))
        .cloned()
        .expect("persisted activity");

    assert_eq!(listed.len(), 1);
    assert_eq!(persisted.name, enriched.name);
    assert_eq!(persisted.activity_type, enriched.activity_type);
    assert_eq!(persisted.metrics, enriched.metrics);
    assert_eq!(persisted.details, enriched.details);
    assert_eq!(persisted.stream_types, enriched.stream_types);
    assert_eq!(persisted.tags, enriched.tags);
}

#[tokio::test]
async fn fake_activity_repository_logs_latest_lookup() {
    let repository = FakeActivityRepository::default();
    let mut older = sample_activity("i89", "Older Ride");
    older.start_date_local = "2026-03-21T08:00:00".to_string();
    let latest = sample_activity("i90", "Latest Ride");
    repository
        .stored
        .lock()
        .unwrap()
        .insert("user-1".to_string(), vec![older, latest.clone()]);

    let activity = repository.find_latest_by_user_id("user-1").await.unwrap();

    assert_eq!(activity, Some(latest));
    assert_eq!(
        repository.call_log.lock().unwrap().as_slice(),
        &[RepoCall::FindLatest("user-1".to_string())]
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
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

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
            activity: update,
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
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.delete_activity("user-1", "i11").await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        sequence.lock().unwrap().as_slice(),
        &["api_delete:i11".to_string(), "repo_delete:i11".to_string()]
    );
}

#[tokio::test]
async fn update_activity_returns_upstream_result_when_local_persistence_fails() {
    let updated_activity = sample_activity("i56", "Updated Ride");
    let api = FakeIntervalsApi::with_updated_activity(updated_activity.clone());
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_upsert_error(IntervalsError::Internal(
        "mongo unavailable".to_string(),
    ));
    let repository_calls = repository.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let update = UpdateActivity {
        name: Some("Updated Ride".to_string()),
        description: Some("more details".to_string()),
        activity_type: Some("VirtualRide".to_string()),
        trainer: Some(true),
        commute: Some(false),
        race: Some(false),
    };

    let result = service
        .update_activity("user-1", "i56", update.clone())
        .await
        .unwrap();

    assert_eq!(result, updated_activity);
    assert_eq!(
        api_calls.lock().unwrap().as_slice(),
        &[ApiCall::UpdateActivity {
            activity_id: "i56".to_string(),
            activity: update,
        }]
    );
    assert_eq!(
        repository_calls.lock().unwrap().as_slice(),
        &[RepoCall::Upsert("i56".to_string())]
    );
}

#[tokio::test]
async fn delete_activity_returns_ok_when_local_delete_fails_after_upstream_success() {
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let sequence = Arc::new(Mutex::new(Vec::new()));
    let repository = FakeActivityRepository::with_sequence_and_delete_error(
        sequence.clone(),
        IntervalsError::Internal("mongo unavailable".to_string()),
    );
    let api = FakeIntervalsApi::default().with_sequence(sequence.clone());
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        NoopActivityUploadOperationRepository::default(),
        NoopActivityFileIdentityExtractor,
    );

    let result = service.delete_activity("user-1", "i12").await;

    assert_eq!(result, Ok(()));
    assert_eq!(
        sequence.lock().unwrap().as_slice(),
        &["api_delete:i12".to_string(), "repo_delete:i12".to_string()]
    );
}
