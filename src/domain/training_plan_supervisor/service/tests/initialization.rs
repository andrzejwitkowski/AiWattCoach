use crate::domain::training_plan_supervisor::{
    NoopTrainingPlanSupervisorScheduler, TrainingPlanSupervisorOperationRepository,
    TrainingPlanSupervisorScheduler,
};

use super::{
    super::TrainingPlanSupervisorService,
    support::{
        FixedClock, InMemorySupervisorOperationRepository, RecordingBatchPort,
        StubUserSettingsService,
    },
};

#[tokio::test]
async fn noop_scheduler_skips_pending_review() {
    let result = NoopTrainingPlanSupervisorScheduler
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap();

    assert_eq!(result, None);
}

#[tokio::test]
async fn supervisor_service_skips_pending_review_when_disabled() {
    let service = TrainingPlanSupervisorService::new(
        InMemorySupervisorOperationRepository::default(),
        StubUserSettingsService::disabled(),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    );

    let result = service
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap();

    assert_eq!(result, None);
}

#[tokio::test]
async fn supervisor_service_creates_pending_review_when_enabled() {
    let repository = InMemorySupervisorOperationRepository::default();
    let batch = RecordingBatchPort::default();
    let service = TrainingPlanSupervisorService::new(
        repository.clone(),
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    )
    .with_batch(batch.clone());

    let result = service
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap()
        .expect("expected pending review");

    assert_eq!(result.worker_operation_key, "worker-op-1");
    assert_eq!(result.user_id, "user-1");
    assert_eq!(result.worker_saved_at_epoch_seconds, 1_700_000_000);
    assert_eq!(result.model, "gemini-2.5-pro");
    assert_eq!(result.status.as_str(), "pending");
    assert_eq!(result.batch_name, Some("batches/supervisor-1".to_string()));
    assert_eq!(result.created_at_epoch_seconds, 1_700_000_200);
    assert_eq!(result.updated_at_epoch_seconds, 1_700_000_200);
    assert_eq!(batch.requests().len(), 1);

    let stored = repository
        .find_by_worker_operation_key("worker-op-1")
        .await
        .unwrap();
    assert_eq!(stored, Some(result));
}

#[tokio::test]
async fn supervisor_service_reuses_existing_operation_for_same_worker_operation() {
    let repository = InMemorySupervisorOperationRepository::default();
    let batch = RecordingBatchPort::default();
    let service = TrainingPlanSupervisorService::new(
        repository.clone(),
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    )
    .with_batch(batch.clone());

    let first = service
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap()
        .expect("expected first pending review");
    let second = service
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap()
        .expect("expected reused pending review");

    assert_eq!(second, first);
    assert_eq!(batch.requests().len(), 1);
}

#[tokio::test]
async fn supervisor_service_skips_pending_review_when_no_settings() {
    let service = TrainingPlanSupervisorService::new(
        InMemorySupervisorOperationRepository::default(),
        StubUserSettingsService::no_settings(),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    );

    let result = service
        .initialize_pending_review("user-1", "worker-op-1", 1_700_000_000, "plan")
        .await
        .unwrap();

    assert_eq!(result, None);
}
