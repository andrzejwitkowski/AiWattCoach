use std::sync::Arc;

use crate::domain::llm::{
    build_chat_request, coach_planning_literature_guidance, conversation_timing_volatile_context,
    packed_training_context_legend_with_guidance, rebuild_conversation_with_provider_transcript,
    reusable_context_cache_key, timestamped_message_content, LlmChatMessage, LlmChatRequest,
    LlmChatRequestInput, LlmMessageRole, LlmProviderConfig, LlmToolChoice,
};
use crate::domain::llm_tools::{
    tool_definitions_for_scope, with_tool_prompt_guidance, GetSelectedWorkoutDataPort,
    ToolExecutionContext, ToolScope,
};
use crate::domain::training_context::TrainingContextBuildResult;

use super::{
    workout_summary_coach_reply_json_schema, ConversationMessage, MessageRole, WorkoutSummary,
};

pub const ADMIN_PREVIEW_USER_MESSAGE: &str = "Preview: [admin] sample athlete message";

const WORKOUT_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Use the provided training context as factual background. Be direct, adult, and concise. Do not flatter, hedge, or act like a yes-man. Challenge weak reasoning when the context does not support it. Keep the conversation focused and practical rather than digressive. In your first reply after a workout, ask all follow-up questions you genuinely need at once instead of stretching them across many turns. The athlete should still feel coached, not interrogated. Ask concrete questions about the workout limiter, legs, breathing, fueling, sleep, stress, pain, readiness for the next days, and any plan constraints when relevant. Add other questions only when the workout characteristics clearly justify them. You may also ask about nutrition, race strategy, or the desired direction of the next 14 days when that would materially improve the next plan. Plan-vs-actual interval execution: aligned_intervals (sa in context) is your primary evidence. Each entry gives planned_step (target_power_min/max, planned_duration_seconds, step_type) against actual execution (actual_duration_seconds, avg_power, normalized_power, avg_cadence, cadence_range) plus an anomalies array of unplanned drops during work steps. Your primary reference is planned_step; compare the athlete's actual metrics against those planned targets. Review the anomalies array for each interval: these represent unplanned drops (traffic, corners, mechanicals) during active work periods. If anomalies are present, expect avg_power to be lower than the target and DO NOT penalize the athlete for it; instead look at normalized_power. If normalized_power is within or above the planned target range despite the anomalies, declare the interval a physical success and explain that the physiological stimulus was preserved despite the road interruption. Use the anomaly type and duration_seconds for context (e.g. a 12s coasting_stop mid-block is a sharp corner or traffic). If step_type is recovery, ensure the athlete kept intensity low; do not flag anomalies during recovery steps. Aggregate metrics like NP, average power, IF, VI, and TSS are secondary context only and are not sufficient proof that interval blocks were or were not executed correctly. If aligned_intervals evidence is insufficient for a confident execution judgment, call get_selected_workout before making a strong claim. Never tell the athlete they have free time, vacation, or a rest block unless prd confirms it or pd/ud/pw show it; if the athlete challenges such a claim, cite the exact context field or admit it was unsupported. If you already have enough information to generate the plan, say that clearly and tell the athlete to save the summary. Return your final answer as JSON only matching the workout summary coach reply schema. The summary may use markdown. Questions may be an empty array when you are ready. Do not output any text outside the JSON object. Do not invent details beyond the provided context.";

pub const ATHLETE_SUMMARY_GUIDANCE: &str = "AI-generated athlete orientation only; NOT calendar truth. Never tell the athlete they have free time, vacation, or a rest block based on this text alone. For any schedule/rest/availability claim, verify packed prd, pd, ud, and pw first.";

const WORKOUT_COACH_SELECTED_WORKOUT_PROMPT: &str = "Use the provided selected workout date as the active workout context for this conversation. When inspecting the current workout, prefer that exact selected workout date or id instead of inferring from nearby history.";

const WORKOUT_COACH_PLANNING_LITERATURE_FRAMING: &str = "Use the scientific foundations below when reasoning about the next 14-day training direction, race-week load tradeoffs, and whether you already have enough information to tell the athlete to save the summary. Do not use this block to justify extra follow-up questions or to interrogate the athlete about physiology they already answered.";

const WORKOUT_COACH_RECENT_WORKOUT_RECAP_PROMPT: &str = "Saved workout summaries for recent sessions appear in packed context as wr (preferred) and optionally as recap on matching entries in rd. Treat each saved recap as already-known context for that workout. Do not ask the athlete again for information clearly stated in wr, recap, RPE, or earlier messages in the current workout thread. Ask follow-up questions only when a decision still depends on missing or ambiguous information.";

pub struct WorkoutSummaryCoachPromptInput {
    pub user_id: String,
    pub config: LlmProviderConfig,
    pub summary: WorkoutSummary,
    pub training_context: TrainingContextBuildResult,
    pub user_message: String,
    pub athlete_summary_text: Option<String>,
    pub conversation_epoch_seconds: i64,
    pub today: String,
    pub data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
    pub reusable_cache_id: Option<String>,
    pub meso_roadmap_stable_context: Option<String>,
    pub power_chart_base64: Option<String>,
}

pub fn assemble_workout_summary_coach_request(
    input: WorkoutSummaryCoachPromptInput,
) -> LlmChatRequest {
    let stable_context = build_stable_context(
        &input.summary,
        &input.training_context.focus_date,
        &input.training_context.rendered.stable_context,
        input.athlete_summary_text.as_deref(),
        input.meso_roadmap_stable_context.as_deref(),
    );
    let volatile_context = build_volatile_context(
        &input.training_context.rendered.volatile_context,
        input.conversation_epoch_seconds,
        latest_user_message_epoch_seconds(&input.summary, input.conversation_epoch_seconds),
    );
    let tool_context = ToolExecutionContext {
        user_id: input.user_id.clone(),
        training_context: input.training_context.context.clone(),
        today: input.today,
        data_port: input.data_port,
        planned_workout_update_port: None,
    };
    let system_prompt = with_tool_prompt_guidance(
        &workout_coach_system_prompt(),
        ToolScope::WorkoutSummaryChat,
        &input.config.provider,
        &tool_context,
    );
    let conversation = build_conversation(
        input.summary.messages.as_slice(),
        &input.summary.provider_transcript,
        &input.user_message,
        input.conversation_epoch_seconds,
        input.power_chart_base64.as_deref(),
    );
    let cache_scope_key = Some(format!(
        "workout-summary:{}:{}",
        input.user_id, input.summary.workout_id
    ));
    let cache_key = Some(reusable_context_cache_key(&system_prompt, &stable_context));
    let mut request = build_chat_request(LlmChatRequestInput {
        user_id: input.user_id,
        system_prompt,
        stable_context,
        volatile_context,
        conversation,
        cache_scope_key,
        cache_key,
        reusable_cache_id: input.reusable_cache_id,
    });
    apply_tool_scope(
        &mut request,
        ToolScope::WorkoutSummaryChat,
        &input.config,
        &tool_context,
    );
    request
}

fn apply_tool_scope(
    request: &mut LlmChatRequest,
    scope: ToolScope,
    config: &LlmProviderConfig,
    tool_context: &ToolExecutionContext,
) {
    request.tools = tool_definitions_for_scope(scope, &config.provider, tool_context);
    request.tool_choice = if request.tools.is_empty() {
        LlmToolChoice::None
    } else {
        LlmToolChoice::Auto
    };
}

pub fn workout_coach_system_prompt() -> String {
    format!(
        "{WORKOUT_COACH_SYSTEM_PROMPT_BASE}\n{WORKOUT_COACH_SELECTED_WORKOUT_PROMPT}\n{WORKOUT_COACH_RECENT_WORKOUT_RECAP_PROMPT}\nworkout_summary_coach_reply_schema={}\n{}\n{WORKOUT_COACH_PLANNING_LITERATURE_FRAMING}\n{}",
        workout_summary_coach_reply_json_schema(),
        packed_training_context_legend_with_guidance(),
        coach_planning_literature_guidance(),
    )
}

pub fn build_stable_context(
    summary: &WorkoutSummary,
    selected_workout_date: &str,
    packed_training_context: &str,
    athlete_summary_text: Option<&str>,
    meso_roadmap_stable_context: Option<&str>,
) -> String {
    let mut context = format!(
        "workout_summary={{\"workoutId\":\"{}\",\"rpe\":{}}}\nselected_workout={{\"workoutId\":\"{}\",\"date\":\"{}\"}}\ncurrent_workout_context=You are discussing the completed workout from {}.",
        summary.workout_id,
        summary
            .rpe
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        summary.workout_id,
        selected_workout_date,
        selected_workout_date,
    );

    if let Some(summary_text) = athlete_summary_text.filter(|value| !value.trim().is_empty()) {
        context.push_str(&format!(
            "\nathlete_summary_guidance={ATHLETE_SUMMARY_GUIDANCE}"
        ));
        context.push_str(&format!("\nathlete_summary_text={summary_text}"));
    }

    if let Some(recap) = summary
        .workout_recap_text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        context.push_str(&format!("\ncurrent_workout_recap={recap}"));
    }

    if let Some(meso_roadmap) = meso_roadmap_stable_context.filter(|value| !value.trim().is_empty())
    {
        context.push_str(&format!("\n{meso_roadmap}"));
    }

    context.push_str(&format!(
        "\ntraining_context_stable={packed_training_context}"
    ));
    context
}

pub fn build_volatile_context(
    packed_training_context: &str,
    current_conversation_epoch_seconds: i64,
    latest_user_message_epoch_seconds: Option<i64>,
) -> String {
    format!(
        "{}\ntraining_context_volatile={packed_training_context}",
        conversation_timing_volatile_context(
            current_conversation_epoch_seconds,
            latest_user_message_epoch_seconds,
        )
    )
}

pub fn build_conversation(
    messages: &[ConversationMessage],
    provider_transcript: &[LlmChatMessage],
    user_message: &str,
    fallback_user_message_epoch_seconds: i64,
    power_chart_base64: Option<&str>,
) -> Vec<LlmChatMessage> {
    let conversation = messages
        .iter()
        .filter_map(|message| match message.role {
            MessageRole::User => Some(LlmChatMessage {
                role: LlmMessageRole::User,
                content: timestamped_message_content(
                    &message.content,
                    message.created_at_epoch_seconds,
                ),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
                // ponytail: re-renders PNG each turn to keep it in LLM context; cache by workout if token cost bites
                image_base64: message
                    .image_url
                    .as_ref()
                    .and_then(|_| power_chart_base64.map(str::to_string)),
            }),
            MessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: timestamped_message_content(
                    &message.content,
                    message.created_at_epoch_seconds,
                ),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
                image_base64: None,
            }),
            MessageRole::Tool => None,
        })
        .collect::<Vec<_>>();

    let mut rebuilt =
        rebuild_conversation_with_provider_transcript(conversation, provider_transcript);

    if let Some(last) = rebuilt.last_mut() {
        if last.role == LlmMessageRole::User {
            last.content = timestamped_message_content(
                user_message,
                latest_user_message_epoch_seconds_in_messages(messages)
                    .unwrap_or(fallback_user_message_epoch_seconds),
            );
            return rebuilt;
        }
    }

    rebuilt.push(LlmChatMessage::user(timestamped_message_content(
        user_message,
        latest_user_message_epoch_seconds_in_messages(messages)
            .unwrap_or(fallback_user_message_epoch_seconds),
    )));

    rebuilt
}

fn latest_user_message_epoch_seconds(
    summary: &WorkoutSummary,
    fallback_epoch_seconds: i64,
) -> Option<i64> {
    latest_user_message_epoch_seconds_in_messages(&summary.messages)
        .or(Some(fallback_epoch_seconds))
}

fn latest_user_message_epoch_seconds_in_messages(messages: &[ConversationMessage]) -> Option<i64> {
    messages
        .iter()
        .rev()
        .find(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.created_at_epoch_seconds)
}
