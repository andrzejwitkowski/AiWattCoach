use std::sync::Arc;

use serde_json::json;

use crate::domain::{
    ai_workflow::ValidationIssue,
    identity::Clock,
    llm::{
        BoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmError, LlmMessageRole,
        UserLlmConfigProvider,
    },
    training_context::TrainingContextBuilder,
    training_plan::{TrainingPlanError, TrainingPlanGenerator},
    workout_summary::WorkoutRecap,
};

const TRAINING_PLAN_RECAP_SYSTEM_PROMPT: &str = "You are an AI cycling coach generating a completed workout recap from packed training context. Use only the provided context, stay factual, concise, and avoid inventing details.";
const TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT: &str = "You are an AI cycling coach generating a 14-day internal cycling plan window. Return only dated workout-plan text sections that the backend parser can validate. Use the packed training context and the completed workout recap as the planning basis.";
const TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT: &str = "You are an AI cycling coach helping correct invalid dated workout sections in a 14-day internal cycling plan window. Only rewrite the invalid dated sections provided. Return only corrected dated workout-plan text sections that the backend parser can validate.";

#[derive(Clone)]
pub struct TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    clock: Time,
}

impl<Time> TrainingPlanLlmGenerator<Time>
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

impl<Time> TrainingPlanGenerator for TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    fn generate_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Generate a concise workout recap for the completed workout. Focus on execution quality, response to the session, and what matters for planning the next training window.";

            let response = llm_chat_port
                .chat(
                    config.clone(),
                    LlmChatRequest {
                        user_id,
                        system_prompt: TRAINING_PLAN_RECAP_SYSTEM_PROMPT.to_string(),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt.to_string(),
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            let generated_at_epoch_seconds = clock.now_epoch_seconds();
            Ok(WorkoutRecap::generated(
                response.message,
                response.provider.as_str(),
                response.model,
                generated_at_epoch_seconds,
            ))
        })
    }

    fn generate_initial_plan_window(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
    ) -> BoxFuture<Result<String, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let workout_recap_json = json!({
                "text": workout_recap.text,
                "provider": workout_recap.provider,
                "model": workout_recap.model,
                "generatedAt": workout_recap.generated_at_epoch_seconds,
            })
            .to_string();
            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\nworkout_recap={workout_recap_json}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Generate the next 14 dated days starting the day after the completed workout. Return only dated sections in parser-friendly workout-builder text. Include rest days explicitly when needed.";

            let response = llm_chat_port
                .chat(
                    config,
                    LlmChatRequest {
                        user_id,
                        system_prompt: TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT.to_string(),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt.to_string(),
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            Ok(response.message)
        })
    }

    fn correct_invalid_days(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
    ) -> BoxFuture<Result<String, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();
        let invalid_day_sections = invalid_day_sections.to_string();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let workout_recap_json = json!({
                "text": workout_recap.text,
                "provider": workout_recap.provider,
                "model": workout_recap.model,
                "generatedAt": workout_recap.generated_at_epoch_seconds,
            })
            .to_string();
            let stable_context = format!(
                "saved_at_epoch_seconds={saved_at_epoch_seconds}\nworkout_recap={workout_recap_json}\ntraining_plan_source_stable={}",
                context.rendered.stable_context
            );
            let volatile_context = format!(
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let issues_text = issues
                .iter()
                .map(|issue| format!("{}: {}", issue.scope, issue.message))
                .collect::<Vec<_>>()
                .join("\n");
            let user_prompt = format!(
                "Correct only these invalid dated sections. Keep valid days untouched.\n\nInvalid sections:\n{invalid_day_sections}\n\nValidation issues:\n{issues_text}"
            );

            let response = llm_chat_port
                .chat(
                    config,
                    LlmChatRequest {
                        user_id,
                        system_prompt: TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT.to_string(),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage {
                            role: LlmMessageRole::User,
                            content: user_prompt,
                        }],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    },
                )
                .await
                .map_err(map_llm_error)?;

            Ok(response.message)
        })
    }
}

fn map_llm_error(error: LlmError) -> TrainingPlanError {
    TrainingPlanError::Unavailable(error.to_string())
}
