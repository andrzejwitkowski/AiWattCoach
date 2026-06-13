use std::sync::Arc;

use crate::domain::{
    athlete_summary::AthleteSummaryGenerator,
    identity::Clock,
    llm::{
        build_chat_request, conversation_timing_volatile_context, hash_text, LlmChatMessage,
        LlmChatPort, LlmChatRequestInput, LlmChatResponse, LlmError, LlmMessageRole,
        UserLlmConfigProvider, ATHLETE_SUMMARY_CALENDAR_GUARD,
    },
    training_context::TrainingContextBuilder,
};

fn athlete_summary_system_prompt() -> String {
    format!(
        "You are an elite endurance coach. Write a concise bird's-eye 360 view of the athlete using the supplied training context. Summarize profile, training patterns, strengths, weaknesses, likely limiters, fatigue/load tendencies, and practical coaching observations. Do not dump raw data arrays or reproduce raw JSON. Prefer compact prose and short bullet-like sections in plain text. {ATHLETE_SUMMARY_CALENDAR_GUARD} Do not invent section titles implying configured calendar facts. Keep load/fatigue observations observational, not scheduled time off."
    )
}

#[derive(Clone)]
pub struct AthleteSummaryLlmGenerator<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    clock: Time,
}

impl<Time> AthleteSummaryLlmGenerator<Time>
where
    Time: Clock,
{
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
            clock,
        }
    }
}

impl<Time> AthleteSummaryGenerator for AthleteSummaryLlmGenerator<Time>
where
    Time: Clock,
{
    fn generate(
        &self,
        user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<LlmChatResponse, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let clock = self.clock.clone();
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
                "{}\nathlete_summary_source_volatile={}",
                conversation_timing_volatile_context(clock.now_epoch_seconds(), None),
                context.rendered.volatile_context
            );
            let user_prompt = "Create an up-to-date athlete summary for future coaching conversations. Keep it textual, high signal, and do not include raw data dumps.";
            let request = build_chat_request(LlmChatRequestInput {
                user_id: user_id.clone(),
                system_prompt: athlete_summary_system_prompt(),
                stable_context: stable_context.clone(),
                volatile_context,
                conversation: vec![LlmChatMessage {
                    role: LlmMessageRole::User,
                    content: user_prompt.to_string(),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                    reasoning_content: None,
                }],
                cache_scope_key: Some("athlete-summary".to_string()),
                cache_key: Some(hash_text(&stable_context)),
                reusable_cache_id: None,
            });

            llm_chat_port.chat(config, request).await
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn athlete_summary_system_prompt_forbids_unconfigured_vacation_claims() {
        let prompt = super::athlete_summary_system_prompt();
        assert!(prompt.contains("unless it appears in packed prd"));
        assert!(prompt.contains("fr:true"));
    }

    #[test]
    fn athlete_summary_volatile_context_includes_conversation_timing() {
        let volatile_context = format!(
            "{}\nathlete_summary_source_volatile={{}}",
            crate::domain::llm::conversation_timing_volatile_context(1_746_489_600, None)
        );

        assert!(volatile_context.contains("currentConversationDatetime"));
        assert!(volatile_context.contains("2025-05-06T00:00:00+00:00"));
    }
}
