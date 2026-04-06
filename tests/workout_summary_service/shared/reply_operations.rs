use super::*;

#[derive(Clone, Default)]
pub(crate) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<ReplyOperationStore>>,
    calls: Arc<Mutex<Vec<String>>>,
    fail_next_upsert: Arc<Mutex<Option<String>>>,
    fail_next_pending_upsert: Arc<Mutex<Option<String>>>,
    fail_next_failed_upsert: Arc<Mutex<Option<String>>>,
    fail_next_completed_upsert: Arc<Mutex<Option<String>>>,
}

impl InMemoryCoachReplyOperationRepository {
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn get(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> Option<CoachReplyOperation> {
        self.operations
            .lock()
            .unwrap()
            .get(&(
                user_id.to_string(),
                workout_id.to_string(),
                user_message_id.to_string(),
            ))
            .cloned()
    }

    pub(crate) fn seed(&self, operation: CoachReplyOperation) {
        self.calls.lock().unwrap().push(format!(
            "seed:{}:{}:{:?}",
            operation.workout_id, operation.user_message_id, operation.status
        ));
        self.operations.lock().unwrap().insert(
            (
                operation.user_id.clone(),
                operation.workout_id.clone(),
                operation.user_message_id.clone(),
            ),
            operation,
        );
    }

    pub(crate) fn fail_next_completed_upsert(&self, message: impl Into<String>) {
        *self.fail_next_completed_upsert.lock().unwrap() = Some(message.into());
    }

    pub(crate) fn fail_next_pending_upsert(&self, message: impl Into<String>) {
        *self.fail_next_pending_upsert.lock().unwrap() = Some(message.into());
    }

    pub(crate) fn fail_next_failed_upsert(&self, message: impl Into<String>) {
        *self.fail_next_failed_upsert.lock().unwrap() = Some(message.into());
    }
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        let operations = self.operations.clone();
        Box::pin(async move { Ok(operations.lock().unwrap().get(&key).cloned()) })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "claim_pending:{}:{}",
                operation.workout_id, operation.user_message_id
            ));
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
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let calls = self.calls.clone();
        let fail_next_upsert = self.fail_next_upsert.clone();
        let fail_next_pending_upsert = self.fail_next_pending_upsert.clone();
        let fail_next_failed_upsert = self.fail_next_failed_upsert.clone();
        let fail_next_completed_upsert = self.fail_next_completed_upsert.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "upsert:{}:{}:{:?}",
                operation.workout_id, operation.user_message_id, operation.status
            ));
            if let Some(message) = fail_next_upsert.lock().unwrap().take() {
                return Err(WorkoutSummaryError::Repository(message));
            }
            if operation.status == CoachReplyOperationStatus::Pending {
                if let Some(message) = fail_next_pending_upsert.lock().unwrap().take() {
                    return Err(WorkoutSummaryError::Repository(message));
                }
            }
            if operation.status == CoachReplyOperationStatus::Failed {
                if let Some(message) = fail_next_failed_upsert.lock().unwrap().take() {
                    return Err(WorkoutSummaryError::Repository(message));
                }
            }
            if operation.status == CoachReplyOperationStatus::Completed {
                if let Some(message) = fail_next_completed_upsert.lock().unwrap().take() {
                    return Err(WorkoutSummaryError::Repository(message));
                }
            }
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
