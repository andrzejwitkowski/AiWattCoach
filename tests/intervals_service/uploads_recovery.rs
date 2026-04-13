use aiwattcoach::domain::intervals::{
    ActivityUploadOperation, ActivityUploadOperationStatus, IntervalsError, IntervalsService,
    IntervalsUseCases, NoopActivityFileIdentityExtractor, UploadActivity, UploadedActivities,
};

use crate::{
    common::{sample_activity, valid_credentials},
    fakes::{
        FakeActivityRepository, FakeActivityUploadOperationRepository, FakeIntervalsApi,
        FakeSettingsPort, UploadOperationRepoCall,
    },
    refresh_support::RecordingCalendarRefresh,
};

#[tokio::test]
async fn upload_activity_records_pending_state_before_upstream_upload() {
    let uploaded_activity = sample_activity("i92", "Uploaded Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec![uploaded_activity.id.clone()],
        activities: vec![uploaded_activity.clone()],
    });
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let upload_operations = FakeActivityUploadOperationRepository::default();
    let operation_calls = upload_operations.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
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
                external_id: Some("garmin-92".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.activities, vec![uploaded_activity]);
    assert_eq!(
        operation_calls.lock().unwrap().as_slice(),
        &[
            UploadOperationRepoCall::ClaimPending("external_id:garmin-92".to_string()),
            UploadOperationRepoCall::Upsert(
                "external_id:garmin-92".to_string(),
                ActivityUploadOperationStatus::Uploaded,
            ),
            UploadOperationRepoCall::Upsert(
                "external_id:garmin-92".to_string(),
                ActivityUploadOperationStatus::Completed,
            ),
        ]
    );
}

#[tokio::test]
async fn upload_activity_marks_operation_failed_when_upstream_upload_fails() {
    let api = FakeIntervalsApi::with_error(IntervalsError::ConnectionError(
        "intervals unavailable".to_string(),
    ));
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let upload_operations = FakeActivityUploadOperationRepository::default();
    let operation_calls = upload_operations.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
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
                external_id: Some("garmin-fail".to_string()),
                paired_event_id: None,
            },
        )
        .await;

    assert_eq!(
        result,
        Err(IntervalsError::ConnectionError(
            "intervals unavailable".to_string()
        ))
    );
    assert_eq!(
        operation_calls.lock().unwrap().as_slice(),
        &[
            UploadOperationRepoCall::ClaimPending("external_id:garmin-fail".to_string()),
            UploadOperationRepoCall::Upsert(
                "external_id:garmin-fail".to_string(),
                ActivityUploadOperationStatus::Failed,
            ),
        ]
    );
}

#[tokio::test]
async fn upload_activity_retries_when_existing_operation_is_failed() {
    let uploaded_activity = sample_activity("i95", "Retried Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec![uploaded_activity.id.clone()],
        activities: vec![uploaded_activity.clone()],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let upload_operations = FakeActivityUploadOperationRepository::with_existing(
        "user-1",
        ActivityUploadOperation {
            operation_key: "external_id:external-i95".to_string(),
            normalized_external_id: Some("external-i95".to_string()),
            fallback_identity: None,
            uploaded_activity_ids: Vec::new(),
            status: ActivityUploadOperationStatus::Failed,
        },
    );
    let operation_calls = upload_operations.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Retried Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("external-i95".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(result.created);
    assert_eq!(result.activities, vec![uploaded_activity]);
    assert_eq!(api_calls.lock().unwrap().len(), 1);
    assert_eq!(
        operation_calls.lock().unwrap().as_slice(),
        &[
            UploadOperationRepoCall::ClaimPending("external_id:external-i95".to_string()),
            UploadOperationRepoCall::Upsert(
                "external_id:external-i95".to_string(),
                ActivityUploadOperationStatus::Uploaded,
            ),
            UploadOperationRepoCall::Upsert(
                "external_id:external-i95".to_string(),
                ActivityUploadOperationStatus::Completed,
            ),
        ]
    );
}

#[tokio::test]
async fn upload_activity_blocks_when_existing_operation_is_pending() {
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["i96".to_string()],
        activities: vec![sample_activity("i96", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let upload_operations = FakeActivityUploadOperationRepository::with_existing(
        "user-1",
        ActivityUploadOperation {
            operation_key: "external_id:external-i96".to_string(),
            normalized_external_id: Some("external-i96".to_string()),
            fallback_identity: None,
            uploaded_activity_ids: Vec::new(),
            status: ActivityUploadOperationStatus::Pending,
        },
    );
    let operation_calls = upload_operations.call_log.clone();
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Blocked Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("external-i96".to_string()),
                paired_event_id: None,
            },
        )
        .await;

    assert_eq!(
        result,
        Err(IntervalsError::Internal(
            "Activity upload is already pending recovery".to_string()
        ))
    );
    assert!(api_calls.lock().unwrap().is_empty());
    assert_eq!(
        operation_calls.lock().unwrap().as_slice(),
        &[UploadOperationRepoCall::ClaimPending(
            "external_id:external-i96".to_string()
        )]
    );
}

#[tokio::test]
async fn upload_activity_recovers_completed_operation_without_second_upload() {
    let existing = sample_activity("i93", "Recovered Ride");
    let api = FakeIntervalsApi::with_uploaded_activities(UploadedActivities {
        created: true,
        activity_ids: vec!["should-not-upload".to_string()],
        activities: vec![sample_activity("should-not-upload", "Should Not Upload")],
    });
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::with_existing("user-1", existing.clone());
    let upload_operations = FakeActivityUploadOperationRepository::with_existing(
        "user-1",
        ActivityUploadOperation {
            operation_key: "external_id:external-i93".to_string(),
            normalized_external_id: Some("external-i93".to_string()),
            fallback_identity: None,
            uploaded_activity_ids: vec![existing.id.clone()],
            status: ActivityUploadOperationStatus::Completed,
        },
    );
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Recovered Ride".to_string()),
                description: None,
                device_name: None,
                external_id: Some("external-i93".to_string()),
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
async fn upload_activity_recovers_uploaded_operation_after_previous_persistence_failure() {
    let recovered = sample_activity("i94", "Recovered Upload");
    let api = FakeIntervalsApi::with_get_activity(recovered.clone());
    let api_calls = api.call_log.clone();
    let settings = FakeSettingsPort::with_credentials(valid_credentials());
    let repository = FakeActivityRepository::default();
    let upload_operations = FakeActivityUploadOperationRepository::with_existing(
        "user-1",
        ActivityUploadOperation {
            operation_key: "external_id:external-i94".to_string(),
            normalized_external_id: Some("external-i94".to_string()),
            fallback_identity: None,
            uploaded_activity_ids: vec![recovered.id.clone()],
            status: ActivityUploadOperationStatus::Uploaded,
        },
    );
    let service = IntervalsService::new(
        api,
        settings,
        repository,
        upload_operations,
        NoopActivityFileIdentityExtractor,
    );

    let result = service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Recovered Upload".to_string()),
                description: None,
                device_name: None,
                external_id: Some("external-i94".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert!(!result.created);
    assert_eq!(result.activity_ids, vec![recovered.id.clone()]);
    assert_eq!(result.activities, vec![recovered]);
    assert!(api_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn upload_activity_recovery_refreshes_calendar_view_when_it_restores_missing_activity() {
    let recovered = sample_activity("i94", "Recovered Upload");
    let refresh = RecordingCalendarRefresh::default();
    let service = IntervalsService::new(
        FakeIntervalsApi::with_get_activity(recovered),
        FakeSettingsPort::with_credentials(valid_credentials()),
        FakeActivityRepository::default(),
        FakeActivityUploadOperationRepository::with_existing(
            "user-1",
            ActivityUploadOperation {
                operation_key: "external_id:external-i94".to_string(),
                normalized_external_id: Some("external-i94".to_string()),
                fallback_identity: None,
                uploaded_activity_ids: vec!["i94".to_string()],
                status: ActivityUploadOperationStatus::Uploaded,
            },
        ),
        NoopActivityFileIdentityExtractor,
    )
    .with_calendar_view_refresh(refresh.clone());

    service
        .upload_activity(
            "user-1",
            UploadActivity {
                filename: "ride.fit".to_string(),
                file_bytes: vec![1, 2, 3],
                name: Some("Recovered Upload".to_string()),
                description: None,
                device_name: None,
                external_id: Some("external-i94".to_string()),
                paired_event_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        refresh.calls(),
        vec![(
            "user-1".to_string(),
            "2026-03-22".to_string(),
            "2026-03-22".to_string(),
        )]
    );
}
