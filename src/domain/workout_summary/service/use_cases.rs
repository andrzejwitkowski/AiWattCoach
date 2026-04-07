use super::*;

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
                if let (Some(training_plan_service), Some(saved_at_epoch_seconds)) = (
                    &service.training_plan_service,
                    existing.saved_at_epoch_seconds,
                ) {
                    match training_plan_service
                        .generate_for_saved_workout(&user_id, &workout_id, saved_at_epoch_seconds)
                        .await
                    {
                        Ok(_) => {
                            return service.get_existing_summary(&user_id, &workout_id).await;
                        }
                        Err(error) => {
                            warn!(
                                user_id,
                                workout_id,
                                saved_at_epoch_seconds,
                                error = %error,
                                "Saved workout summary remains persisted after training plan generation retry failure"
                            );
                        }
                    }
                }
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

            if let Some(training_plan_service) = &service.training_plan_service {
                if let Err(error) = training_plan_service
                    .generate_for_saved_workout(&user_id, &workout_id, now)
                    .await
                {
                    warn!(
                        user_id,
                        workout_id,
                        saved_at_epoch_seconds = now,
                        error = %error,
                        "Saved workout summary remains persisted after training plan generation failure"
                    );
                }
            }

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

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let existing = service.get_existing_summary(&user_id, &workout_id).await?;
            if existing.workout_recap_text.as_deref() == Some(recap.text.as_str())
                && existing.workout_recap_provider.as_deref() == Some(recap.provider.as_str())
                && existing.workout_recap_model.as_deref() == Some(recap.model.as_str())
                && existing.workout_recap_generated_at_epoch_seconds
                    == Some(recap.generated_at_epoch_seconds)
            {
                return Ok(existing);
            }
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .persist_workout_recap(&user_id, &workout_id, recap, now)
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
            let athlete_summary_may_regenerate_before_reply =
                if let Some(athlete_summary_service) = &service.athlete_summary_service {
                    match athlete_summary_service.get_summary_state(&user_id).await {
                        Ok(state) => state.stale,
                        Err(error) => {
                            warn!(
                                user_id = %user_id,
                                workout_id = %workout_id,
                                error = %error,
                                "athlete summary hint lookup failed while appending user message"
                            );
                            false
                        }
                    }
                } else {
                    false
                };

            Ok(PersistedUserMessage {
                summary,
                user_message,
                athlete_summary_may_regenerate_before_reply,
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
            let (athlete_summary_text, athlete_summary_was_regenerated) =
                service.ensure_athlete_summary(&user_id).await?;

            let llm_response = match service
                .coach
                .reply(
                    &user_id,
                    &summary,
                    &user_message.content,
                    athlete_summary_text.as_deref(),
                )
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
                athlete_summary_was_regenerated,
            })
        })
    }
}
