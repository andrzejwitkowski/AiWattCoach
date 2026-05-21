use std::collections::HashSet;

use crate::domain::{
    external_sync::{CanonicalEntityKind, CanonicalEntityRef, ExternalSyncStateRepository},
    identity::Clock,
    settings::UserSettingsUseCases,
    training_plan::{
        TrainingPlanDay, TrainingPlanError, TrainingPlanPartialReplacement,
        TrainingPlanProjectedDay, TrainingPlanProjectionRepository, TrainingPlanSnapshot,
    },
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorDecision,
        TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
        TrainingPlanSupervisorReplacementApplyResult, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorStatus,
    },
};

use super::TrainingPlanSupervisorService;

impl<Repo, Settings, Time, Batch, SyncStates>
    TrainingPlanSupervisorService<Repo, Settings, Time, Batch, SyncStates>
where
    Repo: TrainingPlanSupervisorOperationRepository,
    Settings: UserSettingsUseCases + Clone + 'static,
    Time: Clock,
    Batch: TrainingPlanSupervisorBatchPort + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    pub fn complete_review<Projections>(
        &self,
        projections: Projections,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
    {
        let repository = self.repository.clone();
        let sync_states = self.sync_states.clone();
        let worker_operation_key = worker_operation_key.to_string();
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        Box::pin(async move {
            let completed = repository
                .complete_review_if_pending(&worker_operation_key, review, now_epoch_seconds)
                .await?;
            projections
                .update_supervisor_status(
                    &completed.user_id,
                    &completed.worker_operation_key,
                    Some(completed.status),
                    completed.updated_at_epoch_seconds,
                )
                .await?;
            let completed = if completed.status == TrainingPlanSupervisorStatus::Replaced
                && completed.replacement_apply_result.is_none()
            {
                apply_replacement_review(&projections, &sync_states, &completed, now_epoch_seconds)
                    .await?
            } else {
                completed
            };
            repository.upsert(completed).await
        })
    }

    pub fn complete_review_and_refresh<Projections, Refresh>(
        &self,
        projections: Projections,
        refresh: Refresh,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>
    where
        Projections: TrainingPlanProjectionRepository + Clone,
        Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort + Clone,
    {
        let service = self.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            let completed = service
                .complete_review(projections.clone(), &worker_operation_key, review)
                .await?;
            let active_days = projections
                .find_active_by_user_id_and_operation_key(
                    &completed.user_id,
                    &completed.worker_operation_key,
                )
                .await?;
            if let Some((oldest, newest)) = active_day_range(&active_days) {
                refresh
                    .refresh_range_for_user(&completed.user_id, &oldest, &newest)
                    .await
                    .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            }
            Ok(completed)
        })
    }
}

fn active_day_range(active_days: &[TrainingPlanProjectedDay]) -> Option<(String, String)> {
    let oldest = active_days.iter().map(|day| day.date.as_str()).min()?;
    let newest = active_days.iter().map(|day| day.date.as_str()).max()?;
    Some((oldest.to_string(), newest.to_string()))
}

async fn apply_replacement_review<Projections, SyncStates>(
    projections: &Projections,
    sync_states: &SyncStates,
    operation: &TrainingPlanSupervisorOperation,
    now_epoch_seconds: i64,
) -> Result<TrainingPlanSupervisorOperation, TrainingPlanError>
where
    Projections: TrainingPlanProjectionRepository + Clone,
    SyncStates: ExternalSyncStateRepository + Clone,
{
    let review = operation.review.as_ref().ok_or_else(|| {
        TrainingPlanError::Validation("replacement supervisor review is missing".to_string())
    })?;
    if review.decision != TrainingPlanSupervisorDecision::Replace {
        return Ok(operation.clone());
    }
    let replacement_plan = review.plan.as_ref().ok_or_else(|| {
        TrainingPlanError::Validation(
            "replacement supervisor review must include a replacement plan".to_string(),
        )
    })?;
    let active_days = projections
        .find_active_by_user_id_and_operation_key(
            &operation.user_id,
            &operation.worker_operation_key,
        )
        .await?;
    let replacement_snapshot =
        parse_replacement_snapshot(operation, replacement_plan, &active_days, now_epoch_seconds)?;
    validate_replacement_dates_match_active_window(&replacement_snapshot.days, &active_days)?;
    let replacement_days =
        build_replacement_projected_days(&replacement_snapshot, &active_days, now_epoch_seconds);
    let protected_through_date = replacement_protection_date(&active_days, now_epoch_seconds);
    let eligible_dates = eligible_replacement_dates(
        sync_states,
        &operation.user_id,
        &operation.worker_operation_key,
        &active_days,
        &protected_through_date,
    )
    .await?;
    projections
        .apply_partial_replacement(TrainingPlanPartialReplacement {
            snapshot: preserve_non_applied_days_in_snapshot(
                replacement_snapshot,
                &active_days,
                &eligible_dates.applied_dates,
            ),
            projected_days: replacement_days,
            replace_dates: eligible_dates.applied_dates.clone(),
            replaced_at_epoch_seconds: now_epoch_seconds,
        })
        .await?;

    Ok(operation.with_replacement_apply_result(
        TrainingPlanSupervisorReplacementApplyResult {
            applied_dates: eligible_dates.applied_dates,
            skipped_dates: eligible_dates.skipped_dates,
            skipped_synced_dates: eligible_dates.skipped_synced_dates,
            applied_at_epoch_seconds: now_epoch_seconds,
        },
        now_epoch_seconds,
    ))
}

fn validate_replacement_dates_match_active_window(
    replacement_days: &[TrainingPlanDay],
    active_days: &[TrainingPlanProjectedDay],
) -> Result<(), TrainingPlanError> {
    let mut replacement_dates = replacement_days
        .iter()
        .map(|day| day.date.as_str())
        .collect::<Vec<_>>();
    replacement_dates.sort_unstable();
    let mut active_dates = active_days
        .iter()
        .map(|day| day.date.as_str())
        .collect::<Vec<_>>();
    active_dates.sort_unstable();
    if replacement_dates != active_dates {
        return Err(TrainingPlanError::Validation(
            "training plan supervisor replacement dates must match active projection window"
                .to_string(),
        ));
    }
    Ok(())
}

struct EligibleReplacementDates {
    applied_dates: Vec<String>,
    skipped_dates: Vec<String>,
    skipped_synced_dates: Vec<String>,
}

async fn eligible_replacement_dates<SyncStates>(
    sync_states: &SyncStates,
    user_id: &str,
    operation_key: &str,
    active_days: &[TrainingPlanProjectedDay],
    today: &str,
) -> Result<EligibleReplacementDates, TrainingPlanError>
where
    SyncStates: ExternalSyncStateRepository + Clone,
{
    let entities = active_days
        .iter()
        .map(|day| planned_workout_entity(operation_key, &day.date))
        .collect::<Vec<_>>();
    let owned_entities = sync_states
        .find_by_canonical_entities(user_id, &entities)
        .await
        .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
        .into_iter()
        .map(|state| state.canonical_entity.entity_id)
        .collect::<HashSet<_>>();
    let mut applied_dates = Vec::new();
    let mut skipped_dates = Vec::new();
    let mut skipped_synced_dates = Vec::new();
    for day in active_days {
        let entity_id = planned_workout_entity(operation_key, &day.date).entity_id;
        let is_synced = owned_entities.contains(&entity_id);
        if is_synced {
            skipped_synced_dates.push(day.date.clone());
        }
        if day.date.as_str() <= today {
            skipped_dates.push(day.date.clone());
            continue;
        }
        if is_synced {
            skipped_dates.push(day.date.clone());
        } else {
            applied_dates.push(day.date.clone());
        }
    }
    Ok(EligibleReplacementDates {
        applied_dates,
        skipped_dates,
        skipped_synced_dates,
    })
}

fn parse_replacement_snapshot(
    operation: &TrainingPlanSupervisorOperation,
    replacement_plan: &str,
    active_days: &[TrainingPlanProjectedDay],
    now_epoch_seconds: i64,
) -> Result<TrainingPlanSnapshot, TrainingPlanError> {
    let parsed = crate::domain::intervals::parse_planned_workout_days(replacement_plan)
        .map_err(|error| TrainingPlanError::Validation(error.to_string()))?;
    let mut days = Vec::new();
    for day in parsed.days {
        let date = day.date.clone();
        days.push(TrainingPlanDay {
            date,
            rest_day: day.is_rest_day(),
            rest_day_reason: day.rest_day_reason().map(ToString::to_string),
            workout: day.into_workout(),
        });
    }
    if days.len() != 14 {
        return Err(TrainingPlanError::Validation(
            "training plan supervisor replacement must contain exactly 14 days".to_string(),
        ));
    }
    let start_date = days
        .first()
        .map(|day| day.date.clone())
        .ok_or_else(|| TrainingPlanError::Validation("replacement plan is empty".to_string()))?;
    let end_date = days
        .last()
        .map(|day| day.date.clone())
        .ok_or_else(|| TrainingPlanError::Validation("replacement plan is empty".to_string()))?;
    Ok(TrainingPlanSnapshot {
        user_id: operation.user_id.clone(),
        workout_id: active_days
            .first()
            .map(|day| day.workout_id.clone())
            .unwrap_or_else(|| operation.worker_operation_key.clone()),
        operation_key: operation.worker_operation_key.clone(),
        saved_at_epoch_seconds: operation.worker_saved_at_epoch_seconds,
        start_date,
        end_date,
        days,
        created_at_epoch_seconds: now_epoch_seconds,
    })
}

fn build_replacement_projected_days(
    snapshot: &TrainingPlanSnapshot,
    active_days: &[TrainingPlanProjectedDay],
    now_epoch_seconds: i64,
) -> Vec<TrainingPlanProjectedDay> {
    snapshot
        .days
        .iter()
        .map(|day| TrainingPlanProjectedDay {
            user_id: snapshot.user_id.clone(),
            workout_id: active_days
                .iter()
                .find(|active_day| active_day.date == day.date)
                .map(|active_day| active_day.workout_id.clone())
                .unwrap_or_else(|| snapshot.workout_id.clone()),
            operation_key: snapshot.operation_key.clone(),
            date: day.date.clone(),
            rest_day: day.rest_day,
            rest_day_reason: day.rest_day_reason.clone(),
            workout: day.workout.clone(),
            supervisor_status: Some(TrainingPlanSupervisorStatus::Replaced),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        })
        .collect()
}

fn preserve_non_applied_days_in_snapshot(
    mut snapshot: TrainingPlanSnapshot,
    active_days: &[TrainingPlanProjectedDay],
    applied_dates: &[String],
) -> TrainingPlanSnapshot {
    let applied_dates = applied_dates.iter().collect::<HashSet<_>>();
    for day in &mut snapshot.days {
        if applied_dates.contains(&day.date) {
            continue;
        }
        if let Some(active_day) = active_days
            .iter()
            .find(|active_day| active_day.date == day.date)
        {
            day.rest_day = active_day.rest_day;
            day.rest_day_reason = active_day.rest_day_reason.clone();
            day.workout = active_day.workout.clone();
        }
    }
    snapshot
}

fn replacement_protection_date(
    active_days: &[TrainingPlanProjectedDay],
    now_epoch_seconds: i64,
) -> String {
    let clock_date = chrono::DateTime::from_timestamp(now_epoch_seconds, 0)
        .map(|now| now.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    active_days
        .iter()
        .map(|day| day.date.as_str())
        .min()
        .map(|first_active_date| std::cmp::max(clock_date.as_str(), first_active_date).to_string())
        .unwrap_or(clock_date)
}

fn planned_workout_entity(operation_key: &str, date: &str) -> CanonicalEntityRef {
    CanonicalEntityRef::new(
        CanonicalEntityKind::PlannedWorkout,
        format!("{operation_key}:{date}"),
    )
}
