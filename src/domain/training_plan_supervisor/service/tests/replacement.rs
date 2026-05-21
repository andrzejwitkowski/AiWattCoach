use crate::domain::{
    external_sync::{ExternalProvider, ExternalSyncState, ExternalSyncStateRepository},
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        TrainingPlanSupervisorDecision, TrainingPlanSupervisorReview, TrainingPlanSupervisorStatus,
    },
};

use super::{
    super::TrainingPlanSupervisorService,
    support::{
        planned_workout_entity, replacement_plan, replacement_plan_from, seed_pending_operation,
        seed_projected_days, seed_projected_days_from, shifted_replacement_plan, FixedClock,
        FixedSyncStateRepository, InMemorySupervisorOperationRepository,
        RecordingProjectionRepository, StubUserSettingsService,
    },
};

#[tokio::test]
async fn supervisor_service_applies_replacement_only_to_days_without_sync_state() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let projections = RecordingProjectionRepository::default();
    seed_projected_days(&projections);
    let sync_states = FixedSyncStateRepository::default();
    sync_states.seed_state(ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        planned_workout_entity("worker-op-1", "2026-05-18"),
    ));
    let service = TrainingPlanSupervisorService::new(
        repository.clone(),
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_700_000_200,
        },
    )
    .with_sync_states(sync_states);

    let completed = service
        .complete_review(
            projections.clone(),
            "worker-op-1",
            TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Replace,
                reason: "needs safer progression".to_string(),
                plan: Some(replacement_plan()),
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.status, TrainingPlanSupervisorStatus::Replaced);
    let apply_result = completed
        .replacement_apply_result
        .expect("expected replacement apply result");
    assert!(apply_result
        .applied_dates
        .contains(&"2026-05-19".to_string()));
    assert_eq!(apply_result.skipped_dates, vec!["2026-05-18"]);
    assert_eq!(apply_result.skipped_synced_dates, vec!["2026-05-18"]);
    let days = projections.stored_days();
    let skipped = days
        .iter()
        .find(|day| day.date == "2026-05-18")
        .expect("expected skipped day");
    assert_eq!(
        skipped.workout.as_ref().unwrap().lines[0].text(),
        Some("original day 1")
    );
    let applied = days
        .iter()
        .find(|day| day.date == "2026-05-19")
        .expect("expected applied day");
    assert_eq!(
        applied.workout.as_ref().unwrap().lines[0].text(),
        Some("replacement day 2")
    );
}

#[tokio::test]
async fn supervisor_service_does_not_apply_replacement_to_today_or_past_days() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let projections = RecordingProjectionRepository::default();
    seed_projected_days_from(&projections, 17);
    let service = TrainingPlanSupervisorService::new(
        repository,
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_779_062_400,
        },
    );

    let completed = service
        .complete_review(
            projections.clone(),
            "worker-op-1",
            TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Replace,
                reason: "needs safer progression".to_string(),
                plan: Some(replacement_plan_from(17)),
            },
        )
        .await
        .unwrap();

    let apply_result = completed
        .replacement_apply_result
        .expect("expected replacement apply result");
    assert!(!apply_result
        .applied_dates
        .contains(&"2026-05-17".to_string()));
    assert!(!apply_result
        .applied_dates
        .contains(&"2026-05-18".to_string()));
    assert!(apply_result
        .applied_dates
        .contains(&"2026-05-19".to_string()));
    assert_eq!(apply_result.skipped_dates, vec!["2026-05-17", "2026-05-18"]);
    let days = projections.stored_days();
    let today = days
        .iter()
        .find(|day| day.date == "2026-05-18")
        .expect("expected today day");
    assert_eq!(
        today.workout.as_ref().unwrap().lines[0].text(),
        Some("original day 2")
    );
    let future = days
        .iter()
        .find(|day| day.date == "2026-05-19")
        .expect("expected future day");
    assert_eq!(
        future.workout.as_ref().unwrap().lines[0].text(),
        Some("replacement day 3")
    );
}

#[tokio::test]
async fn supervisor_service_rejects_replacement_with_shifted_dates() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let projections = RecordingProjectionRepository::default();
    seed_projected_days(&projections);
    let service = TrainingPlanSupervisorService::new(
        repository,
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_779_062_400,
        },
    );

    let error = service
        .complete_review(
            projections,
            "worker-op-1",
            TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Replace,
                reason: "shifted dates".to_string(),
                plan: Some(shifted_replacement_plan()),
            },
        )
        .await
        .expect_err("expected shifted replacement dates to fail validation");

    assert_eq!(
        error,
        TrainingPlanError::Validation(
            "training plan supervisor replacement dates must match active projection window"
                .to_string()
        )
    );
}

#[tokio::test]
async fn supervisor_service_protects_first_active_day_when_clock_date_lags_window() {
    let repository = InMemorySupervisorOperationRepository::default();
    seed_pending_operation(&repository).await;
    let projections = RecordingProjectionRepository::default();
    seed_projected_days(&projections);
    let service = TrainingPlanSupervisorService::new(
        repository,
        StubUserSettingsService::enabled("gemini-2.5-pro"),
        FixedClock {
            now_epoch_seconds: 1_778_976_000,
        },
    );

    let completed = service
        .complete_review(
            projections.clone(),
            "worker-op-1",
            TrainingPlanSupervisorReview {
                decision: TrainingPlanSupervisorDecision::Replace,
                reason: "needs safer progression".to_string(),
                plan: Some(replacement_plan()),
            },
        )
        .await
        .unwrap();

    let apply_result = completed
        .replacement_apply_result
        .expect("expected replacement apply result");
    assert!(!apply_result
        .applied_dates
        .contains(&"2026-05-18".to_string()));
    assert!(apply_result
        .applied_dates
        .contains(&"2026-05-19".to_string()));
    let first_day = projections
        .stored_days()
        .into_iter()
        .find(|day| day.date == "2026-05-18")
        .expect("expected first active day");
    assert_eq!(
        first_day.workout.as_ref().unwrap().lines[0].text(),
        Some("original day 1")
    );
}

#[tokio::test]
async fn fixed_sync_state_repository_upsert_persists_state_for_follow_up_reads() {
    let repository = FixedSyncStateRepository::default();
    let state = ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        planned_workout_entity("worker-op-1", "2026-05-18"),
    );

    repository.upsert(state.clone()).await.unwrap();

    let stored = repository
        .find_by_canonical_entities("user-1", std::slice::from_ref(&state.canonical_entity))
        .await
        .unwrap();
    assert_eq!(stored, vec![state]);
}
