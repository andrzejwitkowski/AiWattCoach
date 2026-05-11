use crate::domain::llm::{
    build_chat_request, current_date_string, find_reusable_context_cache,
    persist_reusable_context_cache, reusable_context_cache_key, LlmChatMessage,
    LlmChatRequestInput, LlmChatResponse, LlmProvider, LlmProviderConfig,
    ReusableContextCacheLookup, ReusableContextCacheUpsert,
};
use crate::domain::llm_tools::{
    run_tool_loop, with_tool_prompt_guidance, LlmToolLoopOutput, ToolExecutionContext, ToolScope,
};

use super::{
    super::{CoachConversation, CoachConversationError, CoachConversationMessage},
    transcript::{
        build_calendar_conversation, build_calendar_stable_context,
        build_calendar_volatile_context, calendar_coach_system_prompt,
    },
    SharedCoachConversationService,
};
struct PreparedCalendarLlmRequest {
    config: LlmProviderConfig,
    tool_context: ToolExecutionContext,
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
        let request = build_chat_request(LlmChatRequestInput {
            user_id: conversation.user_id.clone(),
            system_prompt: prepared.system_prompt.clone(),
            stable_context: prepared.stable_context.clone(),
            volatile_context: prepared.volatile_context.clone(),
            conversation: prepared.conversation.clone(),
            cache_scope_key: prepared.cache_scope_key.clone(),
            cache_key: Some(prepared.context_hash.clone()),
            reusable_cache_id: prepared.reusable_cache_id.clone(),
        });
        let response = self.llm_chat_port.clone();
        let response = run_tool_loop(
            response,
            prepared.config.clone(),
            request,
            ToolScope::CalendarCoachChat,
            prepared.tool_context.clone(),
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
        let tool_context = ToolExecutionContext {
            user_id: conversation.user_id.clone(),
            training_context: training_context.context.clone(),
            today: current_date_string(&self.clock),
            data_port: self.data_port.clone(),
            planned_workout_update_port: self.planned_workout_update_port.clone(),
        };
        let system_prompt = with_tool_prompt_guidance(
            &calendar_coach_system_prompt(),
            ToolScope::CalendarCoachChat,
            &config.provider,
            &tool_context,
        );
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

        let cache_scope_key = Some(format!(
            "calendar-coach:{}:{}",
            conversation.user_id,
            conversation.focus.cache_scope_suffix()
        ));
        let context_hash = reusable_context_cache_key(&system_prompt, &stable_context);
        let reusable_cache_id = self
            .find_reusable_cache_id(conversation, &config, &cache_scope_key, &context_hash)
            .await?;

        Ok(PreparedCalendarLlmRequest {
            config,
            tool_context,
            system_prompt,
            stable_context,
            volatile_context,
            conversation: llm_conversation,
            cache_scope_key,
            context_hash,
            reusable_cache_id,
        })
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

        find_reusable_context_cache(ReusableContextCacheLookup {
            repository: self.context_cache_repository.as_deref(),
            user_id: &conversation.user_id,
            provider: &config.provider,
            model: &config.model,
            scope_key: cache_scope_key.as_deref(),
            context_hash,
            now_epoch_seconds: self.clock.now_epoch_seconds(),
        })
        .await
        .map_err(CoachConversationError::Llm)
        .map(|cache| cache.map(|cache| cache.provider_cache_id))
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

        persist_reusable_context_cache(ReusableContextCacheUpsert {
            repository: self.context_cache_repository.as_deref(),
            user_id: &conversation.user_id,
            provider: &prepared.config.provider,
            model: &prepared.config.model,
            scope_key: prepared.cache_scope_key.as_deref(),
            context_hash: &prepared.context_hash,
            provider_cache_id: response.cache.provider_cache_id.as_deref(),
            expires_at_epoch_seconds: response.cache.cache_expires_at_epoch_seconds,
            now_epoch_seconds: self.clock.now_epoch_seconds(),
        })
        .await
        .map_err(CoachConversationError::Llm)?;

        Ok(())
    }
}
