use std::collections::HashMap;

use crate::domain::workout_summary::alias_batch_lookup::{
    collect_unique_lookup_workout_ids, finalize_presented_summaries,
    identity_workout_summary_lookup, load_summaries_by_workout_ids_in_scope,
    lookup_workout_ids_for_target, map_lookup_requests_to_summaries, WorkoutSummaryLookupRequest,
};

use super::*;

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
        options: WorkoutSummaryGetOptions,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id, options.alias_scope)
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
            .resolve_workout_summary_target(user_id, workout_id, None)
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
        options: WorkoutSummaryListOptions,
    ) -> Result<Vec<WorkoutSummary>, WorkoutSummaryError> {
        if let Some(alias_scope) = options.alias_scope.clone() {
            let summaries_by_requested_id = load_summaries_by_workout_ids_in_scope(
                &self.repository,
                self.completed_workout_target_service.as_deref(),
                user_id,
                &workout_ids,
                &alias_scope,
            )
            .await?;

            let summaries = summaries_by_requested_id
                .into_iter()
                .map(|(requested_workout_id, summary)| {
                    self.present_summary(summary, &requested_workout_id)
                })
                .collect::<Vec<_>>();

            return Ok(finalize_presented_summaries(summaries));
        }

        let mut lookup_requests = Vec::new();
        for workout_id in workout_ids {
            if let Some(lookup) = self
                .resolve_list_summary_lookup(user_id, workout_id)
                .await?
            {
                lookup_requests.push(lookup);
            }
        }

        let lookup_workout_ids = collect_unique_lookup_workout_ids(&lookup_requests);
        let summaries_by_lookup_id = self
            .repository
            .find_by_user_id_and_workout_ids(user_id, lookup_workout_ids)
            .await?
            .into_iter()
            .map(|summary| (summary.workout_id.clone(), summary))
            .collect::<HashMap<_, _>>();

        let summaries_by_requested_id =
            map_lookup_requests_to_summaries(lookup_requests, &summaries_by_lookup_id);
        let summaries = summaries_by_requested_id
            .into_iter()
            .map(|(requested_workout_id, summary)| {
                self.present_summary(summary, &requested_workout_id)
            })
            .collect::<Vec<_>>();

        Ok(finalize_presented_summaries(summaries))
    }

    async fn resolve_list_summary_lookup(
        &self,
        user_id: &str,
        workout_id: String,
    ) -> Result<Option<WorkoutSummaryLookupRequest>, WorkoutSummaryError> {
        let Some(service) = &self.completed_workout_target_service else {
            return Ok(Some(identity_workout_summary_lookup(workout_id)));
        };

        let Some(resolved_target) = service
            .resolve_completed_workout_target(user_id, &workout_id)
            .await?
        else {
            return Ok(None);
        };

        Ok(Some(WorkoutSummaryLookupRequest {
            requested_workout_id: workout_id.clone(),
            lookup_workout_ids: lookup_workout_ids_for_target(&workout_id, &resolved_target),
        }))
    }

    pub(super) async fn update_rpe_impl(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id, None)
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
            .update_rpe(user_id, &target.storage_workout_id, rpe, now)
            .await?;

        self.get_existing_summary(user_id, &target.storage_workout_id)
            .await
            .map(|summary| self.present_summary(summary, &target.requested_workout_id))
    }

    pub(super) async fn reopen_summary_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id, None)
            .await?;
        let existing = target
            .existing_summary
            .ok_or(WorkoutSummaryError::NotFound)?;
        if existing.saved_at_epoch_seconds.is_none() {
            return Ok(self.present_summary(existing, &target.requested_workout_id));
        }
        let now = self.clock.now_epoch_seconds();
        self.repository
            .set_saved_state(user_id, &target.storage_workout_id, None, now)
            .await?;

        self.get_existing_summary(user_id, &target.storage_workout_id)
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
            .resolve_workout_summary_target(user_id, workout_id, None)
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
            .persist_workout_recap(user_id, &target.storage_workout_id, recap, now)
            .await?;

        self.get_existing_summary(user_id, &target.storage_workout_id)
            .await
            .map(|summary| self.present_summary(summary, &target.requested_workout_id))
    }
}
