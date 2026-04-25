use tracing::{info, warn};

use super::*;

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    pub(super) async fn get_existing_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<WorkoutSummary, WorkoutSummaryError> {
        self.repository
            .find_by_user_id_and_workout_id(user_id, workout_id)
            .await?
            .ok_or(WorkoutSummaryError::NotFound)
    }

    pub(super) async fn validate_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        let is_completed_target = self
            .is_completed_workout_target(user_id, workout_id)
            .await?;
        if is_completed_target {
            Ok(())
        } else {
            Err(WorkoutSummaryError::Validation(
                "workout summary is only available for completed workouts".to_string(),
            ))
        }
    }

    pub(super) async fn is_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<bool, WorkoutSummaryError> {
        let Some(service) = &self.completed_workout_target_service else {
            return Ok(true);
        };

        service
            .is_completed_workout_target(user_id, workout_id)
            .await
    }

    pub(super) async fn append_message_with_role(
        &self,
        user_id: &str,
        workout_id: &str,
        role: MessageRole,
        content: String,
    ) -> Result<ConversationMessage, WorkoutSummaryError> {
        self.append_message_with_role_and_id(user_id, workout_id, role, content, None, true)
            .await
    }

    pub(super) async fn ensure_availability_configured_for_coach(
        &self,
        user_id: &str,
    ) -> Result<(), WorkoutSummaryError> {
        let Some(settings_service) = &self.settings_service else {
            return Ok(());
        };

        let settings = settings_service
            .find_settings(user_id)
            .await
            .map_err(|error| match error {
                crate::domain::settings::SettingsError::Repository(message) => {
                    WorkoutSummaryError::Repository(message)
                }
                crate::domain::settings::SettingsError::Unauthenticated => {
                    WorkoutSummaryError::Validation("authentication is required".to_string())
                }
                crate::domain::settings::SettingsError::Validation(message) => {
                    WorkoutSummaryError::Validation(message)
                }
            })?
            .unwrap_or_else(|| {
                crate::domain::settings::UserSettings::new_defaults(
                    user_id.to_string(),
                    self.clock.now_epoch_seconds(),
                )
            });

        if settings.availability.is_configured() {
            Ok(())
        } else {
            Err(WorkoutSummaryError::Validation(
                "availability must be configured before chatting with coach".to_string(),
            ))
        }
    }

    pub(super) async fn append_message_with_role_and_id(
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
        if require_open_summary && matches!(role, MessageRole::User) {
            self.ensure_availability_configured_for_coach(user_id)
                .await?;
        }
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

    pub(super) async fn get_message_by_id(
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

    pub(super) async fn get_completed_reply(
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

    pub(super) fn map_existing_llm_failure(
        &self,
        operation: CoachReplyOperation,
    ) -> WorkoutSummaryError {
        if let Some(failure_kind) = operation.failure_kind {
            return WorkoutSummaryError::Llm(failure_kind.to_llm_error(operation.error_message));
        }

        WorkoutSummaryError::Llm(crate::domain::llm::LlmError::Internal(
            operation
                .error_message
                .unwrap_or_else(|| "failed coach reply operation missing failure kind".to_string()),
        ))
    }

    pub(super) async fn try_recover_pending_operation(
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
                self.persist_post_provider_operation(completed, "recover_existing_coach_message")
                    .await?;
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
            self.persist_post_provider_operation(completed, "replay_persisted_coach_reply")
                .await?;
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

    pub(super) async fn persist_post_provider_operation(
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

    pub(super) async fn ensure_athlete_summary(
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
