use futures::{stream, StreamExt, TryStreamExt};

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
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;
        self.get_existing_summary(user_id, workout_id).await
    }

    pub(super) async fn create_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        if let Some(existing) = self
            .repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?
        {
            return Ok(existing);
        }

        let now = self.clock.now_epoch_seconds();
        let summary = WorkoutSummary::new(
            self.ids.new_id("workout-summary"),
            user_id.to_string(),
            workout_id.to_string(),
            now,
        );
        let summary_user_id = summary.user_id.clone();
        let summary_workout_id = summary.workout_id.clone();

        match self.repository.create(summary).await {
            Ok(summary) => Ok(summary),
            Err(WorkoutSummaryError::AlreadyExists) => self
                .repository
                .find_by_user_id_and_workout_id(&summary_user_id, &summary_workout_id)
                .await?
                .ok_or(WorkoutSummaryError::NotFound),
            Err(error) => Err(error),
        }
    }

    pub(super) async fn list_summaries_impl(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> Result<Vec<WorkoutSummary>, WorkoutSummaryError> {
        let completed_workout_ids = stream::iter(workout_ids.into_iter().map(|workout_id| {
            let service = self.clone();
            let user_id = user_id.to_string();
            async move {
                let is_completed = service
                    .is_completed_workout_target(&user_id, &workout_id)
                    .await?;
                Ok::<_, WorkoutSummaryError>(is_completed.then_some(workout_id))
            }
        }))
        .buffered(LIST_SUMMARIES_TARGET_CHECK_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        if completed_workout_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut summaries = self
            .repository
            .find_by_user_id_and_workout_ids(user_id, completed_workout_ids)
            .await?;
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
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let rpe = validate_rpe(rpe)?;
        let existing = self.get_existing_summary(user_id, workout_id).await?;
        if existing.saved_at_epoch_seconds.is_some() {
            return Err(WorkoutSummaryError::Locked);
        }
        let now = self.clock.now_epoch_seconds();

        self.repository
            .update_rpe(user_id, workout_id, rpe, now)
            .await?;

        self.get_existing_summary(user_id, workout_id).await
    }

    pub(super) async fn reopen_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let existing = self.get_existing_summary(user_id, workout_id).await?;
        if existing.saved_at_epoch_seconds.is_none() {
            return Ok(existing);
        }
        let now = self.clock.now_epoch_seconds();
        self.repository
            .set_saved_state(user_id, workout_id, None, now)
            .await?;

        self.get_existing_summary(user_id, workout_id).await
    }

    pub(super) async fn persist_workout_recap_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let existing = self.get_existing_summary(user_id, workout_id).await?;
        if existing.workout_recap_text.as_deref() == Some(recap.text.as_str())
            && existing.workout_recap_provider.as_deref() == Some(recap.provider.as_str())
            && existing.workout_recap_model.as_deref() == Some(recap.model.as_str())
            && existing.workout_recap_generated_at_epoch_seconds
                == Some(recap.generated_at_epoch_seconds)
        {
            return Ok(existing);
        }
        let now = self.clock.now_epoch_seconds();
        self.repository
            .persist_workout_recap(user_id, workout_id, recap, now)
            .await?;

        self.get_existing_summary(user_id, workout_id).await
    }
}
