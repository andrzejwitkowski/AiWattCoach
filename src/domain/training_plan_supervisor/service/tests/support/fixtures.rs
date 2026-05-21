use crate::domain::{
    external_sync::{CanonicalEntityKind, CanonicalEntityRef},
    training_plan::TrainingPlanProjectedDay,
    training_plan_supervisor::{
        TrainingPlanSupervisorDecision, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorStatus,
    },
};

use super::{
    operation_repository::InMemorySupervisorOperationRepository,
    projection_repository::RecordingProjectionRepository,
};

pub(crate) fn accepted_review() -> TrainingPlanSupervisorReview {
    TrainingPlanSupervisorReview {
        decision: TrainingPlanSupervisorDecision::Accept,
        reason: "plan already looks good".to_string(),
        plan: None,
    }
}

pub(crate) async fn seed_pending_operation(repository: &InMemorySupervisorOperationRepository) {
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
}

pub(crate) fn seed_projected_days(projections: &RecordingProjectionRepository) {
    seed_projected_days_from(projections, 18);
}

pub(crate) fn seed_projected_days_from(
    projections: &RecordingProjectionRepository,
    start_day: usize,
) {
    for index in 0..14 {
        projections.seed_day(projected_workout_day(
            &format!("2026-05-{}", start_day + index),
            &format!("original day {}", index + 1),
        ));
    }
}

pub(crate) fn seed_active_pending_day(projections: &RecordingProjectionRepository, date: &str) {
    projections.seed_day(pending_status_day(date, None));
}

pub(crate) fn seed_superseded_pending_day(
    projections: &RecordingProjectionRepository,
    date: &str,
    superseded_at_epoch_seconds: i64,
) {
    projections.seed_day(pending_status_day(date, Some(superseded_at_epoch_seconds)));
}

pub(crate) fn replacement_plan() -> String {
    replacement_plan_from(18)
}

pub(crate) fn replacement_plan_from(start_day: usize) -> String {
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

pub(crate) fn shifted_replacement_plan() -> String {
    (0..14)
        .map(|index| {
            format!(
                "2026-05-{}\nshifted replacement day {}\n- 60m 70%",
                17 + index,
                index + 1
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn planned_workout_entity(operation_key: &str, date: &str) -> CanonicalEntityRef {
    CanonicalEntityRef::new(
        CanonicalEntityKind::PlannedWorkout,
        format!("{operation_key}:{date}"),
    )
}

fn projected_workout_day(date: &str, name: &str) -> TrainingPlanProjectedDay {
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

fn pending_status_day(
    date: &str,
    superseded_at_epoch_seconds: Option<i64>,
) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: "worker-op-1".to_string(),
        date: date.to_string(),
        rest_day: false,
        rest_day_reason: None,
        workout: None,
        supervisor_status: Some(TrainingPlanSupervisorStatus::Pending),
        superseded_at_epoch_seconds,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}
