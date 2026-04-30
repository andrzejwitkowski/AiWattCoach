use futures::{stream, StreamExt, TryStreamExt};

use std::collections::HashSet;

use super::*;

const LIST_SUMMARIES_TARGET_CHECK_CONCURRENCY: usize = 8;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(super) async fn get_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;
        let summary = target
            .existing_summary
            .ok_or(WorkoutSummaryError::NotFound)?;
        Ok(self.present_summary(summary, &target.requested_workout_id))
    }

    pub(super) async fn create_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;

        if let Some(existing) = target.existing_summary {
            return Ok(self.present_summary(existing, &target.requested_workout_id));
        }

        let now = self.clock.now_epoch_seconds();
        let summary = WorkoutSummary::new(
            self.ids.new_id("workout-summary"),
            user_id.to_string(),
            target.summary_workout_id.clone(),
            now,
        );
        let summary_user_id = summary.user_id.clone();
        let summary_workout_id = summary.workout_id.clone();

        match self.repository.create(summary).await {
            Ok(summary) => Ok(self.present_summary(summary, &target.requested_workout_id)),
            Err(WorkoutSummaryError::AlreadyExists) => self
                .get_existing_summary(&summary_user_id, &summary_workout_id)
                .await
                .map(|summary| self.present_summary(summary, &target.requested_workout_id)),
            Err(error) => Err(error),
        }
    }

    pub(super) async fn list_summaries_impl(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> Result<Vec<WorkoutSummary>, WorkoutSummaryError> {
        let mut summaries =
            stream::iter(workout_ids.into_iter().map(|workout_id| {
                let service = self.clone();
                let user_id = user_id.to_string();
                async move {
                    let target = match service
                        .resolve_workout_summary_target(&user_id, &workout_id)
                        .await
                    {
                        Ok(target) => target,
                        Err(WorkoutSummaryError::Validation(_)) => {
                            return Ok::<_, WorkoutSummaryError>(None);
                        }
                        Err(error) => return Err(error),
                    };

                    Ok::<_, WorkoutSummaryError>(target.existing_summary.map(|summary| {
                        service.present_summary(summary, &target.requested_workout_id)
                    }))
                }
            }))
            .buffered(LIST_SUMMARIES_TARGET_CHECK_CONCURRENCY)
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let mut seen_summary_ids = HashSet::new();
        summaries.retain(|summary| seen_summary_ids.insert(summary.id.clone()));

        summaries.sort_by(|left, right| {
            right
                .updated_at_epoch_seconds
                .cmp(&left.updated_at_epoch_seconds)
                .then_with(|| {
                    right
                        .created_at_epoch_seconds
                        .cmp(&left.created_at_epoch_seconds)
                })
        });
        Ok(summaries)
    }

    pub(super) async fn update_rpe_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;
        let rpe = validate_rpe(rpe)?;
        let existing = target
            .existing_summary
            .ok_or(WorkoutSummaryError::NotFound)?;
        if existing.saved_at_epoch_seconds.is_some() {
            return Err(WorkoutSummaryError::Locked);
        }
        let now = self.clock.now_epoch_seconds();

        self.repository
            .update_rpe(user_id, &target.summary_workout_id, rpe, now)
            .await?;

        self.get_existing_summary(user_id, &target.summary_workout_id)
            .await
            .map(|summary| self.present_summary(summary, &target.requested_workout_id))
    }

    pub(super) async fn reopen_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;
        let existing = target
            .existing_summary
            .ok_or(WorkoutSummaryError::NotFound)?;
        if existing.saved_at_epoch_seconds.is_none() {
            return Ok(self.present_summary(existing, &target.requested_workout_id));
        }
        let now = self.clock.now_epoch_seconds();
        self.repository
            .set_saved_state(user_id, &target.summary_workout_id, None, now)
            .await?;

        self.get_existing_summary(user_id, &target.summary_workout_id)
            .await
            .map(|summary| self.present_summary(summary, &target.requested_workout_id))
    }

    pub(super) async fn persist_workout_recap_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
            .await?;
        let existing = target
            .existing_summary
            .ok_or(WorkoutSummaryError::NotFound)?;
        if existing.workout_recap_text.as_deref() == Some(recap.text.as_str())
            && existing.workout_recap_provider.as_deref() == Some(recap.provider.as_str())
            && existing.workout_recap_model.as_deref() == Some(recap.model.as_str())
            && existing.workout_recap_generated_at_epoch_seconds
                == Some(recap.generated_at_epoch_seconds)
        {
            return Ok(self.present_summary(existing, &target.requested_workout_id));
        }
        let now = self.clock.now_epoch_seconds();
        self.repository
            .persist_workout_recap(user_id, &target.summary_workout_id, recap, now)
            .await?;

        self.get_existing_summary(user_id, &target.summary_workout_id)
            .await
            .map(|summary| self.present_summary(summary, &target.requested_workout_id))
    }
}
