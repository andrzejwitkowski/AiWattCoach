use crate::domain::identity::{Clock, IdGenerator};

use super::{
    validate_message_content, validate_rpe, BoxFuture, CoachReply, ConversationMessage,
    MessageRole, PersistedUserMessage, SendMessageResult, WorkoutCoach, WorkoutSummary,
    WorkoutSummaryError, WorkoutSummaryRepository,
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
        user_message_content: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>>;
}

#[derive(Clone)]
pub struct WorkoutSummaryService<Repo, Time, Ids>
where
    Repo: WorkoutSummaryRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    repository: Repo,
    clock: Time,
    ids: Ids,
    coach: std::sync::Arc<dyn WorkoutCoach>,
}

impl<Repo, Time, Ids> WorkoutSummaryService<Repo, Time, Ids>
where
    Repo: WorkoutSummaryRepository,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(repository: Repo, clock: Time, ids: Ids) -> Self {
        Self::with_coach(
            repository,
            clock,
            ids,
            std::sync::Arc::new(super::MockWorkoutCoach),
        )
    }

    pub fn with_coach(
        repository: Repo,
        clock: Time,
        ids: Ids,
        coach: std::sync::Arc<dyn WorkoutCoach>,
    ) -> Self {
        Self {
            repository,
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
}

impl<Repo, Time, Ids> WorkoutSummaryUseCases for WorkoutSummaryService<Repo, Time, Ids>
where
    Repo: WorkoutSummaryRepository,
    Time: Clock,
    Ids: IdGenerator,
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
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut summaries = repository
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
            service.get_existing_summary(&user_id, &workout_id).await?;
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
                .generate_coach_reply(
                    &user_id,
                    &workout_id,
                    persisted.user_message.content.clone(),
                )
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
        user_message_content: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let summary = service.get_existing_summary(&user_id, &workout_id).await?;
            let coach_message = service
                .append_message_with_role(
                    &user_id,
                    &workout_id,
                    MessageRole::Coach,
                    service.coach.reply(&summary, &user_message_content),
                )
                .await?;

            let summary = service.get_existing_summary(&user_id, &workout_id).await?;

            Ok(CoachReply {
                summary,
                coach_message,
            })
        })
    }
}
