use futures::{stream, StreamExt, TryStreamExt};

use std::collections::HashSet;

use super::*;

const LIST_SUMMARIES_TARGET_CHECK_CONCURRENCY: usize = 8;

struct ListSummaryLookup {
    requested_workout_id: String,
    lookup_workout_ids: Vec<String>,
}

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
        let lookup_requests = stream::iter(workout_ids.into_iter().map(|workout_id| {
            let service = self.clone();
            let user_id = user_id.to_string();
            async move {
                service
                    .resolve_list_summary_lookup(&user_id, workout_id)
                    .await
            }
        }))
        .buffered(LIST_SUMMARIES_TARGET_CHECK_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        let mut lookup_workout_ids = Vec::new();
        for request in &lookup_requests {
            for workout_id in &request.lookup_workout_ids {
                push_unique_list_summary_lookup_workout_id(
                    &mut lookup_workout_ids,
                    workout_id.clone(),
                );
            }
        }

        let summaries_by_lookup_workout_id = self
            .repository
            .find_by_user_id_and_workout_ids(user_id, lookup_workout_ids)
            .await?
            .into_iter()
            .map(|summary| (summary.workout_id.clone(), summary))
            .collect::<std::collections::BTreeMap<_, _>>();

        let mut summaries = lookup_requests
            .into_iter()
            .filter_map(|request| {
                request
                    .lookup_workout_ids
                    .iter()
                    .find_map(|workout_id| summaries_by_lookup_workout_id.get(workout_id))
                    .cloned()
                    .map(|summary| self.present_summary(summary, &request.requested_workout_id))
            })
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

    async fn resolve_list_summary_lookup(
        &self,
        user_id: &str,
        workout_id: String,
    ) -> Result<Option<ListSummaryLookup>, WorkoutSummaryError> {
        let Some(service) = &self.completed_workout_target_service else {
            return Ok(Some(ListSummaryLookup {
                requested_workout_id: workout_id.clone(),
                lookup_workout_ids: vec![workout_id],
            }));
        };

        let Some(resolved_target) = service
            .resolve_completed_workout_target(user_id, &workout_id)
            .await?
        else {
            return Ok(None);
        };

        let mut lookup_workout_ids = Vec::new();
        push_unique_list_summary_lookup_workout_id(
            &mut lookup_workout_ids,
            resolved_target.preferred_workout_id,
        );
        push_unique_list_summary_lookup_workout_id(&mut lookup_workout_ids, workout_id.clone());
        for equivalent_workout_id in resolved_target.equivalent_workout_ids {
            push_unique_list_summary_lookup_workout_id(
                &mut lookup_workout_ids,
                equivalent_workout_id,
            );
        }

        Ok(Some(ListSummaryLookup {
            requested_workout_id: workout_id,
            lookup_workout_ids,
        }))
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

fn push_unique_list_summary_lookup_workout_id(workout_ids: &mut Vec<String>, workout_id: String) {
    if !workout_ids.contains(&workout_id) {
        workout_ids.push(workout_id);
    }
}
