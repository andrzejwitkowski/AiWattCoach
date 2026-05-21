use crate::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        TrainingPlanSupervisorDecision, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorStatus,
    },
};

use super::{
    super::TrainingPlanSupervisorService,
    support::{
        accepted_review, seed_active_pending_day, seed_pending_operation,
        seed_superseded_pending_day, FailingOnceProjectionRepository, FixedClock,
        InMemorySupervisorOperationRepository, RecordingProjectionRepository,
        StubUserSettingsService,
    },
};

#[tokio::test]
async fn supervisor_service_completes_review_and_updates_active_projected_days() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let projections = RecordingProjectionRepository::default();
    seed_active_pending_day(&projections, "2026-05-18");
    seed_superseded_pending_day(&projections, "2026-05-17", 1_700_000_050);
    let service = TrainingPlanSupervisorService::new(
        repository.clone(),
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    );

    let completed = service
        .complete_review(projections.clone(), "worker-op-1", accepted_review())
        .await
        .unwrap();

    assert_eq!(completed.status, TrainingPlanSupervisorStatus::Accepted);
    assert_eq!(completed.review, Some(accepted_review()));
    assert_eq!(completed.updated_at_epoch_seconds, 1_700_000_200);

    let stored = repository
        .find_by_worker_operation_key("worker-op-1")
        .await
        .unwrap()
        .expect("expected stored operation");
    assert_eq!(stored, completed);

    let days = projections.stored_days();
    let active = days
        .iter()
        .find(|day| day.superseded_at_epoch_seconds.is_none())
        .expect("expected active day");
    assert_eq!(
        active.supervisor_status,
        Some(TrainingPlanSupervisorStatus::Accepted)
    );
    assert_eq!(active.updated_at_epoch_seconds, 1_700_000_200);

    let superseded = days
        .iter()
        .find(|day| day.superseded_at_epoch_seconds.is_some())
        .expect("expected superseded day");
    assert_eq!(
        superseded.supervisor_status,
        Some(TrainingPlanSupervisorStatus::Pending)
    );
    assert_eq!(superseded.updated_at_epoch_seconds, 1);
}

#[tokio::test]
async fn supervisor_service_retries_same_review_after_projection_failure() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let base_projections = RecordingProjectionRepository::default();
    seed_active_pending_day(&base_projections, "2026-05-18");
    let projections = FailingOnceProjectionRepository::new(base_projections);
    let service = TrainingPlanSupervisorService::new(
        repository.clone(),
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    );

    let first_error = service
        .complete_review(projections.clone(), "worker-op-1", accepted_review())
        .await
        .expect_err("expected first projection update to fail");
    assert_eq!(
        first_error,
        TrainingPlanError::Repository("projection update failed once".to_string())
    );

    let completed = service
        .complete_review(projections.clone(), "worker-op-1", accepted_review())
        .await
        .expect("expected retry to repair projection state");

    assert_eq!(completed.status, TrainingPlanSupervisorStatus::Accepted);
    let stored = repository
        .find_by_worker_operation_key("worker-op-1")
        .await
        .unwrap()
        .expect("expected stored operation");
    assert_eq!(stored, completed);

    let active = projections
        .stored_days()
        .into_iter()
        .find(|day| day.superseded_at_epoch_seconds.is_none())
        .expect("expected active day");
    assert_eq!(
        active.supervisor_status,
        Some(TrainingPlanSupervisorStatus::Accepted)
    );
}

#[tokio::test]
async fn supervisor_service_rejects_conflicting_second_terminal_review() {
    let repository = InMemorySupervisorOperationRepository::default();
    repository
        .upsert(
            TrainingPlanSupervisorOperation::pending(
                "worker-op-1".to_string(),
                "user-1".to_string(),
                1_700_000_000,
                "gemini-2.5-pro".to_string(),
                1_700_000_100,
            )
            .complete_review(
                TrainingPlanSupervisorReview {
                    decision: TrainingPlanSupervisorDecision::Accept,
                    reason: "looks good".to_string(),
                    plan: None,
                },
                1_700_000_150,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let service = TrainingPlanSupervisorService::new(
        repository,
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    );

    let error = service
        .complete_review(
            RecordingProjectionRepository::default(),
            "worker-op-1",
            TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Fail,
                reason: "actually invalid".to_string(),
                plan: None,
            },
        )
        .await
        .expect_err("expected conflicting terminal review to fail");

    assert_eq!(
        error,
        TrainingPlanError::Validation(
            "training plan supervisor review already completed with status accepted".to_string()
        )
    );
}
