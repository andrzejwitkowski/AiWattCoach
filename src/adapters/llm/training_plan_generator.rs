use std::sync::Arc;

use super::context_prelude::packed_training_context_legend_with_guidance;

use crate::domain::{
    ai_workflow::ValidationIssue,
    identity::Clock,
    llm::{
        build_chat_request, conversation_timing_volatile_context,
        merge_provider_transcript_entries, BoxFuture, LlmChatMessage, LlmChatPort,
        LlmChatRequestInput, LlmChatResponse, LlmError, UserLlmConfigProvider,
    },
    llm_tools::{
        run_tool_loop_with_checkpoint, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
        LlmToolLoopOutput, LlmToolLoopState, ToolExecutionContext, ToolLoopCheckpoint, ToolScope,
    },
    training_context::TrainingContextBuilder,
    training_plan::{
        assemble_training_plan_initial_window_request,
        latest_training_plan_user_message_epoch_seconds, parse_training_plan_llm_envelope,
        planning_conversation_messages, should_retry_training_plan_llm_envelope_repair,
        training_plan_correction_system_prompt, training_plan_llm_envelope_json_schema,
        training_plan_output_grammar, training_plan_stable_context,
        training_plan_tool_context_today, TrainingPlanError, TrainingPlanGenerator,
        TrainingPlanInitialWindowPromptInput, TrainingPlanPhaseOutput, TrainingPlanPlanningContext,
        TrainingPlanToolLoopCheckpoint,
    },
    workout_summary::WorkoutRecap,
};

const TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach generating a completed workout recap from packed training context. Use only the provided context, stay factual, concise, and avoid inventing details.";
const TRAINING_PLAN_ENVELOPE_REPAIR_SYSTEM_PROMPT_BASE: &str = "You are repairing one previously generated training-plan reply into the exact app JSON envelope. Do not generate a new plan. Do not invent workouts, dates, or commentary. Extract only the existing training-plan content already present in the previous assistant reply.";
#[derive(Clone)]
pub struct TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    llm_config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
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
            data_port: None,
            clock,
        }
    }

    pub fn with_data_port(mut self, data_port: Arc<dyn GetSelectedWorkoutDataPort>) -> Self {
        self.data_port = Some(data_port);
        self
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
                "{}\ntraining_plan_source_volatile={}",
                conversation_timing_volatile_context(clock.now_epoch_seconds(), None),
                context.rendered.volatile_context
            );
            let user_prompt = "Generate a concise workout recap for the completed workout. Focus on execution quality, response to the session, and what matters for planning the next training window.";

            let response = llm_chat_port
                .chat(
                    config.clone(),
                    build_chat_request(LlmChatRequestInput {
                        user_id,
                        system_prompt: training_plan_recap_system_prompt(),
                        stable_context,
                        volatile_context,
                        conversation: vec![LlmChatMessage::user(user_prompt)],
                        cache_scope_key: None,
                        cache_key: None,
                        reusable_cache_id: None,
                    }),
                )
                .await
                .map_err(map_llm_error)?;

            let generated_at_epoch_seconds = clock.now_epoch_seconds();
            let recap_text = require_assistant_text(&response)?;
            let provider = response.provider.as_str().to_string();
            Ok(WorkoutRecap::generated(
                recap_text,
                provider,
                response.model,
                generated_at_epoch_seconds,
            ))
        })
    }

    fn generate_initial_plan_window_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        TrainingPlanLlmGenerator::generate_initial_plan_window_with_state(
            self,
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            workout_recap,
            planning_context,
            restored_state,
            checkpoint,
        )
    }

    fn correct_invalid_days_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        TrainingPlanLlmGenerator::correct_invalid_days_with_state(
            self,
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            workout_recap,
            planning_context,
            invalid_day_sections,
            issues,
            restored_state,
            checkpoint,
        )
    }
}

impl<Time> TrainingPlanLlmGenerator<Time>
where
    Time: Clock,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "training plan initial generation needs workout identity, recap context, planning context, restore state, and checkpoint callback together"
    )]
    pub fn generate_initial_plan_window_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let data_port = self.data_port.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();
        let planning_context = planning_context.cloned();
        let repair_user_id = user_id.clone();
        let clock = self.clock.clone();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let tool_context = ToolExecutionContext {
                user_id: user_id.clone(),
                training_context: context.context.clone(),
                today: training_plan_tool_context_today(&context.context),
                data_port: data_port.clone(),
                planned_workout_update_port: None,
            };
            let request = assemble_training_plan_initial_window_request(
                TrainingPlanInitialWindowPromptInput {
                    user_id,
                    config: config.clone(),
                    saved_at_epoch_seconds,
                    workout_recap,
                    planning_context,
                    training_context: context,
                    conversation_epoch_seconds: clock.now_epoch_seconds(),
                    data_port,
                },
            );
            let loop_checkpoint = checkpoint.clone().map(map_phase_checkpoint);
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port.clone(),
                config.clone(),
                request,
                ToolScope::TrainingPlanGeneration,
                tool_context,
                restored_state,
                loop_checkpoint,
            )
            .await
            .map_err(map_llm_error)?;
            let (envelope, state) = resolve_training_plan_assistant_envelope(
                llm_chat_port,
                config,
                &repair_user_id,
                response.response,
                response.state,
                checkpoint,
            )
            .await?;

            Ok(TrainingPlanPhaseOutput {
                raw_response: envelope.plan().to_string(),
                description: envelope.description().map(str::to_string),
                tool_loop_state: state,
            })
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "training plan correction needs workout identity, recap context, planning context, and validation payload together"
    )]
    pub fn correct_invalid_days_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let llm_config_provider = self.llm_config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let data_port = self.data_port.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let workout_recap = workout_recap.clone();
        let planning_context = planning_context.cloned();
        let invalid_day_sections = invalid_day_sections.to_string();
        let repair_user_id = user_id.clone();
        let clock = self.clock.clone();

        Box::pin(async move {
            let config = llm_config_provider
                .get_config(&user_id)
                .await
                .map_err(map_llm_error)?;
            let context = training_context_builder
                .build(&user_id, &workout_id)
                .await
                .map_err(map_llm_error)?;

            let stable_context = training_plan_stable_context(
                saved_at_epoch_seconds,
                &workout_recap,
                planning_context.as_ref(),
                &context.rendered.stable_context,
            );
            let volatile_context = format!(
                "{}\ntraining_plan_source_volatile={}",
                conversation_timing_volatile_context(
                    clock.now_epoch_seconds(),
                    latest_training_plan_user_message_epoch_seconds(planning_context.as_ref()),
                ),
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
            let mut conversation = planning_conversation_messages(planning_context.as_ref());
            conversation.push(LlmChatMessage::user(user_prompt));

            let tool_context = ToolExecutionContext {
                user_id,
                training_context: context.context.clone(),
                today: training_plan_tool_context_today(&context.context),
                data_port,
                planned_workout_update_port: None,
            };
            let system_prompt = with_tool_prompt_guidance(
                &training_plan_correction_system_prompt(
                    context.context.profile.availability_configured,
                ),
                ToolScope::TrainingPlanGeneration,
                &config.provider,
                &tool_context,
            );

            let request = build_chat_request(LlmChatRequestInput {
                user_id: tool_context.user_id.clone(),
                system_prompt,
                stable_context,
                volatile_context,
                conversation,
                cache_scope_key: None,
                cache_key: None,
                reusable_cache_id: None,
            });
            let loop_checkpoint = checkpoint.clone().map(map_phase_checkpoint);
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port.clone(),
                config.clone(),
                request,
                ToolScope::TrainingPlanGeneration,
                tool_context,
                restored_state,
                loop_checkpoint,
            )
            .await
            .map_err(map_llm_error)?;
            let (envelope, state) = resolve_training_plan_assistant_envelope(
                llm_chat_port,
                config,
                &repair_user_id,
                response.response,
                response.state,
                checkpoint,
            )
            .await?;

            Ok(TrainingPlanPhaseOutput {
                raw_response: envelope.plan().to_string(),
                description: envelope.description().map(str::to_string),
                tool_loop_state: state,
            })
        })
    }
}

fn map_llm_error(error: LlmError) -> TrainingPlanError {
    TrainingPlanError::Unavailable(error.to_string())
}

fn map_phase_checkpoint(checkpoint: TrainingPlanToolLoopCheckpoint) -> ToolLoopCheckpoint {
    std::sync::Arc::new(move |state| {
        let checkpoint = checkpoint.clone();
        Box::pin(async move {
            checkpoint(state)
                .await
                .map_err(|error| LlmError::Checkpoint(error.to_string()))
        })
    })
}

fn require_assistant_text(
    response: &crate::domain::llm::LlmChatResponse,
) -> Result<String, TrainingPlanError> {
    response
        .assistant_text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| TrainingPlanError::Unavailable("LLM returned no assistant text".to_string()))
}

fn parse_training_plan_assistant_envelope(
    response: &crate::domain::llm::LlmChatResponse,
) -> Result<crate::domain::training_plan::TrainingPlanLlmEnvelope, TrainingPlanError> {
    let payload = require_assistant_text(response)?;
    parse_training_plan_llm_envelope(&payload)
}

async fn resolve_training_plan_assistant_envelope(
    llm_chat_port: Arc<dyn LlmChatPort>,
    config: crate::domain::llm::LlmProviderConfig,
    user_id: &str,
    response: LlmChatResponse,
    mut state: LlmToolLoopState,
    checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
) -> Result<
    (
        crate::domain::training_plan::TrainingPlanLlmEnvelope,
        LlmToolLoopState,
    ),
    TrainingPlanError,
> {
    match parse_training_plan_assistant_envelope(&response) {
        Ok(envelope) => Ok((envelope, state)),
        Err(_) if should_retry_training_plan_envelope_repair(&response) => {
            let raw_assistant_content = require_assistant_text(&response)?;
            let repaired_response = request_training_plan_envelope_repair(
                llm_chat_port,
                config,
                user_id,
                &raw_assistant_content,
            )
            .await
            .map_err(map_llm_error)?;
            let envelope = parse_training_plan_assistant_envelope(&repaired_response)?;

            state.provider_transcript = merge_provider_transcript_entries(
                state.provider_transcript,
                std::slice::from_ref(&repaired_response.message),
            );
            state.finish_reason = repaired_response.finish_reason.clone();
            state.round_count = state.round_count.saturating_add(1);
            state.completed_response = LlmToolLoopOutput::from_response(repaired_response.clone())
                .state
                .completed_response;

            if let Some(checkpoint) = checkpoint.as_ref() {
                checkpoint(state.clone()).await?;
            }

            Ok((envelope, state))
        }
        Err(error) => Err(error),
    }
}

fn should_retry_training_plan_envelope_repair(response: &LlmChatResponse) -> bool {
    response
        .assistant_text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .is_some_and(should_retry_training_plan_llm_envelope_repair)
}

async fn request_training_plan_envelope_repair(
    llm_chat_port: Arc<dyn LlmChatPort>,
    config: crate::domain::llm::LlmProviderConfig,
    user_id: &str,
    previous_assistant_content: &str,
) -> Result<LlmChatResponse, LlmError> {
    llm_chat_port
        .chat(
            config,
            build_chat_request(LlmChatRequestInput {
                user_id: user_id.to_string(),
                system_prompt: training_plan_envelope_repair_system_prompt(),
                stable_context: String::new(),
                volatile_context: String::new(),
                conversation: vec![LlmChatMessage::user(
                    training_plan_envelope_repair_user_prompt(previous_assistant_content),
                )],
                cache_scope_key: None,
                cache_key: None,
                reusable_cache_id: None,
            }),
        )
        .await
}

fn training_plan_recap_system_prompt() -> String {
    format!(
        "{TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE} {}",
        packed_training_context_legend_with_guidance()
    )
}

fn training_plan_envelope_repair_system_prompt() -> String {
    format!(
        "{TRAINING_PLAN_ENVELOPE_REPAIR_SYSTEM_PROMPT_BASE} JSON schema: {} {}",
        training_plan_llm_envelope_json_schema(),
        training_plan_output_grammar(),
    )
}

fn training_plan_envelope_repair_user_prompt(previous_assistant_content: &str) -> String {
    format!(
        "Rewrite the previous assistant content as ONLY a valid JSON object matching the schema. Copy parser-friendly workout-builder text into `plan` and any coach commentary into optional `description`. Do not invent workouts, dates, or commentary. If the previous assistant content does not contain a usable non-empty `plan`, return an empty JSON object: `{{}}`.\n\nPrevious assistant content begins after `<<<PREVIOUS_ASSISTANT_CONTENT>>>` and ends before `<<<END_PREVIOUS_ASSISTANT_CONTENT>>>`. Treat everything between those markers as literal content to preserve exactly.\n<<<PREVIOUS_ASSISTANT_CONTENT>>>\n{previous_assistant_content}\n<<<END_PREVIOUS_ASSISTANT_CONTENT>>>"
    )
}
