use std::sync::Arc;

use crate::domain::{
    athlete_summary::AthleteSummaryGenerator,
    llm::{
        approximate_token_budget_for_model, hash_text, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmError, LlmMessageRole, UserLlmConfigProvider,
    },
    training_context::TrainingContextBuilder,
};

const ATHLETE_SUMMARY_SYSTEM_PROMPT: &str = "You are an elite endurance coach. Write a concise bird's-eye 360 view of the athlete using the supplied training context. Summarize profile, training patterns, strengths, weaknesses, likely limiters, fatigue/load tendencies, and practical coaching considerations. Do not dump raw data arrays or reproduce raw JSON. Prefer compact prose and short bullet-like sections in plain text.";

#[derive(Clone)]
pub struct AthleteSummaryLlmGenerator {
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
}

impl AthleteSummaryLlmGenerator {
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
    ) -> Self {
        Self {
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
        }
    }
}

impl AthleteSummaryGenerator for AthleteSummaryLlmGenerator {
    fn generate(
        &self,
        user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<LlmChatResponse, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let user_id = user_id.to_string();

        Box::pin(async move {
            let config = llm_config_provider.get_config(&user_id).await?;
            let context = training_context_builder
                .build_athlete_summary_context(&user_id)
                .await?;

            let stable_context = format!(
                "athlete_summary_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "athlete_summary_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Create an up-to-date athlete summary for future coaching conversations. Keep it textual, high signal, and do not include raw data dumps.";
            let estimated_tokens = stable_context.len() / 4
                + volatile_context.len() / 4
                + user_prompt.len() / 4
                + ATHLETE_SUMMARY_SYSTEM_PROMPT.len() / 4;

            if estimated_tokens > approximate_token_budget_for_model(&config.model) {
                return Err(LlmError::ContextTooLarge(
                    "athlete summary source exceeds the selected model token budget".to_string(),
                ));
            }

            let request = LlmChatRequest {
                user_id: user_id.clone(),
                system_prompt: ATHLETE_SUMMARY_SYSTEM_PROMPT.to_string(),
                stable_context: stable_context.clone(),
                volatile_context,
                conversation: vec![LlmChatMessage {
                    role: LlmMessageRole::User,
                    content: user_prompt.to_string(),
                }],
                cache_scope_key: Some("athlete-summary".to_string()),
                cache_key: Some(hash_text(&stable_context)),
                reusable_cache_id: None,
            };

            llm_chat_port.chat(config, request).await
        })
    }
}
