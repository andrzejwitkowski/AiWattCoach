use std::sync::Arc;

use chrono::NaiveDate;
use chrono::{TimeZone, Utc};

use crate::domain::{
    identity::Clock,
    llm::{BoxFuture, LlmChatPort, LlmChatResponse, LlmError},
    llm_tools::{
        run_tool_loop_with_checkpoint, GetSelectedWorkoutDataPort, LlmToolLoopState,
        ToolLoopCheckpoint, ToolScope,
    },
    meso_cycle::{
        assemble_meso_cycle_coach_request, MesoCycleCoachPromptInput, MesoCycleError,
        MesoCycleGenerator, MesoCycleLlmConfigPort, MesoCyclePhaseOutput,
        MesoCycleToolLoopCheckpoint, MesoCycleWindow,
    },
    training_context::TrainingContextBuilder,
    training_plan::parse_training_plan_llm_envelope,
};

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
            let meso_end = NaiveDate::parse_from_str(&window.meso_end, "%Y-%m-%d")
                .map_err(|error| MesoCycleError::Validation(error.to_string()))?;
            let training_context = training_context_builder
                .build_meso_cycle_context(&user_id, meso_end)
                .await
                .map_err(map_llm_error)?;
            let today = Utc
                .timestamp_opt(clock.now_epoch_seconds(), 0)
                .single()
                .map(|now| now.date_naive().format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| window.meso_start.clone());
            let bundle = assemble_meso_cycle_coach_request(MesoCycleCoachPromptInput {
                user_id: user_id.clone(),
                config: config.clone(),
                window: window.clone(),
                training_context,
                conversation_epoch_seconds: clock.now_epoch_seconds(),
                today,
                data_port: Some(data_port),
            });
            let loop_checkpoint = checkpoint.map(map_meso_checkpoint);
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port,
                config,
                bundle.request,
                ToolScope::TrainingPlanGeneration,
                bundle.tool_context,
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
