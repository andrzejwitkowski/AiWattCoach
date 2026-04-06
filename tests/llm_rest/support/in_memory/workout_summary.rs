use super::*;

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<BTreeMap<SummaryKey, WorkoutSummary>>>,
}

impl InMemoryWorkoutSummaryRepository {
    pub(crate) fn seed(&self, summary: WorkoutSummary) {
        self.summaries.lock().unwrap().insert(
            (summary.user_id.clone(), summary.workout_id.clone()),
            summary,
        );
    }
}

impl WorkoutSummaryRepository for InMemoryWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move { Ok(summaries.lock().unwrap().get(&key).cloned()) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> WorkoutBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter_map(|workout_id| {
                    summaries
                        .lock()
                        .unwrap()
                        .get(&(user_id.clone(), workout_id))
                        .cloned()
                })
                .collect())
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> WorkoutBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let key = (summary.user_id.clone(), summary.workout_id.clone());
            if summaries.contains_key(&key) {
                return Err(WorkoutSummaryError::AlreadyExists);
            }
            summaries.insert(key, summary.clone());
            Ok(summary)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn append_message(
        &self,
        user_id: &str,
        workout_id: &str,
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary
                .messages
                .iter()
                .any(|existing| existing.id == message.id)
            {
                return Ok(());
            }
            summary.messages.push(message);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn set_saved_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: Option<i64>,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.saved_at_epoch_seconds = saved_at_epoch_seconds;
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.workout_recap_text = Some(recap.text);
            summary.workout_recap_provider = Some(recap.provider);
            summary.workout_recap_model = Some(recap.model);
            summary.workout_recap_generated_at_epoch_seconds =
                Some(recap.generated_at_epoch_seconds);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        let message_id = message_id.to_string();
        Box::pin(async move {
            Ok(summaries.lock().unwrap().get(&key).and_then(|summary| {
                summary
                    .messages
                    .iter()
                    .find(|message| message.id == message_id)
                    .cloned()
            }))
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<BTreeMap<ReplyOperationKey, CoachReplyOperation>>>,
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        Box::pin(async move { Ok(operations.lock().unwrap().get(&key).cloned()) })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        Box::pin(async move {
            let key = (
                operation.user_id.clone(),
                operation.workout_id.clone(),
                operation.user_message_id.clone(),
            );
            let mut operations = operations.lock().unwrap();
            if let Some(existing) = operations.get(&key).cloned() {
                let reclaimable = match existing.status {
                    CoachReplyOperationStatus::Pending => {
                        existing.is_stale(stale_before_epoch_seconds)
                    }
                    CoachReplyOperationStatus::Failed => true,
                    CoachReplyOperationStatus::Completed => false,
                };
                if reclaimable {
                    let fallback_coach_message_id =
                        operation.coach_message_id.clone().ok_or_else(|| {
                            WorkoutSummaryError::Repository(
                                "pending coach reply operation missing reserved coach message id"
                                    .to_string(),
                            )
                        })?;
                    let reclaimed = existing.reclaim(
                        fallback_coach_message_id,
                        operation.last_attempt_at_epoch_seconds,
                    );
                    operations.insert(key, reclaimed.clone());
                    return Ok(CoachReplyClaimResult::Claimed(reclaimed));
                }
                return Ok(CoachReplyClaimResult::Existing(existing));
            }

            operations.insert(key, operation.clone());
            Ok(CoachReplyClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> WorkoutBoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        Box::pin(async move {
            operations.lock().unwrap().insert(
                (
                    operation.user_id.clone(),
                    operation.workout_id.clone(),
                    operation.user_message_id.clone(),
                ),
                operation.clone(),
            );
            Ok(operation)
        })
    }
}

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}
