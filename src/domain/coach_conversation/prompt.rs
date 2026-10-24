use std::sync::Arc;

use crate::domain::llm::{
    build_chat_request, LlmChatRequest, LlmChatRequestInput, LlmProviderConfig, LlmToolChoice,
};
use crate::domain::llm_tools::{
    tool_definitions_for_scope, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
    ToolExecutionContext, ToolScope,
};
use crate::domain::training_context::TrainingContextBuildResult;

use super::{
    service::transcript::{
        build_calendar_conversation, build_calendar_stable_context,
        build_calendar_volatile_context, calendar_coach_system_prompt,
    },
    CoachConversation, CoachConversationMessage,
};

pub struct CalendarCoachPromptInput {
    pub user_id: String,
    pub config: LlmProviderConfig,
    pub conversation: CoachConversation,
    pub messages: Vec<CoachConversationMessage>,
    pub training_context: TrainingContextBuildResult,
    pub preview_message_id: String,
    pub conversation_epoch_seconds: i64,
    pub latest_user_message_epoch_seconds: Option<i64>,
    pub today: String,
    pub data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
    pub planned_workout_update_port:
        Option<Arc<dyn crate::domain::llm_tools::UpdatePlannedWorkoutDataPort>>,
}

pub fn assemble_calendar_coach_request(input: CalendarCoachPromptInput) -> LlmChatRequest {
    let tool_context = ToolExecutionContext {
        user_id: input.user_id.clone(),
        training_context: input.training_context.context.clone(),
        today: input.today,
        data_port: input.data_port,
        planned_workout_update_port: input.planned_workout_update_port,
    };
    let system_prompt = with_tool_prompt_guidance(
        &calendar_coach_system_prompt(),
        ToolScope::CalendarCoachChat,
        &input.config.provider,
        &tool_context,
    );
    let stable_context = build_calendar_stable_context(
        &input.conversation,
        &input.training_context.rendered.stable_context,
    );
    let volatile_context = build_calendar_volatile_context(
        &input.conversation,
        &input.training_context.rendered.volatile_context,
        input.conversation_epoch_seconds,
        input.latest_user_message_epoch_seconds,
    );
    let conversation = build_calendar_conversation(
        input.messages.as_slice(),
        &input.conversation.provider_transcript,
        &input.preview_message_id,
        input.training_context.pack_mode.is_lean(),
    );
    let cache_scope_key = Some(format!(
        "calendar-coach:{}:{}",
        input.conversation.user_id,
        input.conversation.focus.cache_scope_suffix()
    ));
    let mut request = build_chat_request(LlmChatRequestInput {
        user_id: input.user_id,
        system_prompt,
        stable_context,
        volatile_context,
        conversation,
        cache_scope_key,
        cache_key: None,
        reusable_cache_id: None,
    });
    request.tools = tool_definitions_for_scope(
        ToolScope::CalendarCoachChat,
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
