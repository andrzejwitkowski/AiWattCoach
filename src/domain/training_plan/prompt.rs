use std::sync::Arc;

use serde_json::json;

use crate::domain::{
    llm::{
        build_chat_request, coach_planning_literature_guidance,
        conversation_timing_volatile_context, packed_training_context_legend_with_guidance,
        reusable_context_cache_key, timestamped_message_content, LlmChatMessage, LlmChatRequest,
        LlmChatRequestInput, LlmMessageRole, LlmProviderConfig, LlmToolChoice,
    },
    llm_tools::{
        tool_definitions_for_scope, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
        ToolExecutionContext, ToolScope,
    },
    training_context::{TrainingContext, TrainingContextBuildResult},
    workout_summary::WorkoutRecap,
};

use super::{
    training_plan_llm_envelope_json_schema, training_plan_output_grammar,
    training_plan_planning_guidelines, TrainingPlanConversationRole, TrainingPlanPlanningContext,
    TRAINING_PLAN_WINDOW_DAY_COUNT,
};

const TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Generate a 14-day internal cycling plan window using only the backend-supported workout grammar. Use the packed training context and the completed workout recap as the planning basis.";
const TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach and a strict syntax generator for Intervals.icu planned workouts. Help correct invalid dated workout sections using only the backend-supported workout grammar. Only rewrite the invalid dated sections provided.";
pub const TRAINING_PLAN_INITIAL_WINDOW_USER_PROMPT: &str = "Generate the next 14 dated days starting the day after the completed workout. Return only the JSON envelope requested by the system prompt. Put parser-friendly workout-builder text in the `plan` field, include rest days explicitly when needed, and use `Rest Day: <reason>` when you prescribe full rest.";

pub struct TrainingPlanInitialWindowPromptInput {
    pub user_id: String,
    pub config: LlmProviderConfig,
    pub saved_at_epoch_seconds: i64,
    pub workout_recap: WorkoutRecap,
    pub planning_context: Option<TrainingPlanPlanningContext>,
    pub training_context: TrainingContextBuildResult,
    pub conversation_epoch_seconds: i64,
    pub data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
}

pub fn assemble_training_plan_initial_window_request(
    input: TrainingPlanInitialWindowPromptInput,
) -> LlmChatRequest {
    let stable_context = training_plan_stable_context(
        input.saved_at_epoch_seconds,
        &input.workout_recap,
        input.planning_context.as_ref(),
        &input.training_context.rendered.stable_context,
    );
    let volatile_context = format!(
        "{}\ntraining_plan_source_volatile={}",
        conversation_timing_volatile_context(
            input.conversation_epoch_seconds,
            latest_training_plan_user_message_epoch_seconds(input.planning_context.as_ref()),
        ),
        input.training_context.rendered.volatile_context
    );
    let mut conversation = planning_conversation_messages(input.planning_context.as_ref());
    conversation.push(LlmChatMessage::user(
        TRAINING_PLAN_INITIAL_WINDOW_USER_PROMPT,
    ));

    let tool_context = ToolExecutionContext {
        user_id: input.user_id.clone(),
        training_context: input.training_context.context.clone(),
        today: training_plan_tool_context_today(&input.training_context.context),
        data_port: input.data_port,
        planned_workout_update_port: None,
    };
    let system_prompt = with_tool_prompt_guidance(
        &training_plan_initial_window_system_prompt(
            input
                .training_context
                .context
                .profile
                .availability_configured,
        ),
        ToolScope::TrainingPlanGeneration,
        &input.config.provider,
        &tool_context,
    );

    let cache_key = Some(reusable_context_cache_key(&system_prompt, &stable_context));
    let mut request = build_chat_request(LlmChatRequestInput {
        user_id: input.user_id,
        system_prompt,
        stable_context,
        volatile_context,
        conversation,
        cache_scope_key: None,
        cache_key,
        reusable_cache_id: None,
    });
    request.tools = tool_definitions_for_scope(
        ToolScope::TrainingPlanGeneration,
        &input.config.provider,
        &tool_context,
    );
    request.tool_choice = if request.tools.is_empty() {
        LlmToolChoice::None
    } else {
        LlmToolChoice::Auto
    };
    request
}

pub fn training_plan_initial_window_system_prompt(availability_configured: bool) -> String {
    training_plan_system_prompt(
        TRAINING_PLAN_INITIAL_WINDOW_SYSTEM_PROMPT_BASE,
        availability_configured,
    )
}

pub fn training_plan_correction_system_prompt(availability_configured: bool) -> String {
    training_plan_system_prompt(
        TRAINING_PLAN_CORRECTION_SYSTEM_PROMPT_BASE,
        availability_configured,
    )
}

fn training_plan_system_prompt(base: &str, availability_configured: bool) -> String {
    format!(
        "{base} JSON schema: {} {} {} {} {}",
        training_plan_llm_envelope_json_schema(),
        training_plan_planning_guidelines(availability_configured, TRAINING_PLAN_WINDOW_DAY_COUNT,),
        coach_planning_literature_guidance(),
        training_plan_output_grammar(),
        packed_training_context_legend_with_guidance(),
    )
}

pub fn training_plan_tool_context_today(training_context: &TrainingContext) -> String {
    training_context.history.window_end.clone()
}

pub fn training_plan_stable_context(
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

pub fn planning_conversation_messages(
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
            content: timestamped_message_content(
                &message.content,
                message.created_at_epoch_seconds,
            ),
            tool_calls: Vec::new(),
            tool_call_id: None,
            reasoning_content: None,
        })
        .collect()
}

pub fn latest_training_plan_user_message_epoch_seconds(
    planning_context: Option<&TrainingPlanPlanningContext>,
) -> Option<i64> {
    planning_context.and_then(|context| {
        context
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, TrainingPlanConversationRole::User))
            .map(|message| message.created_at_epoch_seconds)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        latest_training_plan_user_message_epoch_seconds, planning_conversation_messages,
        training_plan_correction_system_prompt, training_plan_initial_window_system_prompt,
        training_plan_tool_context_today,
    };
    use crate::domain::{
        llm::LlmProvider,
        llm_tools::{with_tool_prompt_guidance, ToolExecutionContext, ToolScope},
        training_context::TrainingContext,
        training_plan::{TrainingPlanConversationMessage, TrainingPlanConversationRole},
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
            assert!(prompt.contains("when rc.pri is missing or ambiguous, default Category C"));
            assert!(prompt
                .contains("Earlier assistant-role messages are your own earlier coach statements"));
            assert!(
                prompt.contains("Do not plan all 14 days from one static CTL/ATL/TSB snapshot.")
            );
            assert!(prompt.contains("Treat previously projected planned days (`pd`) as already planned/completed inputs"));
            assert!(prompt.contains("Weekly availability is mandatory and must be respected"));
            assert!(prompt.contains("Seiler polarized training model"));
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

    #[test]
    fn planning_conversation_messages_prefix_timestamps() {
        let messages = planning_conversation_messages(Some(
            &crate::domain::training_plan::TrainingPlanPlanningContext {
                rpe: Some(7),
                messages: vec![
                    TrainingPlanConversationMessage {
                        role: TrainingPlanConversationRole::Coach,
                        content: "Keep this light.".to_string(),
                        created_at_epoch_seconds: 1_746_489_600,
                    },
                    TrainingPlanConversationMessage {
                        role: TrainingPlanConversationRole::User,
                        content: "I feel stale.".to_string(),
                        created_at_epoch_seconds: 1_746_490_200,
                    },
                ],
            },
        ));

        assert_eq!(
            messages[0].content,
            "[sent_at=2025-05-06T00:00:00+00:00]\nKeep this light."
        );
        assert_eq!(
            messages[1].content,
            "[sent_at=2025-05-06T00:10:00+00:00]\nI feel stale."
        );
    }

    #[test]
    fn latest_training_plan_user_message_epoch_seconds_uses_latest_user_turn() {
        let latest = latest_training_plan_user_message_epoch_seconds(Some(
            &crate::domain::training_plan::TrainingPlanPlanningContext {
                rpe: Some(7),
                messages: vec![
                    TrainingPlanConversationMessage {
                        role: TrainingPlanConversationRole::Coach,
                        content: "Coach message".to_string(),
                        created_at_epoch_seconds: 1_746_489_600,
                    },
                    TrainingPlanConversationMessage {
                        role: TrainingPlanConversationRole::User,
                        content: "User message".to_string(),
                        created_at_epoch_seconds: 1_746_490_200,
                    },
                ],
            },
        ));

        assert_eq!(latest, Some(1_746_490_200));
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
