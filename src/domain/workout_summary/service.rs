use crate::domain::identity::{Clock, IdGenerator};

use super::{
    validate_message_content, validate_rpe, BoxFuture, CoachReply, CoachReplyClaimResult,
    CoachReplyOperation, CoachReplyOperationRepository, CoachReplyOperationStatus,
    CompletedCoachReply, ConversationMessage, MessageRole, PersistedUserMessage, SendMessageResult,
    WorkoutCoach, WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
};

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
    coach: std::sync::Arc<dyn WorkoutCoach>,
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub fn new(repository: Repo, reply_operations: Ops, clock: Time, ids: Ids) -> Self {
        Self::with_coach(
            repository,
            reply_operations,
            clock,
            ids,
            std::sync::Arc::new(super::MockWorkoutCoach),
        )
    }

    pub fn with_coach(
        repository: Repo,
        reply_operations: Ops,
        clock: Time,
        ids: Ids,
        coach: std::sync::Arc<dyn WorkoutCoach>,
    ) -> Self {
        Self {
            repository,
            reply_operations,
            clock,
            ids,
            coach,
        }
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
        let summary = self.get_existing_summary(user_id, workout_id).await?;
        if summary.saved_at_epoch_seconds.is_some() {
            return Err(WorkoutSummaryError::Locked);
        }
        if summary.rpe.is_none() {
            return Err(WorkoutSummaryError::Validation(
                "rpe must be set before chatting with coach".to_string(),
            ));
        }
        let content = validate_message_content(&content)?;
        let now = self.clock.now_epoch_seconds();
        let message = ConversationMessage {
            id: self.ids.new_id("message"),
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
        })
    }
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryUseCases for WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move { service.get_existing_summary(&user_id, &workout_id).await })
    }

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            if let Some(existing) = service
                .repository
                .find_by_user_id_and_workout_id(&user_id, &workout_id)
                .await?
            {
                return Ok(existing);
            }

            let now = service.clock.now_epoch_seconds();
            let summary = WorkoutSummary::new(
                service.ids.new_id("workout-summary"),
                user_id,
                workout_id,
                now,
            );
            let summary_user_id = summary.user_id.clone();
            let summary_workout_id = summary.workout_id.clone();

            match service.repository.create(summary).await {
                Ok(summary) => Ok(summary),
                Err(WorkoutSummaryError::AlreadyExists) => service
                    .repository
                    .find_by_user_id_and_workout_id(&summary_user_id, &summary_workout_id)
                    .await?
                    .ok_or(WorkoutSummaryError::NotFound),
                Err(error) => Err(error),
            }
        })
    }

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut summaries = service
                .repository
                .find_by_user_id_and_workout_ids(&user_id, workout_ids)
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
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let rpe = validate_rpe(rpe)?;
            let existing = service.get_existing_summary(&user_id, &workout_id).await?;
            if existing.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            let now = service.clock.now_epoch_seconds();

            service
                .repository
                .update_rpe(&user_id, &workout_id, rpe, now)
                .await?;

            service.get_existing_summary(&user_id, &workout_id).await
        })
    }

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let existing = service.get_existing_summary(&user_id, &workout_id).await?;
            if existing.saved_at_epoch_seconds.is_some() {
                return Ok(existing);
            }
            if existing.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before saving workout summary".to_string(),
                ));
            }

            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .set_saved_state(&user_id, &workout_id, Some(now), now)
                .await?;

            service.get_existing_summary(&user_id, &workout_id).await
        })
    }

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let existing = service.get_existing_summary(&user_id, &workout_id).await?;
            if existing.saved_at_epoch_seconds.is_none() {
                return Ok(existing);
            }
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .set_saved_state(&user_id, &workout_id, None, now)
                .await?;

            service.get_existing_summary(&user_id, &workout_id).await
        })
    }

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let persisted = service
                .append_user_message(&user_id, &workout_id, content)
                .await?;
            let reply = service
                .generate_coach_reply(&user_id, &workout_id, persisted.user_message.id.clone())
                .await?;

            Ok(SendMessageResult {
                summary: reply.summary,
                user_message: persisted.user_message,
                coach_message: reply.coach_message,
            })
        })
    }

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let user_message = service
                .append_message_with_role(&user_id, &workout_id, MessageRole::User, content)
                .await?;

            let summary = service.get_existing_summary(&user_id, &workout_id).await?;

            Ok(PersistedUserMessage {
                summary,
                user_message,
            })
        })
    }

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let summary = service.get_existing_summary(&user_id, &workout_id).await?;
            let user_message = service
                .get_message_by_id(&user_id, &workout_id, &user_message_id)
                .await?;

            if user_message.role != MessageRole::User {
                return Err(WorkoutSummaryError::Validation(
                    "user message must be persisted before generating coach reply".to_string(),
                ));
            }

            let now = service.clock.now_epoch_seconds();
            let pending_operation = CoachReplyOperation::pending(
                user_id.clone(),
                workout_id.clone(),
                user_message.id.clone(),
                Some(format!("workout-summary:{user_id}:{workout_id}")),
                now,
            );
            let operation = match service
                .reply_operations
                .claim_pending(pending_operation.clone())
                .await?
            {
                CoachReplyClaimResult::Claimed(operation) => operation,
                CoachReplyClaimResult::Existing(existing) => match existing.status {
                    CoachReplyOperationStatus::Completed => {
                        return service
                            .get_completed_reply(&user_id, &workout_id, existing)
                            .await;
                    }
                    CoachReplyOperationStatus::Pending => {
                        return Err(WorkoutSummaryError::ReplyAlreadyPending);
                    }
                    CoachReplyOperationStatus::Failed => pending_operation,
                },
            };

            let llm_response = match service
                .coach
                .reply(&user_id, &summary, &user_message.content)
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let failed =
                        operation.mark_failed(error.to_string(), service.clock.now_epoch_seconds());
                    service.reply_operations.upsert(failed).await?;
                    return Err(WorkoutSummaryError::Llm(error));
                }
            };

            let coach_message = service
                .append_message_with_role(
                    &user_id,
                    &workout_id,
                    MessageRole::Coach,
                    llm_response.message.clone(),
                )
                .await?;

            let completed = operation.mark_completed(CompletedCoachReply {
                provider: llm_response.provider,
                model: llm_response.model.clone(),
                provider_request_id: llm_response.provider_request_id.clone(),
                coach_message_id: coach_message.id.clone(),
                provider_cache_id: llm_response.cache.provider_cache_id.clone(),
                token_usage: llm_response.usage.clone(),
                cache_usage: llm_response.cache.clone(),
                updated_at_epoch_seconds: service.clock.now_epoch_seconds(),
            });
            service.reply_operations.upsert(completed).await?;

            let summary = service.get_existing_summary(&user_id, &workout_id).await?;

            Ok(CoachReply {
                summary,
                coach_message,
            })
        })
    }
}
