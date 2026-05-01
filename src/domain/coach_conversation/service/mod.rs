use std::sync::Arc;

use crate::domain::{
    identity::{Clock, IdGenerator},
    llm::{
        approximate_token_budget_for_model, hash_text, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmContextCache, LlmContextCacheRepository, LlmError, LlmMessageRole,
        LlmProvider, UserLlmConfigProvider,
    },
    settings::UserSettingsUseCases,
    training_context::TrainingContextBuilder,
};

use super::{
    validate_conversation_message_content, BoxFuture, CoachConversation, CoachConversationError,
    CoachConversationFocus, CoachConversationMessage, CoachConversationMessageRepository,
    CoachConversationMessageRole, CoachConversationReply, CoachConversationReplyClaimResult,
    CoachConversationReplyOperation, CoachConversationReplyOperationRepository,
    CoachConversationReplyOperationStatus, CoachConversationRepository, CoachConversationStatus,
    CoachConversationSurface, CompletedCoachConversationReply,
    PendingCoachConversationReplyCheckpoint, PersistedConversationUserMessage,
    SendConversationMessageResult,
};

mod scheduler;

pub use scheduler::{
    coach_conversation_reply_task_handler, SchedulerBackedCoachConversationService,
};

const POST_PROVIDER_WRITE_ATTEMPTS: usize = 2;
pub(super) const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

const CALENDAR_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reason about their training from the calendar view. Use the packed training context as factual background. This is a general coaching conversation: the athlete may ask about a workout on a given date, why a planned workout appears in the schedule, how to fuel sessions, how to approach a race strategically, or how the broader week fits together. Be direct, concise, and evidence-based. Do not invent details beyond the provided context. Do not claim that workouts were regenerated, changed, or committed unless the application explicitly says so.";

pub trait CoachConversationUseCases: Send + Sync {
    fn get_or_create_active_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn start_new_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn get_calendar_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>;

    fn send_calendar_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>>;

    fn append_calendar_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>>;

    fn generate_calendar_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>>;
}

#[derive(Clone)]
pub struct SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: CoachConversationRepository + Clone,
    Messages: CoachConversationMessageRepository + Clone,
    Ops: CoachConversationReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    conversations: Conversations,
    messages: Messages,
    reply_operations: Ops,
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    settings_service: Option<Arc<dyn UserSettingsUseCases>>,
    context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    clock: Time,
    ids: Ids,
}

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: CoachConversationRepository + Clone,
    Messages: CoachConversationMessageRepository + Clone,
    Ops: CoachConversationReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversations: Conversations,
        messages: Messages,
        reply_operations: Ops,
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            conversations,
            messages,
            reply_operations,
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
            settings_service: None,
            context_cache_repository: None,
            clock,
            ids,
        }
    }

    pub fn with_settings_service(
        mut self,
        settings_service: Arc<dyn UserSettingsUseCases>,
    ) -> Self {
        self.settings_service = Some(settings_service);
        self
    }

    pub fn with_context_cache_repository(
        mut self,
        context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.context_cache_repository = Some(context_cache_repository);
        self
    }

    async fn get_existing_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        self.conversations
            .find_by_user_id_and_conversation_id(user_id, conversation_id)
            .await?
            .ok_or(CoachConversationError::NotFound)
    }

    async fn get_existing_active_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        let conversation = self
            .get_existing_conversation(user_id, conversation_id)
            .await?;
        if conversation.status == CoachConversationStatus::Archived {
            return Err(CoachConversationError::Archived);
        }
        Ok(conversation)
    }

    async fn list_messages(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Vec<CoachConversationMessage>, CoachConversationError> {
        self.messages
            .list_by_user_id_and_conversation_id(user_id, conversation_id)
            .await
    }

    async fn create_calendar_conversation(
        &self,
        user_id: &str,
    ) -> Result<CoachConversation, CoachConversationError> {
        self.conversations
            .create(CoachConversation::new(
                self.ids.new_id("conversation"),
                user_id.to_string(),
                CoachConversationSurface::Calendar,
                CoachConversationFocus::Overview,
                self.clock.now_epoch_seconds(),
            ))
            .await
    }

    async fn archive_active_calendar_conversation_if_present(
        &self,
        user_id: &str,
    ) -> Result<(), CoachConversationError> {
        let Some(existing) = self
            .conversations
            .find_active_by_user_id_and_surface(user_id, &CoachConversationSurface::Calendar)
            .await?
        else {
            return Ok(());
        };

        self.conversations
            .update_status(
                user_id,
                &existing.conversation_id,
                CoachConversationStatus::Archived,
                self.clock.now_epoch_seconds(),
            )
            .await
    }

    async fn ensure_availability_configured_for_coach(
        &self,
        user_id: &str,
    ) -> Result<(), CoachConversationError> {
        let Some(settings_service) = &self.settings_service else {
            return Ok(());
        };

        let settings = settings_service
            .find_settings(user_id)
            .await
            .map_err(|error| match error {
                crate::domain::settings::SettingsError::Repository(message) => {
                    CoachConversationError::Repository(message)
                }
                crate::domain::settings::SettingsError::Unauthenticated => {
                    CoachConversationError::Validation("authentication is required".to_string())
                }
                crate::domain::settings::SettingsError::Validation(message) => {
                    CoachConversationError::Validation(message)
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
            Err(CoachConversationError::Validation(
                "availability must be configured before chatting with coach".to_string(),
            ))
        }
    }

    async fn append_message(
        &self,
        conversation: &CoachConversation,
        role: CoachConversationMessageRole,
        content: String,
        message_id: Option<String>,
    ) -> Result<CoachConversationMessage, CoachConversationError> {
        if conversation.status == CoachConversationStatus::Archived {
            return Err(CoachConversationError::Archived);
        }

        let content = validate_conversation_message_content(&content)?;
        if matches!(role, CoachConversationMessageRole::User) {
            self.ensure_availability_configured_for_coach(&conversation.user_id)
                .await?;
        }

        let message = self
            .messages
            .append(CoachConversationMessage {
                id: message_id.unwrap_or_else(|| self.ids.new_id("message")),
                conversation_id: conversation.conversation_id.clone(),
                user_id: conversation.user_id.clone(),
                role,
                content,
                created_at_epoch_seconds: self.clock.now_epoch_seconds(),
            })
            .await?;

        self.conversations
            .touch_updated_at(
                &conversation.user_id,
                &conversation.conversation_id,
                self.clock.now_epoch_seconds(),
            )
            .await?;

        Ok(message)
    }

    async fn load_persisted_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> Result<CoachConversationMessage, CoachConversationError> {
        let user_message = self
            .messages
            .find_by_user_id_and_conversation_id_and_message_id(
                user_id,
                conversation_id,
                user_message_id,
            )
            .await?
            .ok_or(CoachConversationError::NotFound)?;

        if user_message.role != CoachConversationMessageRole::User {
            return Err(CoachConversationError::Validation(
                "user message must be persisted before generating coach reply".to_string(),
            ));
        }

        Ok(user_message)
    }

    fn build_pending_reply_operation(
        &self,
        conversation: &CoachConversation,
        user_message: &CoachConversationMessage,
    ) -> CoachConversationReplyOperation {
        let now = self.clock.now_epoch_seconds();
        CoachConversationReplyOperation::pending(
            conversation.user_id.clone(),
            conversation.conversation_id.clone(),
            user_message.id.clone(),
            Some(format!(
                "calendar-coach:{}:{}",
                conversation.user_id,
                conversation.focus.cache_scope_suffix()
            )),
            self.ids.new_id("message"),
            now,
        )
    }

    async fn get_completed_reply(
        &self,
        conversation: &CoachConversation,
        operation: CoachConversationReplyOperation,
    ) -> Result<CoachConversationReply, CoachConversationError> {
        let coach_message_id = operation.coach_message_id.ok_or_else(|| {
            CoachConversationError::Repository(
                "completed coach reply operation missing coach message id".to_string(),
            )
        })?;
        let coach_message = self
            .messages
            .find_by_user_id_and_conversation_id_and_message_id(
                &conversation.user_id,
                &conversation.conversation_id,
                &coach_message_id,
            )
            .await?
            .ok_or(CoachConversationError::NotFound)?;
        let messages = self
            .list_messages(&conversation.user_id, &conversation.conversation_id)
            .await?;

        Ok(CoachConversationReply {
            conversation: conversation.clone(),
            messages,
            coach_message,
            athlete_summary_was_regenerated: false,
        })
    }

    fn map_existing_llm_failure(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> CoachConversationError {
        if let Some(failure_kind) = operation.failure_kind {
            return CoachConversationError::Llm(failure_kind.to_llm_error(operation.error_message));
        }

        CoachConversationError::Llm(LlmError::Internal(
            operation
                .error_message
                .unwrap_or_else(|| "failed coach reply operation missing failure kind".to_string()),
        ))
    }

    async fn persist_post_provider_operation(
        &self,
        operation: CoachConversationReplyOperation,
        write_label: &'static str,
    ) -> Result<CoachConversationReplyOperation, CoachConversationError> {
        let mut last_error = None;

        for attempt in 1..=POST_PROVIDER_WRITE_ATTEMPTS {
            match self.reply_operations.upsert(operation.clone()).await {
                Ok(saved) => {
                    if attempt > 1 {
                        tracing::info!(
                            conversation_id = %saved.conversation_id,
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
                Err(error @ CoachConversationError::Repository(_)) => {
                    if attempt == POST_PROVIDER_WRITE_ATTEMPTS {
                        return Err(error);
                    }

                    tracing::warn!(
                        conversation_id = %operation.conversation_id,
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
            CoachConversationError::Repository(
                "post-provider coach reply write failed without error".to_string(),
            )
        }))
    }

    async fn request_reply_from_llm(
        &self,
        conversation: &CoachConversation,
        messages: &[CoachConversationMessage],
        user_message: &CoachConversationMessage,
    ) -> Result<LlmChatResponse, CoachConversationError> {
        let config = self
            .llm_config_provider
            .get_config(&conversation.user_id)
            .await
            .map_err(CoachConversationError::Llm)?;
        let training_context = self
            .training_context_builder
            .build_calendar_overview_context(&conversation.user_id)
            .await
            .map_err(CoachConversationError::Llm)?;
        let stable_context =
            build_calendar_stable_context(conversation, &training_context.rendered.stable_context);
        let volatile_context = build_calendar_volatile_context(
            conversation,
            &training_context.rendered.volatile_context,
        );
        let system_prompt = calendar_coach_system_prompt();
        let estimated_request_tokens = approximate_token_usage(&stable_context)
            + approximate_token_usage(&volatile_context)
            + approximate_token_usage(&system_prompt)
            + messages
                .iter()
                .map(|message| approximate_token_usage(&message.content))
                .sum::<usize>();
        let token_budget = approximate_token_budget_for_model(&config.model);
        if estimated_request_tokens > token_budget {
            return Err(CoachConversationError::Llm(LlmError::ContextTooLarge(
                format!(
                    "packed training context exceeds model limits: estimated {estimated_request_tokens} tokens exceeds {token_budget} token budget"
                ),
            )));
        }

        let cache_scope_key = Some(format!(
            "calendar-coach:{}:{}",
            conversation.user_id,
            conversation.focus.cache_scope_suffix()
        ));
        let context_hash = hash_text(&format!("{system_prompt}\n{stable_context}"));
        let reusable_cache_id = if config.provider == LlmProvider::Gemini {
            match (&self.context_cache_repository, cache_scope_key.as_deref()) {
                (Some(repository), Some(scope_key)) => repository
                    .find_reusable(
                        &conversation.user_id,
                        &config.provider,
                        &config.model,
                        scope_key,
                        &context_hash,
                        self.clock.now_epoch_seconds(),
                    )
                    .await
                    .map_err(CoachConversationError::Llm)?
                    .map(|cache| cache.provider_cache_id),
                _ => None,
            }
        } else {
            None
        };

        let request = LlmChatRequest {
            user_id: conversation.user_id.clone(),
            system_prompt,
            stable_context,
            volatile_context,
            conversation: build_calendar_conversation(messages, &user_message.id),
            cache_scope_key: cache_scope_key.clone(),
            cache_key: Some(context_hash.clone()),
            reusable_cache_id,
        };
        let response = self
            .llm_chat_port
            .chat(config.clone(), request)
            .await
            .map_err(CoachConversationError::Llm)?;

        if config.provider == LlmProvider::Gemini {
            if let (Some(repository), Some(scope_key), Some(provider_cache_id)) = (
                self.context_cache_repository.clone(),
                cache_scope_key,
                response.cache.provider_cache_id.clone(),
            ) {
                repository
                    .upsert(LlmContextCache {
                        user_id: conversation.user_id.clone(),
                        provider: config.provider.clone(),
                        model: config.model.clone(),
                        scope_key,
                        context_hash,
                        provider_cache_id,
                        expires_at_epoch_seconds: response.cache.cache_expires_at_epoch_seconds,
                        created_at_epoch_seconds: self.clock.now_epoch_seconds(),
                        updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
                    })
                    .await
                    .map_err(CoachConversationError::Llm)?;
            }
        }

        Ok(response)
    }

    async fn try_recover_pending_operation(
        &self,
        conversation: &CoachConversation,
        operation: &CoachConversationReplyOperation,
    ) -> Result<Option<CoachConversationReply>, CoachConversationError> {
        if let Some(existing_coach_message_id) = operation.coach_message_id.clone() {
            if let Some(existing_coach_message) = self
                .messages
                .find_by_user_id_and_conversation_id_and_message_id(
                    &conversation.user_id,
                    &conversation.conversation_id,
                    &existing_coach_message_id,
                )
                .await?
            {
                let completed = operation.mark_completed_from_existing_message(
                    existing_coach_message.id.clone(),
                    self.clock.now_epoch_seconds(),
                );
                self.persist_post_provider_operation(
                    completed,
                    "recover_existing_coach_conversation_message",
                )
                .await?;
                let messages = self
                    .list_messages(&conversation.user_id, &conversation.conversation_id)
                    .await?;
                return Ok(Some(CoachConversationReply {
                    conversation: conversation.clone(),
                    messages,
                    coach_message: existing_coach_message,
                    athlete_summary_was_regenerated: false,
                }));
            }
        }

        if let Some(response_message) = operation.response_message.clone() {
            let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
                CoachConversationError::Repository(
                    "pending coach reply operation missing reserved coach message id".to_string(),
                )
            })?;
            let coach_message = self
                .append_message(
                    conversation,
                    CoachConversationMessageRole::Coach,
                    response_message,
                    Some(coach_message_id),
                )
                .await?;
            let completed = operation.mark_completed_from_existing_message(
                coach_message.id.clone(),
                self.clock.now_epoch_seconds(),
            );
            self.persist_post_provider_operation(completed, "replay_persisted_conversation_reply")
                .await?;
            let messages = self
                .list_messages(&conversation.user_id, &conversation.conversation_id)
                .await?;
            return Ok(Some(CoachConversationReply {
                conversation: conversation.clone(),
                messages,
                coach_message,
                athlete_summary_was_regenerated: false,
            }));
        }

        Ok(None)
    }
}

impl<Conversations, Messages, Ops, Time, Ids> CoachConversationUseCases
    for SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: CoachConversationRepository + Clone,
    Messages: CoachConversationMessageRepository + Clone,
    Ops: CoachConversationReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    fn get_or_create_active_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let conversation = if let Some(existing) = service
                .conversations
                .find_active_by_user_id_and_surface(&user_id, &CoachConversationSurface::Calendar)
                .await?
            {
                existing
            } else {
                service.create_calendar_conversation(&user_id).await?
            };
            let messages = service
                .list_messages(&user_id, &conversation.conversation_id)
                .await?;
            Ok((conversation, messages))
        })
    }

    fn start_new_calendar_conversation(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service
                .archive_active_calendar_conversation_if_present(&user_id)
                .await?;
            let conversation = service.create_calendar_conversation(&user_id).await?;
            Ok((conversation, Vec::new()))
        })
    }

    fn get_calendar_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>>
    {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_conversation(&user_id, &conversation_id)
                .await?;
            let messages = service.list_messages(&user_id, &conversation_id).await?;
            Ok((conversation, messages))
        })
    }

    fn send_calendar_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let persisted = service
                .append_calendar_user_message(&user_id, &conversation_id, content)
                .await?;
            let reply = service
                .generate_calendar_reply(
                    &user_id,
                    &conversation_id,
                    persisted.user_message.id.clone(),
                )
                .await?;
            Ok(SendConversationMessageResult {
                conversation: reply.conversation,
                messages: reply.messages,
                user_message: persisted.user_message,
                coach_message: reply.coach_message,
            })
        })
    }

    fn append_calendar_user_message(
        &self,
        user_id: &str,
        conversation_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&user_id, &conversation_id)
                .await?;
            let user_message = service
                .append_message(
                    &conversation,
                    CoachConversationMessageRole::User,
                    content,
                    None,
                )
                .await?;
            let messages = service.list_messages(&user_id, &conversation_id).await?;

            Ok(PersistedConversationUserMessage {
                conversation,
                messages,
                user_message,
                athlete_summary_may_regenerate_before_reply: false,
            })
        })
    }

    fn generate_calendar_reply(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let conversation = service
                .get_existing_active_conversation(&user_id, &conversation_id)
                .await?;
            let user_message = service
                .load_persisted_user_message(&user_id, &conversation_id, &user_message_id)
                .await?;
            let operation = service.build_pending_reply_operation(&conversation, &user_message);
            let stale_before_epoch_seconds =
                service.clock.now_epoch_seconds() - STALE_PENDING_TIMEOUT_SECONDS;

            let operation = match service
                .reply_operations
                .claim_pending(operation, stale_before_epoch_seconds)
                .await?
            {
                CoachConversationReplyClaimResult::Claimed(operation) => {
                    if let Some(reply) = service
                        .try_recover_pending_operation(&conversation, &operation)
                        .await?
                    {
                        return Ok(reply);
                    }
                    operation
                }
                CoachConversationReplyClaimResult::Existing(existing) => match existing.status {
                    CoachConversationReplyOperationStatus::Completed => {
                        return service.get_completed_reply(&conversation, existing).await;
                    }
                    CoachConversationReplyOperationStatus::Failed => {
                        return Err(service.map_existing_llm_failure(existing));
                    }
                    CoachConversationReplyOperationStatus::Pending => {
                        if let Some(reply) = service
                            .try_recover_pending_operation(&conversation, &existing)
                            .await?
                        {
                            return Ok(reply);
                        }
                        return Err(CoachConversationError::ReplyAlreadyPending);
                    }
                },
            };

            let messages = service
                .list_messages(&conversation.user_id, &conversation.conversation_id)
                .await?;
            let llm_response = match service
                .request_reply_from_llm(&conversation, &messages, &user_message)
                .await
            {
                Ok(response) => response,
                Err(CoachConversationError::Llm(error)) => {
                    let failed = operation.mark_failed(&error, service.clock.now_epoch_seconds());
                    service
                        .persist_post_provider_operation(failed, "persist_failed_checkpoint")
                        .await?;
                    tracing::warn!(
                        user_id = %conversation.user_id,
                        conversation_id = %conversation.conversation_id,
                        user_message_id = %user_message.id,
                        retryable = error.is_retryable(),
                        error = %error,
                        "coach conversation reply failed"
                    );
                    return Err(CoachConversationError::Llm(error));
                }
                Err(error) => return Err(error),
            };

            let operation = service
                .persist_post_provider_operation(
                    operation.record_provider_response(PendingCoachConversationReplyCheckpoint {
                        provider: llm_response.provider.clone(),
                        model: llm_response.model.clone(),
                        provider_request_id: llm_response.provider_request_id.clone(),
                        provider_cache_id: llm_response.cache.provider_cache_id.clone(),
                        token_usage: llm_response.usage.clone(),
                        cache_usage: llm_response.cache.clone(),
                        response_message: llm_response.message.clone(),
                        updated_at_epoch_seconds: service.clock.now_epoch_seconds(),
                    }),
                    "persist_provider_response_checkpoint",
                )
                .await?;

            let coach_message_id = operation.coach_message_id.clone().ok_or_else(|| {
                CoachConversationError::Repository(
                    "pending coach reply operation missing reserved coach message id".to_string(),
                )
            })?;
            let coach_message = service
                .append_message(
                    &conversation,
                    CoachConversationMessageRole::Coach,
                    llm_response.message.clone(),
                    Some(coach_message_id),
                )
                .await?;
            let completed_reply = CompletedCoachConversationReply {
                provider: llm_response.provider,
                model: llm_response.model,
                provider_request_id: llm_response.provider_request_id,
                coach_message_id: coach_message.id.clone(),
                provider_cache_id: llm_response.cache.provider_cache_id.clone(),
                token_usage: llm_response.usage,
                cache_usage: llm_response.cache,
                updated_at_epoch_seconds: service.clock.now_epoch_seconds(),
            };
            service
                .persist_post_provider_operation(
                    operation.mark_completed(completed_reply),
                    "finalize_completed_reply",
                )
                .await?;
            let messages = service
                .list_messages(&conversation.user_id, &conversation.conversation_id)
                .await?;
            Ok(CoachConversationReply {
                conversation,
                messages,
                coach_message,
                athlete_summary_was_regenerated: false,
            })
        })
    }
}

fn approximate_token_usage(value: &str) -> usize {
    value.chars().count().div_ceil(3)
}

fn calendar_coach_system_prompt() -> String {
    format!(
        "{CALENDAR_COACH_SYSTEM_PROMPT_BASE} {}",
        crate::adapters::llm::context_prelude::PACKED_TRAINING_CONTEXT_LEGEND,
    )
}

fn build_calendar_stable_context(
    conversation: &CoachConversation,
    packed_training_context: &str,
) -> String {
    let mut context = format!(
        "calendar_conversation={{\"conversationId\":\"{}\",\"surface\":\"{}\",\"focus\":\"{}\"}}",
        conversation.conversation_id,
        conversation.surface.as_str(),
        conversation.focus.kind(),
    );

    context.push_str(&format!(
        "\ntraining_context_stable={packed_training_context}"
    ));
    context
}

fn build_calendar_volatile_context(
    conversation: &CoachConversation,
    packed_training_context: &str,
) -> String {
    format!(
        "calendar_focus={{\"kind\":\"{}\"}}\ntraining_context_volatile={packed_training_context}",
        conversation.focus.kind(),
    )
}

fn build_calendar_conversation(
    messages: &[CoachConversationMessage],
    up_to_message_id: &str,
) -> Vec<LlmChatMessage> {
    let messages = match messages.iter().position(|msg| msg.id == up_to_message_id) {
        Some(pos) => &messages[..=pos],
        None => messages,
    };

    messages
        .iter()
        .filter_map(|message| match message.role {
            CoachConversationMessageRole::User => Some(LlmChatMessage {
                role: LlmMessageRole::User,
                content: message.content.clone(),
            }),
            CoachConversationMessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: message.content.clone(),
            }),
            CoachConversationMessageRole::System => None,
        })
        .collect::<Vec<_>>()
}
