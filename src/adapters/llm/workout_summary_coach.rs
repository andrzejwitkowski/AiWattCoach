use std::sync::Arc;

use crate::domain::{
    identity::Clock,
    llm::{
        hash_text, BoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmContextCache, LlmContextCacheRepository, LlmError, LlmMessageRole, LlmProvider,
        UserLlmConfigProvider,
    },
    workout_summary::{WorkoutCoach, WorkoutSummary},
};

#[derive(Clone)]
pub struct LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    config_provider: Arc<dyn UserLlmConfigProvider>,
    context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    clock: Time,
}

impl<Time> LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        config_provider: Arc<dyn UserLlmConfigProvider>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            config_provider,
            context_cache_repository: None,
            clock,
        }
    }

    pub fn with_context_cache_repository(
        mut self,
        context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.context_cache_repository = Some(context_cache_repository);
        self
    }
}

impl<Time> WorkoutCoach for LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let config_provider = self.config_provider.clone();
        let context_cache_repository = self.context_cache_repository.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let summary = summary.clone();
        let user_message = user_message.to_string();

        Box::pin(async move {
            let config = config_provider.get_config(&user_id).await?;
            let stable_context = build_stable_context(&summary);
            let cache_scope_key = Some(format!("workout-summary:{user_id}:{}", summary.workout_id));
            let context_hash =
                hash_text(&format!("{WORKOUT_COACH_SYSTEM_PROMPT}\n{stable_context}"));
            let reusable_cache_id = if config.provider == LlmProvider::Gemini {
                match (&context_cache_repository, cache_scope_key.as_deref()) {
                    (Some(repository), Some(scope_key)) => repository
                        .find_reusable(
                            &user_id,
                            &config.provider,
                            &config.model,
                            scope_key,
                            &context_hash,
                            clock.now_epoch_seconds(),
                        )
                        .await?
                        .map(|cache| cache.provider_cache_id),
                    _ => None,
                }
            } else {
                None
            };
            let request = LlmChatRequest {
                user_id: user_id.clone(),
                system_prompt: WORKOUT_COACH_SYSTEM_PROMPT.to_string(),
                stable_context,
                conversation: build_conversation(&summary, &user_message),
                cache_scope_key: cache_scope_key.clone(),
                cache_key: Some(context_hash.clone()),
                reusable_cache_id,
            };

            let response = llm_chat_port.chat(config.clone(), request).await?;

            if config.provider == LlmProvider::Gemini {
                if let (Some(repository), Some(scope_key), Some(provider_cache_id)) = (
                    context_cache_repository,
                    cache_scope_key,
                    response.cache.provider_cache_id.clone(),
                ) {
                    if let Err(error) = repository
                        .upsert(LlmContextCache {
                            user_id: user_id.clone(),
                            provider: config.provider,
                            model: config.model.clone(),
                            scope_key,
                            context_hash,
                            provider_cache_id,
                            expires_at_epoch_seconds: response.cache.cache_expires_at_epoch_seconds,
                            created_at_epoch_seconds: clock.now_epoch_seconds(),
                            updated_at_epoch_seconds: clock.now_epoch_seconds(),
                        })
                        .await
                    {
                        tracing::warn!(error = %error, "failed to persist reusable gemini context cache");
                    }
                }
            }

            Ok(response)
        })
    }
}

const WORKOUT_COACH_SYSTEM_PROMPT: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Respond briefly, ask one focused follow-up question, and stay grounded in the provided workout summary conversation only.";

fn build_stable_context(summary: &WorkoutSummary) -> String {
    format!(
        "workout_id: {}\nrpe: {}\nsaved: {}",
        summary.workout_id,
        summary
            .rpe
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        summary.saved_at_epoch_seconds.is_some()
    )
}

fn build_conversation(summary: &WorkoutSummary, user_message: &str) -> Vec<LlmChatMessage> {
    let mut conversation = summary
        .messages
        .iter()
        .cloned()
        .map(|message| LlmChatMessage {
            role: match message.role {
                crate::domain::workout_summary::MessageRole::User => LlmMessageRole::User,
                crate::domain::workout_summary::MessageRole::Coach => LlmMessageRole::Assistant,
            },
            content: message.content,
        })
        .collect::<Vec<_>>();

    if conversation.last().map(|message| message.role.clone()) != Some(LlmMessageRole::User)
        || conversation.last().map(|message| message.content.as_str()) != Some(user_message)
    {
        conversation.push(LlmChatMessage {
            role: LlmMessageRole::User,
            content: user_message.to_string(),
        });
    }

    conversation
}
