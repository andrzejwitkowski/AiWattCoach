use std::sync::Arc;

use serde_json::json;

use super::{
    context_prelude::PACKED_TRAINING_CONTEXT_LEGEND,
    training_plan_prompt_grammar::TRAINING_PLAN_OUTPUT_GRAMMAR,
};

use crate::domain::{
    ai_workflow::ValidationIssue,
    identity::Clock,
    llm::{
        build_chat_request, BoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequestInput, LlmError,
        LlmMessageRole, UserLlmConfigProvider,
    },
    llm_tools::{
        run_tool_loop_with_checkpoint, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
        LlmToolLoopState, ToolExecutionContext, ToolLoopCheckpoint, ToolScope,
    },
    training_context::{TrainingContext, TrainingContextBuilder},
    training_plan::{
        TrainingPlanConversationRole, TrainingPlanError, TrainingPlanGenerator,
        TrainingPlanPhaseOutput, TrainingPlanPlanningContext, TrainingPlanToolLoopCheckpoint,
    },
    workout_summary::WorkoutRecap,
};

const TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach generating a completed workout recap from packed training context. Use only the provided context, stay factual, concise, and avoid inventing details.";
const TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Generate a 14-day internal cycling plan window using only the backend-supported workout grammar. Use the packed training context and the completed workout recap as the planning basis.";
const TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Help correct invalid dated workout sections using only the backend-supported workout grammar. Only rewrite the invalid dated sections provided.";
const TRAINING_PLAN_PLANNING_GUIDELINES_BASE: &str = "Planning guidelines: Follow a durability-first approach. Road cycling, especially masters racing, is stochastic; prioritize power repeatability and lactate clearance over pure steady-state aerobic work. Treat athlete age 45+, body-weight changes, and medications such as beta-blockers as fixed environmental constraints, not pathologies. Metric hierarchy: RPE over power over TSS/TSB over heart rate. If RPE stays low or moderate despite high fatigue metrics, trust recovery capacity and maintain load. Ignore heart rate for intensity pacing when beta-blockers are present. Never prescribe more than 2 consecutive Rest Day entries unless the athlete explicitly reports illness or injury. During build phases, TSB/Form may sit in the -15 to -25 range without forcing emergency rest. Prevent detraining by preferring Active Recovery or Z1 over total inactivity when extra recovery is needed. If the athlete reports fatigue or low freshness, first choose a short Z1 ride when availability allows a safe low-load session; prescribe Rest Day only when availability blocks even an easy ride or the context clearly supports full rest, and include a short concrete reason after `Rest Day:`. Plan beyond isolated days: shape the 14-day window as part of a coherent mesocycle with a clear phase progression, not a pile of disconnected sessions. Weekly load progression should be intentional. Treat races as Category C by default unless the context explicitly says otherwise. For Category C races, do not taper: treat the race like a high-intensity stochastic interval session, keep normal training load during race week, keep Tuesday and Wednesday interval sessions before a Sunday race when the context supports it, allow at most one light spinning or Rest Day on Friday or Saturday before the race, and schedule recovery or light endurance the day after the race before returning to structured intervals within 48 hours. When race time is materially earlier than normal training time, gradually shift key sessions toward the race start window to support circadian rhythm and heat adaptation.";
const TRAINING_PLAN_CONVERSATION_GUIDANCE: &str = "If earlier conversation messages are present, treat them as the exact conversation that led to this plan. Earlier assistant-role messages are your own earlier coach statements. If those earlier coach statements promised specific workouts, sequencing, or an easy/recovery/rest week structure, return a plan that stays consistent with those promises unless the packed training context clearly makes them unsafe or impossible. When you must override an earlier promise for safety, availability, or hard context constraints, stay as close as possible to the original intent and preserve any easy/recovery character of the block.";
const TRAINING_PLAN_FORWARD_LOAD_GUIDANCE: &str = "Forecast load sequentially before choosing each next day. Start from the current historical CTL, ATL, and TSB in the packed training context. Treat previously projected planned days (`pd`) as already planned/completed inputs when they exist, then simulate the effect of each newly planned workout before choosing the following day. Do not plan all 14 days from one static CTL/ATL/TSB snapshot. If the conversation or context says rest week, easy week, or recovery block, keep the forward simulation aligned with that low-load intent and avoid hard sessions unless they are truly necessary.";
const TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE: &str = "Weekly availability is mandatory and must be respected: only schedule workouts on weekdays marked available, keep unavailable days as Rest Day with a reason when full rest is intentional, and never exceed the configured max duration minutes for each available weekday.";
const TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE: &str = "Weekly availability is not configured in this context. Do not infer unavailable days or extra rest constraints from missing availability data. Plan a sensible 14-day cycling window from the training context alone, and avoid claiming that weekly availability is configured.";

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
                "training_plan_source_volatile={}",
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
        self.generate_initial_plan_window_with_state(
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
        self.correct_invalid_days_with_state(
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
                "training_plan_source_volatile={}",
                context.rendered.volatile_context
            );
            let user_prompt = "Generate the next 14 dated days starting the day after the completed workout. Return only dated sections in parser-friendly workout-builder text. Include rest days explicitly when needed, and use `Rest Day: <reason>` when you prescribe full rest.";
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
                &training_plan_initial_window_system_prompt(
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
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port,
                config,
                request,
                ToolScope::TrainingPlanGeneration,
                tool_context,
                restored_state,
                checkpoint.map(map_phase_checkpoint),
            )
            .await
            .map_err(map_llm_error)?;

            Ok(TrainingPlanPhaseOutput {
                raw_response: require_assistant_text(&response.response)?,
                tool_loop_state: response.state,
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
            let response = run_tool_loop_with_checkpoint(
                llm_chat_port,
                config,
                request,
                ToolScope::TrainingPlanGeneration,
                tool_context,
                restored_state,
                checkpoint.map(map_phase_checkpoint),
            )
            .await
            .map_err(map_llm_error)?;

            Ok(TrainingPlanPhaseOutput {
                raw_response: require_assistant_text(&response.response)?,
                tool_loop_state: response.state,
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

fn training_plan_recap_system_prompt() -> String {
    format!("{TRAINING_PLAN_RECAP_SYSTEM_PROMPT_BASE} {PACKED_TRAINING_CONTEXT_LEGEND}")
}

fn training_plan_initial_window_system_prompt(availability_configured: bool) -> String {
    format!(
        "{TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE} {} {TRAINING_PLAN_OUTPUT_GRAMMAR} {PACKED_TRAINING_CONTEXT_LEGEND}",
        training_plan_planning_guidelines(availability_configured),
    )
}

fn training_plan_correction_system_prompt(availability_configured: bool) -> String {
    format!(
        "{TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE} {} {TRAINING_PLAN_OUTPUT_GRAMMAR} {PACKED_TRAINING_CONTEXT_LEGEND}",
        training_plan_planning_guidelines(availability_configured),
    )
}

fn training_plan_planning_guidelines(availability_configured: bool) -> String {
    let availability_guidance = if availability_configured {
        TRAINING_PLAN_AVAILABILITY_CONFIGURED_GUIDANCE
    } else {
        TRAINING_PLAN_AVAILABILITY_UNCONFIGURED_GUIDANCE
    };

    format!(
        "{TRAINING_PLAN_PLANNING_GUIDELINES_BASE} {TRAINING_PLAN_CONVERSATION_GUIDANCE} {TRAINING_PLAN_FORWARD_LOAD_GUIDANCE} {availability_guidance}"
    )
}

/// Uses the current planning window end as the synthetic "today" anchor for
/// both initial generation and correction. Callers should rebuild the training
/// context before correction if they want that planning anchor refreshed.
fn training_plan_tool_context_today(training_context: &TrainingContext) -> String {
    training_context.history.window_end.clone()
}

fn training_plan_stable_context(
    saved_at_epoch_seconds: i64,
    workout_recap: &WorkoutRecap,
    planning_context: Option<&TrainingPlanPlanningContext>,
    packed_training_context: &str,
) -> String {
    let workout_recap_json = json!({
        "text": workout_recap.text,
        "provider": workout_recap.provider,
        "model": workout_recap.model,
        "generatedAt": workout_recap.generated_at_epoch_seconds,
    })
    .to_string();
    let mut stable_context = format!(
        "saved_at_epoch_seconds={saved_at_epoch_seconds}\nworkout_recap={workout_recap_json}"
    );

    if let Some(planning_rpe) = planning_context.and_then(|context| context.rpe) {
        stable_context.push_str(&format!("\nplanning_rpe={planning_rpe}"));
    }

    stable_context.push_str(&format!(
        "\ntraining_plan_source_stable={packed_training_context}"
    ));
    stable_context
}

fn planning_conversation_messages(
    planning_context: Option<&TrainingPlanPlanningContext>,
) -> Vec<LlmChatMessage> {
    planning_context
        .into_iter()
        .flat_map(|planning_context| planning_context.messages.iter())
        .map(|message| LlmChatMessage {
            role: match message.role {
                TrainingPlanConversationRole::Coach => LlmMessageRole::Assistant,
                TrainingPlanConversationRole::User => LlmMessageRole::User,
            },
            content: message.content.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            reasoning_content: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        training_plan_correction_system_prompt, training_plan_initial_window_system_prompt,
        training_plan_tool_context_today,
    };
    use crate::domain::{
        llm::LlmProvider,
        llm_tools::{with_tool_prompt_guidance, ToolExecutionContext, ToolScope},
        training_context::TrainingContext,
    };

    #[test]
    fn training_plan_prompts_include_durability_guidelines() {
        for prompt in [
            training_plan_initial_window_system_prompt(true),
            training_plan_correction_system_prompt(true),
        ] {
            assert!(
                prompt.contains("Metric hierarchy: RPE over power over TSS/TSB over heart rate.")
            );
            assert!(prompt.contains("Never prescribe more than 2 consecutive Rest Day entries unless the athlete explicitly reports illness or injury."));
            assert!(prompt.contains(
                "first choose a short Z1 ride when availability allows a safe low-load session"
            ));
            assert!(prompt.contains("include a short concrete reason after `Rest Day:`"));
            assert!(prompt.contains("part of a coherent mesocycle with a clear phase progression"));
            assert!(prompt.contains("Treat races as Category C by default unless the context explicitly says otherwise."));
            assert!(prompt
                .contains("Earlier assistant-role messages are your own earlier coach statements"));
            assert!(
                prompt.contains("Do not plan all 14 days from one static CTL/ATL/TSB snapshot.")
            );
            assert!(prompt.contains("Treat previously projected planned days (`pd`) as already planned/completed inputs"));
            assert!(prompt.contains("Weekly availability is mandatory and must be respected"));
        }
    }

    #[test]
    fn training_plan_prompts_adjust_availability_guidance_when_not_configured() {
        for prompt in [
            training_plan_initial_window_system_prompt(false),
            training_plan_correction_system_prompt(false),
        ] {
            assert!(prompt.contains("Weekly availability is not configured in this context."));
            assert!(!prompt.contains("Weekly availability is mandatory and must be respected"));
        }
    }

    #[test]
    fn training_plan_prompt_guidance_includes_forward_load_tool() {
        let prompt = with_tool_prompt_guidance(
            &training_plan_initial_window_system_prompt(true),
            ToolScope::TrainingPlanGeneration,
            &LlmProvider::OpenAi,
            &sample_tool_context(),
        );

        assert!(prompt.contains("`simulate_forward_load`"));
        assert!(prompt.contains("future fatigue"));
        assert!(!prompt.contains("`get_selected_workout`"));
        assert!(!prompt.contains("`selected_workout_power_curve`"));
    }

    #[test]
    fn training_plan_tool_context_today_uses_focus_window_end() {
        let today = training_plan_tool_context_today(&TrainingContext {
            history: crate::domain::training_context::HistoricalTrainingContext {
                window_end: "2026-05-01".to_string(),
                ..Default::default()
            },
            ..TrainingContext::default()
        });

        assert_eq!(today, "2026-05-01");
    }

    fn sample_tool_context() -> ToolExecutionContext {
        ToolExecutionContext {
            user_id: "user-1".to_string(),
            training_context: TrainingContext {
                focus_kind: "training_plan".to_string(),
                history: crate::domain::training_context::HistoricalTrainingContext {
                    window_end: "2026-05-06".to_string(),
                    ..Default::default()
                },
                ..TrainingContext::default()
            },
            today: "2026-05-06".to_string(),
            data_port: None,
            planned_workout_update_port: None,
        }
    }
}
