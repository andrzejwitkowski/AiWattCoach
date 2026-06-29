use std::sync::Arc;

use crate::domain::llm::{
    build_chat_request, conversation_timing_volatile_context,
    packed_training_context_legend_with_guidance, reusable_context_cache_key, LlmChatMessage,
    LlmChatRequest, LlmChatRequestInput, LlmProviderConfig, LlmToolChoice,
};
use crate::domain::llm_tools::{
    tool_definitions_for_scope, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
    ToolExecutionContext, ToolScope,
};
use crate::domain::training_context::TrainingContextBuildResult;
use crate::domain::training_plan::{
    training_plan_llm_envelope_json_schema, training_plan_output_grammar,
    training_plan_planning_guidelines,
};

use super::{MesoCycleWindow, MESO_CYCLE_WINDOW_DAY_COUNT};

const MESO_CYCLE_SYSTEM_PROMPT_BASE: &str = "You are an expert cycling coach generating a preliminary 30-day mesocycle plan. Use packed training context and athlete constraints. Plan only the requested dated window. Do not modify the athlete's existing AI coach 14-day window. This meso plan is strategic guidance for the athlete to review on a separate calendar.";

pub struct MesoCycleCoachPromptInput {
    pub user_id: String,
    pub config: LlmProviderConfig,
    pub window: MesoCycleWindow,
    pub training_context: TrainingContextBuildResult,
    pub conversation_epoch_seconds: i64,
    pub today: String,
    pub data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
}

pub struct MesoCycleCoachPromptBundle {
    pub request: LlmChatRequest,
    pub tool_context: ToolExecutionContext,
}

pub fn assemble_meso_cycle_coach_request(
    input: MesoCycleCoachPromptInput,
) -> MesoCycleCoachPromptBundle {
    let stable_context = format!(
        "meso_cycle_window_start={}\nmeso_cycle_window_end={}\nmeso_cycle_source_stable={}",
        input.window.meso_start,
        input.window.meso_end,
        input.training_context.rendered.stable_context
    );
    let volatile_context = format!(
        "{}\nmeso_cycle_source_volatile={}",
        conversation_timing_volatile_context(input.conversation_epoch_seconds, None),
        input.training_context.rendered.volatile_context
    );
    let user_prompt = format!(
        "Generate exactly {MESO_CYCLE_WINDOW_DAY_COUNT} dated training days from {} through {} inclusive. Return only the JSON envelope requested by the system prompt. Put parser-friendly workout-builder text in the `plan` field, include rest days explicitly when needed, use `Rest Day: <reason>` when you prescribe full rest, and name every workout day on the first line after the date before any `-` steps.",
        input.window.meso_start, input.window.meso_end
    );
    let tool_context = ToolExecutionContext {
        user_id: input.user_id.clone(),
        training_context: input.training_context.context.clone(),
        today: input.today,
        data_port: input.data_port,
        planned_workout_update_port: None,
    };
    let system_prompt = with_tool_prompt_guidance(
        &meso_cycle_system_prompt(
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
        conversation: vec![LlmChatMessage::user(user_prompt)],
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
    MesoCycleCoachPromptBundle {
        request,
        tool_context,
    }
}

pub fn meso_cycle_system_prompt(availability_configured: bool) -> String {
    format!(
        "{MESO_CYCLE_SYSTEM_PROMPT_BASE} JSON schema: {} {} {} {}",
        training_plan_llm_envelope_json_schema(),
        training_plan_planning_guidelines(availability_configured, MESO_CYCLE_WINDOW_DAY_COUNT),
        training_plan_output_grammar(),
        packed_training_context_legend_with_guidance()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        llm::{LlmProvider, LlmProviderConfig},
        training_context::{RenderedTrainingContext, TrainingContext, TrainingContextBuildResult},
    };

    #[test]
    fn meso_prompt_bundle_includes_window_and_packed_context() {
        let bundle = assemble_meso_cycle_coach_request(MesoCycleCoachPromptInput {
            user_id: "user-1".to_string(),
            config: LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-test".to_string(),
                api_key: "secret".to_string(),
            },
            window: MesoCycleWindow {
                meso_start: "2026-06-06".to_string(),
                meso_end: "2026-07-05".to_string(),
                ai_coach_last_date: Some("2026-06-05".to_string()),
                source_training_plan_operation_key: None,
            },
            training_context: TrainingContextBuildResult {
                context: TrainingContext::default(),
                focus_date: "2026-06-05".to_string(),
                rendered: RenderedTrainingContext {
                    stable_context: r#"{"pd":[]}"#.to_string(),
                    volatile_context: r#"{"ctl":42}"#.to_string(),
                    approximate_tokens: 0,
                },
            },
            conversation_epoch_seconds: 1_700_000_000,
            today: "2026-06-05".to_string(),
            data_port: None,
        });

        assert!(bundle
            .request
            .stable_context
            .contains("meso_cycle_window_start=2026-06-06"));
        assert!(bundle
            .request
            .stable_context
            .contains("meso_cycle_source_stable="));
        assert!(bundle
            .request
            .volatile_context
            .contains("meso_cycle_source_volatile="));
        assert!(bundle
            .request
            .conversation
            .first()
            .is_some_and(|message| message.content.contains("2026-06-06")));
        assert!(bundle
            .request
            .system_prompt
            .contains("30-day mesocycle plan"));
    }
}
