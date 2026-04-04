use std::sync::Arc;

use crate::domain::{
    identity::Clock,
    llm::{
        approximate_token_budget_for_model, hash_text, BoxFuture, LlmChatMessage, LlmChatPort,
        LlmChatRequest, LlmChatResponse, LlmContextCache, LlmContextCacheRepository, LlmError,
        LlmMessageRole, LlmProvider, UserLlmConfigProvider,
    },
    training_context::TrainingContextBuilder,
    workout_summary::{WorkoutCoach, WorkoutSummary},
};

#[derive(Clone)]
pub struct LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
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
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            config_provider,
            training_context_builder,
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
        athlete_summary_text: Option<&str>,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let config_provider = self.config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let context_cache_repository = self.context_cache_repository.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let summary = summary.clone();
        let user_message = user_message.to_string();
        let athlete_summary_text = athlete_summary_text.map(str::to_string);

        Box::pin(async move {
            let config = config_provider.get_config(&user_id).await?;
            tracing::info!(
                user_id = %user_id,
                provider = %config.provider,
                model = %config.model,
                "selected llm provider for workout summary coach"
            );
            let training_context = training_context_builder
                .build(&user_id, &summary.workout_id)
                .await?;
            let stable_context = build_stable_context(
                &summary,
                &training_context.rendered.stable_context,
                athlete_summary_text.as_deref(),
            );
            let volatile_context =
                build_volatile_context(&training_context.rendered.volatile_context);
            let estimated_request_tokens = approximate_token_usage(&stable_context)
                + approximate_token_usage(&volatile_context)
                + approximate_token_usage(WORKOUT_COACH_SYSTEM_PROMPT)
                + summary
                    .messages
                    .iter()
                    .map(|message| approximate_token_usage(&message.content))
                    .sum::<usize>()
                + approximate_token_usage(&user_message);
            let token_budget = approximate_token_budget_for_model(&config.model);
            if estimated_request_tokens > token_budget {
                return Err(LlmError::ContextTooLarge(format!(
                    "packed training context exceeds model limits: estimated {estimated_request_tokens} tokens exceeds {token_budget} token budget"
                )));
            }
            let cache_scope_key = Some(format!("workout-summary:{user_id}:{}", summary.workout_id));
            let context_hash =
                hash_text(&format!("{WORKOUT_COACH_SYSTEM_PROMPT}\n{stable_context}"));
            let reusable_cache_id = if config.provider == LlmProvider::Gemini {
                match (&context_cache_repository, cache_scope_key.as_deref()) {
                    (Some(repository), Some(scope_key)) => {
                        let reusable = match repository
                            .find_reusable(
                                &user_id,
                                &config.provider,
                                &config.model,
                                scope_key,
                                &context_hash,
                                clock.now_epoch_seconds(),
                            )
                            .await
                        {
                            Ok(reusable) => reusable,
                            Err(error) => {
                                tracing::warn!(
                                    error = %error,
                                    user_id = %user_id,
                                    provider = %config.provider,
                                    model = %config.model,
                                    cache_scope_key = %scope_key,
                                    "failed to load reusable gemini context cache"
                                );
                                None
                            }
                        };
                        if let Some(cache) = reusable {
                            tracing::info!(
                                user_id = %user_id,
                                provider = %config.provider,
                                model = %config.model,
                                cache_scope_key = %scope_key,
                                "reusing persisted gemini context cache"
                            );
                            Some(cache.provider_cache_id)
                        } else {
                            tracing::info!(
                                user_id = %user_id,
                                provider = %config.provider,
                                model = %config.model,
                                cache_scope_key = %scope_key,
                                "no reusable gemini context cache found"
                            );
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            let request = LlmChatRequest {
                user_id: user_id.clone(),
                system_prompt: WORKOUT_COACH_SYSTEM_PROMPT.to_string(),
                stable_context,
                volatile_context,
                conversation: build_conversation(&summary, &user_message),
                cache_scope_key: cache_scope_key.clone(),
                cache_key: Some(context_hash.clone()),
                reusable_cache_id,
            };

            tracing::info!(
                user_id = %user_id,
                workout_id = %summary.workout_id,
                provider = %config.provider,
                model = %config.model,
                estimated_request_tokens,
                system_prompt_chars = request.system_prompt.chars().count(),
                stable_context_chars = request.stable_context.chars().count(),
                volatile_context_chars = request.volatile_context.chars().count(),
                conversation_messages = request.conversation.len(),
                "prepared workout summary llm request"
            );

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
                            provider: config.provider.clone(),
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
                    } else {
                        tracing::info!(
                            user_id = %user_id,
                            provider = %config.provider,
                            model = %config.model,
                            "persisted reusable gemini context cache"
                        );
                    }
                }
            }

            Ok(response)
        })
    }
}

const WORKOUT_COACH_SYSTEM_PROMPT: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Use the packed training context as factual background, respond briefly, ask one focused follow-up question, and do not invent details beyond the provided context.";

fn build_stable_context(
    summary: &WorkoutSummary,
    packed_training_context: &str,
    athlete_summary_text: Option<&str>,
) -> String {
    let mut context = format!(
        "workout_summary={{\"workoutId\":\"{}\",\"rpe\":{}}}",
        summary.workout_id,
        summary
            .rpe
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
    );

    if let Some(summary_text) = athlete_summary_text.filter(|value| !value.trim().is_empty()) {
        context.push_str(&format!("\nathlete_summary_text={summary_text}"));
    }

    context.push_str(&format!(
        "\ntraining_context_stable={packed_training_context}"
    ));
    context
}

fn build_volatile_context(packed_training_context: &str) -> String {
    format!("training_context_volatile={packed_training_context}")
}

fn approximate_token_usage(value: &str) -> usize {
    value.chars().count().div_ceil(3)
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

    if let Some(last) = conversation.last_mut() {
        if last.role == LlmMessageRole::User {
            last.content = user_message.to_string();
            return conversation;
        }
    }

    conversation.push(LlmChatMessage {
        role: LlmMessageRole::User,
        content: user_message.to_string(),
    });

    conversation
}
