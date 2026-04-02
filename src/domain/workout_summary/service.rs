use crate::domain::identity::{Clock, IdGenerator};

use tracing::{info, warn};

use super::{
    validate_message_content, validate_rpe, BoxFuture, CoachReply, CoachReplyClaimResult,
    CoachReplyOperation, CoachReplyOperationRepository, CoachReplyOperationStatus,
    CompletedCoachReply, ConversationMessage, MessageRole, PendingCoachReplyCheckpoint,
    PersistedUserMessage, SendMessageResult, WorkoutCoach, WorkoutSummary, WorkoutSummaryError,
    WorkoutSummaryRepository,
};

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
    const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

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
                            attempt,
                            max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                            operation_status = ?saved.status,
                            write_label,
                            "recovered post-provider coach reply write after retry"
                        );
                    }
                    return Ok(saved);
                }
                Err(error) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    warn!(
                        attempt,
                        max_attempts = POST_PROVIDER_WRITE_ATTEMPTS,
                        operation_status = ?operation.status,
                        write_label,
                        error = %error,
                        "retrying transient post-provider coach reply write"
                    );
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            WorkoutSummaryError::Repository(
                "post-provider coach reply write failed without error".to_string(),
            )
        }))
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
            let user_message = service
                .get_message_by_id(&user_id, &workout_id, &user_message_id)
                .await?;

            if user_message.role != MessageRole::User {
                return Err(WorkoutSummaryError::Validation(
                    "user message must be persisted before generating coach reply".to_string(),
                ));
            }

            let now = service.clock.now_epoch_seconds();
            let reserved_coach_message_id = service.ids.new_id("message");
            let pending_operation = CoachReplyOperation::pending(
                user_id.clone(),
                workout_id.clone(),
                user_message.id.clone(),
                Some(format!("workout-summary:{user_id}:{workout_id}")),
                reserved_coach_message_id,
                now,
            );
            let stale_before_epoch_seconds = now - Self::STALE_PENDING_TIMEOUT_SECONDS;
            let operation = match service
                .reply_operations
                .claim_pending(pending_operation.clone(), stale_before_epoch_seconds)
                .await?
            {
                CoachReplyClaimResult::Claimed(operation) => {
                    if let Some(reply) = service
                        .try_recover_pending_operation(
                            &user_id,
                            &workout_id,
                            &user_message.id,
                            &operation,
                        )
                        .await?
                    {
                        return Ok(reply);
                    }

                    operation
                }
                CoachReplyClaimResult::Existing(existing) => match existing.status {
                    CoachReplyOperationStatus::Completed => {
                        return service
                            .get_completed_reply(&user_id, &workout_id, existing)
                            .await;
                    }
                    CoachReplyOperationStatus::Failed => {
                        return Err(service.map_existing_llm_failure(existing));
                    }
                    CoachReplyOperationStatus::Pending => {
                        if let Some(reply) = service
                            .try_recover_pending_operation(
                                &user_id,
                                &workout_id,
                                &user_message.id,
                                &existing,
                            )
                            .await?
                        {
                            return Ok(reply);
                        }

                        return Err(WorkoutSummaryError::ReplyAlreadyPending);
                    }
                },
            };

            info!(
                workout_id = %workout_id,
                user_message_id = %user_message.id,
                attempt_count = operation.attempt_count,
                "requesting workout summary coach reply"
            );

            let summary = service.get_existing_summary(&user_id, &workout_id).await?;

            let llm_response = match service
                .coach
                .reply(&user_id, &summary, &user_message.content)
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let failed = operation.mark_failed(&error, service.clock.now_epoch_seconds());
                    service
                        .persist_post_provider_operation(failed, "persist_failed_checkpoint")
                        .await?;
                    warn!(
                        workout_id = %workout_id,
                        user_message_id = %user_message.id,
                        retryable = error.is_retryable(),
                        error = %error,
                        "workout summary coach reply failed"
                    );
                    return Err(WorkoutSummaryError::Llm(error));
                }
            };

            let operation = operation.record_provider_response(PendingCoachReplyCheckpoint {
                provider: llm_response.provider.clone(),
                model: llm_response.model.clone(),
                provider_request_id: llm_response.provider_request_id.clone(),
                provider_cache_id: llm_response.cache.provider_cache_id.clone(),
                token_usage: llm_response.usage.clone(),
                cache_usage: llm_response.cache.clone(),
                response_message: llm_response.message.clone(),
                updated_at_epoch_seconds: service.clock.now_epoch_seconds(),
            });
            let operation = service
                .persist_post_provider_operation(operation, "persist_success_checkpoint")
                .await?;

            let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
                WorkoutSummaryError::Repository(
                    "pending coach reply operation missing reserved coach message id".to_string(),
                )
            })?;
            let coach_message = service
                .append_message_with_role_and_id(
                    &user_id,
                    &workout_id,
                    MessageRole::Coach,
                    llm_response.message.clone(),
                    Some(coach_message_id.clone()),
                    false,
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
            service
                .persist_post_provider_operation(completed, "persist_completed_reply")
                .await?;

            let summary = service.get_existing_summary(&user_id, &workout_id).await?;

            Ok(CoachReply {
                summary,
                coach_message,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::{
        identity::{Clock, IdGenerator},
        llm::LlmError,
        workout_summary::{
            CoachReplyOperation, CoachReplyOperationRepository, MockWorkoutCoach,
            WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryService,
        },
    };

    #[derive(Clone)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            1_700_000_000
        }
    }

    #[derive(Clone)]
    struct FixedIds;

    impl IdGenerator for FixedIds {
        fn new_id(&self, prefix: &str) -> String {
            format!("{prefix}-1")
        }
    }

    #[test]
    fn map_existing_llm_failure_falls_back_to_internal_error_when_kind_is_missing() {
        let service = WorkoutSummaryService::with_coach(
            StubSummaryRepository,
            StubReplyOperations,
            FixedClock,
            FixedIds,
            Arc::new(MockWorkoutCoach),
        );

        let mut operation = CoachReplyOperation::pending(
            "user-1".to_string(),
            "workout-1".to_string(),
            "message-1".to_string(),
            Some("workout-summary:user-1:workout-1".to_string()),
            "coach-message-1".to_string(),
            1_700_000_000,
        )
        .mark_failed(
            &LlmError::Internal("persisted failure without kind".to_string()),
            1_700_000_001,
        );
        operation.failure_kind = None;

        assert_eq!(
            service.map_existing_llm_failure(operation),
            WorkoutSummaryError::Llm(LlmError::Internal(
                "persisted failure without kind".to_string()
            ))
        );
    }

    #[derive(Clone)]
    struct StubSummaryRepository;

    impl WorkoutSummaryRepository for StubSummaryRepository {
        fn find_by_user_id_and_workout_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
        ) -> super::BoxFuture<Result<Option<super::WorkoutSummary>, WorkoutSummaryError>> {
            Box::pin(async { Ok(None) })
        }

        fn find_by_user_id_and_workout_ids(
            &self,
            _user_id: &str,
            _workout_ids: Vec<String>,
        ) -> super::BoxFuture<Result<Vec<super::WorkoutSummary>, WorkoutSummaryError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn create(
            &self,
            _summary: super::WorkoutSummary,
        ) -> super::BoxFuture<Result<super::WorkoutSummary, WorkoutSummaryError>> {
            Box::pin(async { Err(WorkoutSummaryError::NotFound) })
        }

        fn update_rpe(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _rpe: u8,
            _updated_at_epoch_seconds: i64,
        ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
            Box::pin(async { Ok(()) })
        }

        fn append_message(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message: super::ConversationMessage,
            _updated_at_epoch_seconds: i64,
        ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
            Box::pin(async { Ok(()) })
        }

        fn set_saved_state(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _saved_at_epoch_seconds: Option<i64>,
            _updated_at_epoch_seconds: i64,
        ) -> super::BoxFuture<Result<(), WorkoutSummaryError>> {
            Box::pin(async { Ok(()) })
        }

        fn find_message_by_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _message_id: &str,
        ) -> super::BoxFuture<Result<Option<super::ConversationMessage>, WorkoutSummaryError>>
        {
            Box::pin(async { Ok(None) })
        }
    }

    #[derive(Clone)]
    struct StubReplyOperations;

    impl CoachReplyOperationRepository for StubReplyOperations {
        fn find_by_user_message_id(
            &self,
            _user_id: &str,
            _workout_id: &str,
            _user_message_id: &str,
        ) -> super::BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
            Box::pin(async { Ok(None) })
        }

        fn claim_pending(
            &self,
            _operation: CoachReplyOperation,
            _stale_before_epoch_seconds: i64,
        ) -> super::BoxFuture<Result<super::CoachReplyClaimResult, WorkoutSummaryError>> {
            Box::pin(async { Err(WorkoutSummaryError::NotFound) })
        }

        fn upsert(
            &self,
            operation: CoachReplyOperation,
        ) -> super::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
            Box::pin(async move { Ok(operation) })
        }
    }
}
