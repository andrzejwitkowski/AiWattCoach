use super::super::*;
use super::push_unique_workout_id;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(in super::super) async fn get_existing_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    pub(in super::super) async fn resolve_workout_summary_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<ResolvedWorkoutSummaryTarget, WorkoutSummaryError> {
        let Some(service) = &self.completed_workout_target_service else {
            return self
                .resolve_direct_workout_summary_target(user_id, workout_id)
                .await;
        };

        let Some(resolved_target) = service
            .resolve_completed_workout_target(user_id, workout_id)
            .await?
        else {
            return Err(not_completed_workout_target_error());
        };

        self.resolve_completed_workout_summary_target(user_id, workout_id, resolved_target)
            .await
    }

    pub(in super::super) async fn validate_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        self.resolve_workout_summary_target(user_id, workout_id)
            .await
            .map(|_| ())
    }

    async fn resolve_direct_workout_summary_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<ResolvedWorkoutSummaryTarget, WorkoutSummaryError> {
        let existing_summary = self
            .repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?;

        Ok(ResolvedWorkoutSummaryTarget {
            requested_workout_id: workout_id.to_string(),
            preferred_workout_id: workout_id.to_string(),
            summary_workout_id: workout_id.to_string(),
            storage_workout_id: workout_id.to_string(),
            existing_summary,
        })
    }

    async fn resolve_completed_workout_summary_target(
        &self,
        user_id: &str,
        workout_id: &str,
        resolved_target: ResolvedCompletedWorkoutTarget,
    ) -> Result<ResolvedWorkoutSummaryTarget, WorkoutSummaryError> {
        let existing = self
            .find_existing_summary_for_candidates(
                user_id,
                candidate_workout_ids(workout_id, &resolved_target),
            )
            .await?;

        let (existing_summary, summary_workout_id, storage_workout_id) =
            if let Some((summary, storage_workout_id)) = existing {
                let summary_workout_id = summary.workout_id.clone();
                (Some(summary), summary_workout_id, storage_workout_id)
            } else {
                (
                    None,
                    resolved_target.preferred_workout_id.clone(),
                    resolved_target.preferred_workout_id.clone(),
                )
            };

        Ok(ResolvedWorkoutSummaryTarget {
            requested_workout_id: workout_id.to_string(),
            preferred_workout_id: resolved_target.preferred_workout_id,
            summary_workout_id,
            storage_workout_id,
            existing_summary,
        })
    }

    async fn find_existing_summary_for_candidates(
        &self,
        user_id: &str,
        candidate_workout_ids: Vec<String>,
    ) -> Result<Option<(WorkoutSummary, String)>, WorkoutSummaryError> {
        for candidate_workout_id in candidate_workout_ids {
            if let Some(summary) = self
                .repository
                .find_by_user_id_and_workout_id(user_id, &candidate_workout_id)
                .await?
            {
                return Ok(Some((summary, candidate_workout_id)));
            }
        }

        Ok(None)
    }
}

fn candidate_workout_ids(
    workout_id: &str,
    resolved_target: &ResolvedCompletedWorkoutTarget,
) -> Vec<String> {
    let mut candidate_workout_ids = Vec::new();
    push_unique_workout_id(&mut candidate_workout_ids, workout_id.to_string());
    push_unique_workout_id(
        &mut candidate_workout_ids,
        resolved_target.preferred_workout_id.clone(),
    );
    for equivalent_workout_id in &resolved_target.equivalent_workout_ids {
        push_unique_workout_id(&mut candidate_workout_ids, equivalent_workout_id.clone());
    }
    candidate_workout_ids
}

fn not_completed_workout_target_error() -> WorkoutSummaryError {
    WorkoutSummaryError::Validation(
        "workout summary is only available for completed workouts".to_string(),
    )
}
