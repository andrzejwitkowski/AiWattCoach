use std::sync::Arc;

use chrono::{TimeZone, Utc};

use crate::domain::{
    identity::Clock,
    llm::{
        build_chat_request, conversation_timing_volatile_context, BoxFuture, LlmChatMessage,
        LlmChatPort, LlmChatRequestInput, LlmChatResponse, LlmError,
    },
    llm_tools::{
        run_tool_loop_with_checkpoint, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
        LlmToolLoopState, ToolExecutionContext, ToolLoopCheckpoint, ToolScope,
    },
    meso_cycle::{
        MesoCycleError, MesoCycleGenerator, MesoCycleLlmConfigPort, MesoCyclePhaseOutput,
        MesoCycleToolLoopCheckpoint, MesoCycleWindow, MESO_CYCLE_WINDOW_DAY_COUNT,
    },
    training_context::TrainingContextBuilder,
    training_plan::{parse_training_plan_llm_envelope, training_plan_llm_envelope_json_schema},
};

use super::context_prelude::PACKED_TRAINING_CONTEXT_LEGEND;
use super::training_plan_generator::{
    training_plan_output_grammar, training_plan_planning_guidelines,
};

const MESO_CYCLE_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach generating a preliminary 30-day mesocycle plan. Use packed training context and athlete constraints. Plan only the requested dated window. Do not modify the athlete's existing AI coach 14-day window. This meso plan is strategic guidance for the athlete to review on a separate calendar.";

#[derive(Clone)]
pub struct MesoCycleLlmGenerator<Time> {
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn MesoCycleLlmConfigPort>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    data_port: Arc<dyn GetSelectedWorkoutDataPort>,
    clock: Time,
}

impl<Time> MesoCycleLlmGenerator<Time>
where
    Time: Clock,
{
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        llm_config_provider: Arc<dyn MesoCycleLlmConfigPort>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        data_port: Arc<dyn GetSelectedWorkoutDataPort>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            llm_config_provider,
            training_context_builder,
            data_port,
            clock,
        }
    }
}

impl<Time> MesoCycleGenerator for MesoCycleLlmGenerator<Time>
where
    Time: Clock,
{
    fn generate_plan_window_with_state(
        &self,
        user_id: &str,
        window: &MesoCycleWindow,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<MesoCycleToolLoopCheckpoint>,
    ) -> BoxFuture<Result<MesoCyclePhaseOutput, MesoCycleError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let data_port = self.data_port.clone();
        let user_id = user_id.to_string();
        let window = window.clone();
        let clock = self.clock.clone();

        Box::pin(async move {
            let config = llm_config_provider.get_meso_cycle_config(&user_id).await?;
            let meso_end = chrono::NaiveDate::parse_from_str(&window.meso_end, "%Y-%m-%d")
                .map_err(|error| MesoCycleError::Validation(error.to_string()))?;
            let context = training_context_builder
                .build_meso_cycle_context(&user_id, meso_end)
                .await
                .map_err(map_llm_error)?;

            let stable_context = format!(
                "meso_cycle_window_start={}\nmeso_cycle_window_end={}\nmeso_cycle_source_stable={}",
                window.meso_start, window.meso_end, context.rendered.stable_context
            );
            let volatile_context = format!(
                "{}\nmeso_cycle_source_volatile={}",
                conversation_timing_volatile_context(clock.now_epoch_seconds(), None),
                context.rendered.volatile_context
            );
            let user_prompt = format!(
                "Generate exactly {MESO_CYCLE_WINDOW_DAY_COUNT} dated training days from {} through {} inclusive. Return only the JSON envelope requested by the system prompt. Put parser-friendly workout-builder text in the `plan` field, include rest days explicitly when needed, and use `Rest Day: <reason>` when you prescribe full rest.",
                window.meso_start, window.meso_end
            );

            let today = Utc
                .timestamp_opt(clock.now_epoch_seconds(), 0)
                .single()
                .map(|now| now.date_naive().format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| window.meso_start.clone());
            let tool_context = ToolExecutionContext {
                user_id: user_id.clone(),
                training_context: context.context.clone(),
                today,
                data_port: Some(data_port),
                planned_workout_update_port: None,
            };
            let system_prompt = with_tool_prompt_guidance(
                &meso_cycle_system_prompt(context.context.profile.availability_configured),
                ToolScope::TrainingPlanGeneration,
                &config.provider,
                &tool_context,
            );

            let request = build_chat_request(LlmChatRequestInput {
                user_id: tool_context.user_id.clone(),
                system_prompt,
                stable_context,
                volatile_context,
                conversation: vec![LlmChatMessage::user(user_prompt)],
                cache_scope_key: None,
                cache_key: None,
                reusable_cache_id: None,
            });

            let loop_checkpoint = checkpoint.map(map_meso_checkpoint);
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port,
                config,
                request,
                ToolScope::TrainingPlanGeneration,
                tool_context,
                restored_state,
                loop_checkpoint,
            )
            .await
            .map_err(map_llm_error)?;

            let content = require_assistant_text(&response.response)?;
            let envelope = parse_training_plan_llm_envelope(&content)
                .map_err(|error| MesoCycleError::Validation(error.to_string()))?;

            Ok(MesoCyclePhaseOutput {
                raw_response: envelope.plan().to_string(),
                description: envelope.description().map(str::to_string),
                tool_loop_state: response.state,
            })
        })
    }
}

fn meso_cycle_system_prompt(availability_configured: bool) -> String {
    format!(
        "{MESO_CYCLE_SYSTEM_PROMPT_BASE} JSON schema: {} {} {} {PACKED_TRAINING_CONTEXT_LEGEND}",
        training_plan_llm_envelope_json_schema(),
        training_plan_planning_guidelines(availability_configured),
        training_plan_output_grammar(),
    )
}

fn map_meso_checkpoint(checkpoint: MesoCycleToolLoopCheckpoint) -> ToolLoopCheckpoint {
    Arc::new(move |state| {
        let checkpoint = checkpoint.clone();
        Box::pin(async move {
            checkpoint(state)
                .await
                .map_err(|error| LlmError::Checkpoint(error.to_string()))
        })
    })
}

fn require_assistant_text(response: &LlmChatResponse) -> Result<String, MesoCycleError> {
    response
        .assistant_text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| MesoCycleError::Unavailable("LLM returned no assistant text".to_string()))
}

fn map_llm_error(error: LlmError) -> MesoCycleError {
    match error {
        LlmError::ProviderNotConfigured
        | LlmError::ModelNotConfigured
        | LlmError::CredentialsNotConfigured => MesoCycleError::NotConfigured,
        LlmError::InvalidResponse(message) => MesoCycleError::Validation(message),
        other => MesoCycleError::Unavailable(other.to_string()),
    }
}
