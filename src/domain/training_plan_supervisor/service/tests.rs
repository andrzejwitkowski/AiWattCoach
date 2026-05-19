use crate::domain::{
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository,
    },
    training_plan::{TrainingPlanError, TrainingPlanProjectedDay},
    training_plan_supervisor::{
        NoopTrainingPlanSupervisorScheduler, TrainingPlanSupervisorDecision,
        TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
        TrainingPlanSupervisorReview, TrainingPlanSupervisorScheduler,
        TrainingPlanSupervisorStatus,
    },
};

use super::{
    tests_support::{
        accepted_review, FailingOnceProjectionRepository, FixedClock, FixedSyncStateRepository,
        InMemorySupervisorOperationRepository, RecordingBatchPort, RecordingProjectionRepository,
        StubUserSettingsService,
    },
    TrainingPlanSupervisorService,
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
async fn supervisor_service_applies_replacement_only_to_days_without_sync_state() {
    let repository = InMemorySupervisorOperationRepository::default();
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
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
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
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
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
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
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
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

#[tokio::test]
async fn supervisor_service_completes_review_and_updates_active_projected_days() {
    let repository = InMemorySupervisorOperationRepository::default();
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
    let projections = RecordingProjectionRepository::default();
    projections.seed_day(TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: "worker-op-1".to_string(),
        date: "2026-05-18".to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: None,
        supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    });
    projections.seed_day(TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: "worker-op-1".to_string(),
        date: "2026-05-17".to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: None,
        supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
        superseded_at_epoch_seconds: Some(1_700_000_050),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    });
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
    repository
        .upsert(TrainingPlanSupervisorOperation::pending(
            "worker-op-1".to_string(),
            "user-1".to_string(),
            1_700_000_000,
            "gemini-2.5-pro".to_string(),
            1_700_000_100,
        ))
        .await
        .unwrap();
    let base_projections = RecordingProjectionRepository::default();
    base_projections.seed_day(TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: "worker-op-1".to_string(),
        date: "2026-05-18".to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: None,
        supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    });
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

fn projected_day(date: &str, name: &str) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: "worker-op-1".to_string(),
        date: date.to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: Some(
            crate::domain::intervals::parse_planned_workout(&format!("{name}\n- 60m 65%")).unwrap(),
        ),
        supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

fn seed_projected_days(projections: &RecordingProjectionRepository) {
    seed_projected_days_from(projections, 18);
}

fn seed_projected_days_from(projections: &RecordingProjectionRepository, start_day: usize) {
    for index in 0..14 {
        projections.seed_day(projected_day(
            &format!("2026-05-{}", start_day + index),
            &format!("original day {}", index + 1),
        ));
    }
}

fn replacement_plan() -> String {
    replacement_plan_from(18)
}

fn replacement_plan_from(start_day: usize) -> String {
    (0..14)
        .map(|index| {
            format!(
                "2026-05-{}\nreplacement day {}\n- 60m 70%",
                start_day + index,
                index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn shifted_replacement_plan() -> String {
    (0..14)
        .map(|index| {
            format!(
                "2026-05-{}\nshifted replacement day {}\n- 60m 70%",
                19 + index,
                index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn planned_workout_entity(operation_key: &str, date: &str) -> CanonicalEntityRef {
    CanonicalEntityRef::new(
        CanonicalEntityKind::PlannedWorkout,
        format!("{operation_key}:{date}"),
    )
}
