use std::sync::Arc;

use crate::domain::{
    athlete_summary::AthleteSummaryUseCases,
    identity::{Clock, IdGenerator},
    training_plan::TrainingPlanUseCases,
};

use tracing::{info, warn};

use super::{
    validate_message_content, validate_rpe, BoxFuture, CoachReply, CoachReplyClaimResult,
    CoachReplyOperation, CoachReplyOperationRepository, CoachReplyOperationStatus,
    CompletedCoachReply, ConversationMessage, MessageRole, PendingCoachReplyCheckpoint,
    PersistedUserMessage, SendMessageResult, WorkoutCoach, WorkoutRecap, WorkoutSummary,
    WorkoutSummaryError, WorkoutSummaryRepository,
};

mod use_cases;

#[cfg(test)]
mod tests;

const POST_PROVIDER_WRITE_ATTEMPTS: usize = 2;

pub trait WorkoutSummaryUseCases: Send + Sync {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>;

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>>;

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>>;

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>>;
}

#[derive(Clone)]
pub struct WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    repository: Repo,
    reply_operations: Ops,
    clock: Time,
    ids: Ids,
    coach: Arc<dyn WorkoutCoach>,
    athlete_summary_service: Option<Arc<dyn AthleteSummaryUseCases>>,
    training_plan_service: Option<Arc<dyn TrainingPlanUseCases>>,
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

    pub fn new(repository: Repo, reply_operations: Ops, clock: Time, ids: Ids) -> Self {
        Self::with_coach(
            repository,
            reply_operations,
            clock,
            ids,
            Arc::new(super::MockWorkoutCoach),
        )
    }

    pub fn with_coach(
        repository: Repo,
        reply_operations: Ops,
        clock: Time,
        ids: Ids,
        coach: Arc<dyn WorkoutCoach>,
    ) -> Self {
        Self {
            repository,
            reply_operations,
            clock,
            ids,
            coach,
            athlete_summary_service: None,
            training_plan_service: None,
        }
    }

    pub fn with_athlete_summary_service(
        mut self,
        athlete_summary_service: Arc<dyn AthleteSummaryUseCases>,
    ) -> Self {
        self.athlete_summary_service = Some(athlete_summary_service);
        self
    }

    pub fn with_training_plan_service(
        mut self,
        training_plan_service: Arc<dyn TrainingPlanUseCases>,
    ) -> Self {
        self.training_plan_service = Some(training_plan_service);
        self
    }

    async fn get_existing_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    async fn append_message_with_role(
        &self,
        user_id: &str,
        workout_id: &str,
        role: MessageRole,
        content: String,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.append_message_with_role_and_id(user_id, workout_id, role, content, None, true)
            .await
    }

    async fn append_message_with_role_and_id(
        &self,
        user_id: &str,
        workout_id: &str,
        role: MessageRole,
        content: String,
        message_id: Option<String>,
        require_open_summary: bool,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        if require_open_summary && summary.saved_at_epoch_seconds.is_some() {
            return Err(WorkoutSummaryError::Locked);
        }
        if require_open_summary && summary.rpe.is_none() {
            return Err(WorkoutSummaryError::Validation(
                "rpe must be set before chatting with coach".to_string(),
            ));
        }
        let content = validate_message_content(&content)?;
        let now = self.clock.now_epoch_seconds();
        let message = ConversationMessage {
            id: message_id.unwrap_or_else(|| self.ids.new_id("message")),
            role,
            content,
            created_at_epoch_seconds: now,
        };

        self.repository
            .append_message(user_id, workout_id, message.clone(), now)
            .await?;

        Ok(message)
    }

    async fn get_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.repository
            .find_message_by_id(user_id, workout_id, message_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    async fn get_completed_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        operation: CoachReplyOperation,
    ) -> Result<CoachReply, WorkoutSummaryError> {
        let coach_message_id = operation.coach_message_id.ok_or_else(|| {
            WorkoutSummaryError::Repository(
                "completed coach reply operation missing coach message id".to_string(),
            )
        })?;
        let coach_message = self
            .get_message_by_id(user_id, workout_id, &coach_message_id)
            .await?;
        let summary = self.get_existing_summary(user_id, workout_id).await?;

        Ok(CoachReply {
            summary,
            coach_message,
            athlete_summary_was_regenerated: false,
        })
    }

    fn map_existing_llm_failure(&self, operation: CoachReplyOperation) -> WorkoutSummaryError {
        if let Some(failure_kind) = operation.failure_kind {
            return WorkoutSummaryError::Llm(failure_kind.to_llm_error(operation.error_message));
        }

        WorkoutSummaryError::Llm(crate::domain::llm::LlmError::Internal(
            operation
                .error_message
                .unwrap_or_else(|| "failed coach reply operation missing failure kind".to_string()),
        ))
    }

    async fn try_recover_pending_operation(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
        operation: &CoachReplyOperation,
    ) -> Result<Option<CoachReply>, WorkoutSummaryError> {
        if let Some(existing_coach_message_id) = operation.coach_message_id.clone() {
            if let Some(existing_coach_message) = self
                .repository
                .find_message_by_id(user_id, workout_id, &existing_coach_message_id)
                .await?
            {
                let completed = operation.mark_completed_from_existing_message(
                    existing_coach_message.id.clone(),
                    self.clock.now_epoch_seconds(),
                );
                self.reply_operations.upsert(completed).await?;
                let summary = self.get_existing_summary(user_id, workout_id).await?;
                info!(
                    workout_id = %workout_id,
                    user_message_id = %user_message_id,
                    coach_message_id = %existing_coach_message.id,
                    "recovered coach reply from persisted message"
                );
                return Ok(Some(CoachReply {
                    summary,
                    coach_message: existing_coach_message,
                    athlete_summary_was_regenerated: false,
                }));
            }
        }

        if let Some(response_message) = operation.response_message.clone() {
            let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
                WorkoutSummaryError::Repository(
                    "pending coach reply operation missing reserved coach message id".to_string(),
                )
            })?;
            let coach_message = self
                .append_message_with_role_and_id(
                    user_id,
                    workout_id,
                    MessageRole::Coach,
                    response_message,
                    Some(coach_message_id),
                    false,
                )
                .await?;
            let completed = operation.mark_completed_from_existing_message(
                coach_message.id.clone(),
                self.clock.now_epoch_seconds(),
            );
            self.reply_operations.upsert(completed).await?;
            let summary = self.get_existing_summary(user_id, workout_id).await?;
            info!(
                workout_id = %workout_id,
                user_message_id = %user_message_id,
                coach_message_id = %coach_message.id,
                "replayed persisted coach reply after partial crash"
            );
            return Ok(Some(CoachReply {
                summary,
                coach_message,
                athlete_summary_was_regenerated: false,
            }));
        }

        Ok(None)
    }

    async fn persist_post_provider_operation(
        &self,
        operation: CoachReplyOperation,
        write_label: &'static str,
    ) -> Result<CoachReplyOperation, WorkoutSummaryError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            match self.reply_operations.upsert(operation.clone()).await {
                Ok(saved) => {
                    if attempt > 1 {
                        info!(
                            workout_id = %saved.workout_id,
                            user_message_id = %saved.user_message_id,
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            operation_status = ?saved.status,
                            write_label,
                            "recovered post-provider coach reply write after retry"
                        );
                    }
                    return Ok(saved);
                }
                Err(error @ WorkoutSummaryError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        workout_id = %operation.workout_id,
                        user_message_id = %operation.user_message_id,
                        attempt,
                        max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                        operation_status = ?operation.status,
                        write_label,
                        error = %error,
                        "retrying post-provider coach reply write after repository error"
                    );
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            WorkoutSummaryError::Repository(
                "post-provider coach reply write failed without error".to_string(),
            )
        }))
    }

    async fn ensure_athlete_summary(
        &self,
        user_id: &str,
    ) -> Result<(Option<String>, bool), WorkoutSummaryError> {
        let Some(service) = &self.athlete_summary_service else {
            return Ok((None, false));
        };

        let ensured = match service.ensure_fresh_summary_state(user_id).await {
            Ok(ensured) => ensured,
            Err(crate::domain::athlete_summary::AthleteSummaryError::Llm(error)) => {
                return Err(WorkoutSummaryError::Llm(error));
            }
            Err(error) => {
                warn!(
                    user_id = %user_id,
                    error = %error,
                    "athlete summary skipped while generating coach reply"
                );
                return Ok((None, false));
            }
        };

        Ok((Some(ensured.summary.summary_text), ensured.was_regenerated))
    }
}
