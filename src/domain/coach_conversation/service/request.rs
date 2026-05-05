use crate::domain::llm::{
    approximate_token_budget_for_model, hash_text, LlmChatMessage, LlmChatRequest, LlmChatResponse,
    LlmContextCache, LlmError, LlmProvider, LlmProviderConfig,
};
use crate::domain::llm_tools::{run_tool_loop, LlmToolLoopOutput, ToolExecutionContext, ToolScope};

use super::{
    super::{CoachConversation, CoachConversationError, CoachConversationMessage},
    transcript::{
        build_calendar_conversation, build_calendar_stable_context,
        build_calendar_volatile_context, calendar_coach_system_prompt,
    },
    SharedCoachConversationService,
};
use crate::domain::training_context::TrainingContext;

struct PreparedCalendarLlmRequest {
    config: LlmProviderConfig,
    training_context: TrainingContext,
    system_prompt: String,
    stable_context: String,
    volatile_context: String,
    conversation: Vec<LlmChatMessage>,
    cache_scope_key: Option<String>,
    context_hash: String,
    reusable_cache_id: Option<String>,
}

impl<Conversations, Messages, Ops, Time, Ids>
    SharedCoachConversationService<Conversations, Messages, Ops, Time, Ids>
where
    Conversations: super::super::CoachConversationRepository + Clone,
    Messages: super::super::CoachConversationMessageRepository + Clone,
    Ops: super::super::CoachConversationReplyOperationRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Ids: crate::domain::identity::IdGenerator + Clone,
{
    pub(super) async fn request_reply_from_llm(
        &self,
        conversation: &CoachConversation,
        messages: &[CoachConversationMessage],
        user_message: &CoachConversationMessage,
    ) -> Result<LlmToolLoopOutput, CoachConversationError> {
        let prepared = self
            .prepare_calendar_llm_request(conversation, messages, user_message)
            .await?;
        let request = LlmChatRequest {
            user_id: conversation.user_id.clone(),
            system_prompt: prepared.system_prompt.clone(),
            stable_context: prepared.stable_context.clone(),
            volatile_context: prepared.volatile_context.clone(),
            conversation: prepared.conversation.clone(),
            cache_scope_key: prepared.cache_scope_key.clone(),
            cache_key: Some(prepared.context_hash.clone()),
            reusable_cache_id: prepared.reusable_cache_id.clone(),
            ..Default::default()
        };
        let response = self.llm_chat_port.clone();
        let response = run_tool_loop(
            response,
            prepared.config.clone(),
            request,
            ToolScope::CalendarCoachChat,
            ToolExecutionContext {
                training_context: prepared.training_context.clone(),
                today: current_date_string(self.clock.now_epoch_seconds()),
            },
            None,
        )
        .await
        .map_err(CoachConversationError::Llm)?;

        self.persist_context_cache_if_needed(conversation, &prepared, &response.response)
            .await?;

        Ok(response)
    }

    async fn prepare_calendar_llm_request(
        &self,
        conversation: &CoachConversation,
        messages: &[CoachConversationMessage],
        user_message: &CoachConversationMessage,
    ) -> Result<PreparedCalendarLlmRequest, CoachConversationError> {
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
        let system_prompt = calendar_coach_system_prompt();
        let stable_context =
            build_calendar_stable_context(conversation, &training_context.rendered.stable_context);
        let volatile_context = build_calendar_volatile_context(
            conversation,
            &training_context.rendered.volatile_context,
        );
        let llm_conversation = build_calendar_conversation(
            messages,
            &conversation.provider_transcript,
            &user_message.id,
        );

        self.ensure_request_fits_model_budget(
            &config.model,
            &system_prompt,
            &stable_context,
            &volatile_context,
            &llm_conversation,
        )?;

        let cache_scope_key = Some(format!(
            "calendar-coach:{}:{}",
            conversation.user_id,
            conversation.focus.cache_scope_suffix()
        ));
        let context_hash = hash_text(&format!("{system_prompt}\n{stable_context}"));
        let reusable_cache_id = self
            .find_reusable_cache_id(conversation, &config, &cache_scope_key, &context_hash)
            .await?;

        Ok(PreparedCalendarLlmRequest {
            config,
            training_context: training_context.context,
            system_prompt,
            stable_context,
            volatile_context,
            conversation: llm_conversation,
            cache_scope_key,
            context_hash,
            reusable_cache_id,
        })
    }

    fn ensure_request_fits_model_budget(
        &self,
        model: &str,
        system_prompt: &str,
        stable_context: &str,
        volatile_context: &str,
        conversation: &[LlmChatMessage],
    ) -> Result<(), CoachConversationError> {
        let estimated_request_tokens = approximate_token_usage(stable_context)
            + approximate_token_usage(volatile_context)
            + approximate_token_usage(system_prompt)
            + conversation
                .iter()
                .map(estimate_message_token_usage)
                .sum::<usize>();
        let token_budget = approximate_token_budget_for_model(model);

        if estimated_request_tokens > token_budget {
            return Err(CoachConversationError::Llm(LlmError::ContextTooLarge(
                format!(
                    "packed training context exceeds model limits: estimated {estimated_request_tokens} tokens exceeds {token_budget} token budget"
                ),
            )));
        }

        Ok(())
    }

    async fn find_reusable_cache_id(
        &self,
        conversation: &CoachConversation,
        config: &LlmProviderConfig,
        cache_scope_key: &Option<String>,
        context_hash: &str,
    ) -> Result<Option<String>, CoachConversationError> {
        if config.provider != LlmProvider::Gemini {
            return Ok(None);
        }

        match (&self.context_cache_repository, cache_scope_key.as_deref()) {
            (Some(repository), Some(scope_key)) => repository
                .find_reusable(
                    &conversation.user_id,
                    &config.provider,
                    &config.model,
                    scope_key,
                    context_hash,
                    self.clock.now_epoch_seconds(),
                )
                .await
                .map_err(CoachConversationError::Llm)
                .map(|cache| cache.map(|cache| cache.provider_cache_id)),
            _ => Ok(None),
        }
    }

    async fn persist_context_cache_if_needed(
        &self,
        conversation: &CoachConversation,
        prepared: &PreparedCalendarLlmRequest,
        response: &LlmChatResponse,
    ) -> Result<(), CoachConversationError> {
        if prepared.config.provider != LlmProvider::Gemini {
            return Ok(());
        }

        let (Some(repository), Some(scope_key), Some(provider_cache_id)) = (
            self.context_cache_repository.clone(),
            prepared.cache_scope_key.clone(),
            response.cache.provider_cache_id.clone(),
        ) else {
            return Ok(());
        };

        repository
            .upsert(LlmContextCache {
                user_id: conversation.user_id.clone(),
                provider: prepared.config.provider.clone(),
                model: prepared.config.model.clone(),
                scope_key,
                context_hash: prepared.context_hash.clone(),
                provider_cache_id,
                expires_at_epoch_seconds: response.cache.cache_expires_at_epoch_seconds,
                created_at_epoch_seconds: self.clock.now_epoch_seconds(),
                updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
            })
            .await
            .map_err(CoachConversationError::Llm)?;

        Ok(())
    }
}

fn current_date_string(now_epoch_seconds: i64) -> String {
    chrono::DateTime::from_timestamp(now_epoch_seconds, 0)
        .map(|time| time.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| {
            chrono::DateTime::UNIX_EPOCH
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        })
}

fn estimate_message_token_usage(message: &LlmChatMessage) -> usize {
    approximate_token_usage(&message.content)
        + message
            .tool_calls
            .iter()
            .map(|tool| {
                approximate_token_usage(&tool.name) + approximate_token_usage(&tool.arguments_json)
            })
            .sum::<usize>()
}

fn approximate_token_usage(value: &str) -> usize {
    value.chars().count().div_ceil(3)
}
