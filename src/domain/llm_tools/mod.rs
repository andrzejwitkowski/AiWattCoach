use std::{future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::domain::{
    llm::{
        llm_full_debug_logging_enabled, merge_provider_transcript_entries, truncate_logged_body,
        LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmError, LlmFinishReason,
        LlmProvider, LlmProviderConfig, LlmToolChoice, LlmToolDefinition,
    },
    training_context::TrainingContext,
    workout_summary::PublicToolCall,
};

pub const TOOL_LOOP_MAX_ROUNDS: u32 = 6;

mod simulate_forward_load;
pub use simulate_forward_load::SimulateForwardLoad;

mod get_selected_workout;
pub use get_selected_workout::{GetSelectedWorkout, GetSelectedWorkoutDataPort};

mod selected_workout_power_curve;
pub use selected_workout_power_curve::SelectedWorkoutPowerCurve;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolScope {
    WorkoutSummaryChat,
    CalendarCoachChat,
    TrainingPlanGeneration,
}

/// Common interface for all LLM-callable tools.
///
/// To add a new tool:
/// 1. Create a struct (e.g. `struct MyTool`) and implement this trait.
/// 2. Register it in `tools_for_scope` so the appropriate scopes expose it.
/// 3. (Optional) Re-export the struct from `mod.rs`.
pub trait LlmTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn definition(&self) -> LlmToolDefinition;
    fn prompt_guidance(&self) -> Option<&'static str> {
        None
    }
    fn execute(
        &self,
        arguments_json: &str,
        context: &ToolExecutionContext,
    ) -> Pin<Box<dyn Future<Output = String> + Send>>;
    fn preview_arguments(&self, arguments_json: &str) -> Option<String>;
    fn is_available(&self, _context: &ToolExecutionContext) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmToolLoopState {
    pub provider_transcript: Vec<LlmChatMessage>,
    pub finish_reason: Option<LlmFinishReason>,
    pub public_tool_calls: Vec<PublicToolCall>,
    pub round_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmToolLoopOutput {
    pub response: LlmChatResponse,
    pub state: LlmToolLoopState,
}

impl LlmToolLoopOutput {
    pub fn from_response(response: LlmChatResponse) -> Self {
        let message = response.message.clone();
        let finish_reason = response.finish_reason.clone();

        Self {
            response,
            state: LlmToolLoopState {
                provider_transcript: vec![message],
                finish_reason,
                public_tool_calls: Vec::new(),
                round_count: 1,
            },
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub user_id: String,
    pub training_context: TrainingContext,
    pub today: String,
    pub data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
}

impl std::fmt::Debug for ToolExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutionContext")
            .field("user_id", &redacted_user_id(&self.user_id))
            .field("training_context", &self.training_context.redacted_debug())
            .field("today", &self.today)
            .field("data_port", &self.data_port.is_some())
            .finish()
    }
}

fn redacted_user_id(user_id: &str) -> String {
    let _ = user_id;
    "[redacted]".to_string()
}

type BoxToolFuture = Pin<Box<dyn Future<Output = Result<LlmToolLoopOutput, LlmError>> + Send>>;

/// Future returned by a tool-loop checkpoint callback.
pub type ToolLoopCheckpointFuture = Pin<Box<dyn Future<Output = Result<(), LlmError>> + Send>>;

/// Callback invoked after each tool round in `run_tool_loop_with_checkpoint`.
/// Receives the current loop state so callers can persist resumable progress.
pub type ToolLoopCheckpoint =
    Arc<dyn Fn(LlmToolLoopState) -> ToolLoopCheckpointFuture + Send + Sync>;

/// Run a tool loop without per-round checkpointing.
/// Equivalent to `run_tool_loop_with_checkpoint(..., None)`.
pub fn run_tool_loop(
    chat_port: std::sync::Arc<dyn LlmChatPort>,
    config: LlmProviderConfig,
    request: LlmChatRequest,
    scope: ToolScope,
    tool_context: ToolExecutionContext,
    restored_state: Option<LlmToolLoopState>,
) -> BoxToolFuture {
    run_tool_loop_with_checkpoint(
        chat_port,
        config,
        request,
        scope,
        tool_context,
        restored_state,
        None,
    )
}

/// Run a tool loop with optional per-round checkpointing.
///
/// If `checkpoint` is provided, it is invoked after each tool round (after tool
/// results have been appended to the conversation) so callers can persist
/// resumable state. If the checkpoint returns an error the loop aborts and
/// returns that error immediately.
pub fn run_tool_loop_with_checkpoint(
    chat_port: std::sync::Arc<dyn LlmChatPort>,
    config: LlmProviderConfig,
    mut request: LlmChatRequest,
    scope: ToolScope,
    tool_context: ToolExecutionContext,
    restored_state: Option<LlmToolLoopState>,
    checkpoint: Option<ToolLoopCheckpoint>,
) -> BoxToolFuture {
    Box::pin(async move {
        let mut conversation = request.conversation;
        let mut state = restored_state.unwrap_or_default();
        if !state.provider_transcript.is_empty() {
            conversation.extend(state.provider_transcript.clone());
        }

        let available_tools = available_tools_for_scope(scope, &config.provider, &tool_context);
        let tools = available_tools
            .iter()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>();
        let tool_choice = if tools.is_empty() {
            LlmToolChoice::None
        } else {
            LlmToolChoice::Auto
        };

        tracing::info!(
            provider = %config.provider,
            model = %config.model,
            scope = %tool_scope_name(scope),
            restored_round_count = state.round_count,
            restored_provider_transcript_messages = state.provider_transcript.len(),
            available_tool_names = ?available_tools.iter().map(|tool| tool.name()).collect::<Vec<_>>(),
            full_debug_logging = llm_full_debug_logging_enabled(),
            "starting llm tool loop"
        );

        for _ in state.round_count..TOOL_LOOP_MAX_ROUNDS {
            let round = state.round_count.saturating_add(1);
            request.conversation = conversation.clone();
            request.tools = tools.clone();
            request.tool_choice = tool_choice.clone();

            tracing::info!(
                provider = %config.provider,
                model = %config.model,
                scope = %tool_scope_name(scope),
                round,
                conversation_messages = request.conversation.len(),
                provider_transcript_messages = state.provider_transcript.len(),
                tool_count = request.tools.len(),
                tool_choice = %tool_choice_name(&request.tool_choice),
                conversation = %logged_conversation(&request.conversation),
                "sending llm tool loop round"
            );

            let response = chat_port.chat(config.clone(), request.clone()).await?;
            tracing::info!(
                provider = %response.provider,
                model = %response.model,
                scope = %tool_scope_name(scope),
                round,
                finish_reason = ?response.finish_reason,
                tool_call_count = response.tool_calls().len(),
                assistant_message = %logged_message(&response.message),
                usage = ?response.usage,
                cache = ?response.cache,
                "received llm tool loop response"
            );
            let new_public_tool_calls = response
                .tool_calls()
                .iter()
                .map(public_tool_call_from_llm)
                .collect::<Vec<_>>();

            let mut provider_transcript = merge_provider_transcript_entries(
                state.provider_transcript.clone(),
                std::slice::from_ref(&response.message),
            );
            let public_tool_calls =
                merge_public_tool_calls(state.public_tool_calls.clone(), &new_public_tool_calls);
            let round_count = state.round_count.saturating_add(1);

            conversation.push(response.message.clone());

            if response.tool_calls().is_empty() {
                tracing::info!(
                    provider = %response.provider,
                    model = %response.model,
                    scope = %tool_scope_name(scope),
                    round,
                    finish_reason = ?response.finish_reason,
                    final_assistant_message = %logged_message(&response.message),
                    "llm tool loop finished without tool calls"
                );
                return Ok(LlmToolLoopOutput {
                    response: response.clone(),
                    state: LlmToolLoopState {
                        provider_transcript,
                        finish_reason: response.finish_reason,
                        public_tool_calls,
                        round_count,
                    },
                });
            }

            for tool_call in response.tool_calls() {
                tracing::info!(
                    provider = %response.provider,
                    model = %response.model,
                    scope = %tool_scope_name(scope),
                    round,
                    tool_call_id = %tool_call.id,
                    tool_name = %tool_call.name,
                    arguments_json = %truncate_logged_body(&tool_call.arguments_json),
                    arguments_preview = ?find_tool(&tool_call.name)
                        .and_then(|tool| tool.preview_arguments(&tool_call.arguments_json)),
                    "executing llm tool call"
                );
                let execution_result = execute_available_tool_call(
                    available_tools.as_slice(),
                    tool_call.name.as_str(),
                    tool_call.arguments_json.as_str(),
                    &tool_context,
                )
                .await;
                let (result, tool_unavailable) = match execution_result {
                    ToolExecutionResult::Success(result) => (result, false),
                    ToolExecutionResult::ToolUnavailable(result) => (result, true),
                };
                let tool_message = LlmChatMessage::tool(tool_call.id.clone(), result);
                tracing::info!(
                    provider = %response.provider,
                    model = %response.model,
                    scope = %tool_scope_name(scope),
                    round,
                    tool_call_id = %tool_call.id,
                    tool_name = %tool_call.name,
                    tool_unavailable,
                    tool_result = %truncate_logged_body(&tool_message.content),
                    "completed llm tool call"
                );
                provider_transcript.push(tool_message.clone());
                conversation.push(tool_message);

                if tool_unavailable {
                    tracing::warn!(
                        provider = %response.provider,
                        model = %response.model,
                        scope = %tool_scope_name(scope),
                        round,
                        tool_call_id = %tool_call.id,
                        tool_name = %tool_call.name,
                        "llm tool loop stopped because requested tool was unavailable"
                    );
                    return Ok(LlmToolLoopOutput {
                        response: response.clone(),
                        state: LlmToolLoopState {
                            provider_transcript,
                            finish_reason: response.finish_reason.clone(),
                            public_tool_calls,
                            round_count,
                        },
                    });
                }
            }

            state = LlmToolLoopState {
                provider_transcript,
                finish_reason: response.finish_reason.clone(),
                public_tool_calls,
                round_count,
            };

            if let Some(checkpoint) = checkpoint.as_ref() {
                checkpoint(state.clone()).await?;
            }
        }

        tracing::warn!(
            provider = %config.provider,
            model = %config.model,
            scope = %tool_scope_name(scope),
            max_rounds = TOOL_LOOP_MAX_ROUNDS,
            "llm tool loop exceeded max rounds"
        );

        Err(LlmError::InvalidResponse(format!(
            "tool loop exceeded {TOOL_LOOP_MAX_ROUNDS} rounds"
        )))
    })
}

/// Single registry of all tools.  Adding a tool is a one-line change here.
fn all_tools() -> Vec<Box<dyn LlmTool>> {
    vec![
        Box::new(SimulateForwardLoad),
        Box::new(GetSelectedWorkout),
        Box::new(SelectedWorkoutPowerCurve),
    ]
}

/// Resolve a tool by its declared name.
fn find_tool(name: &str) -> Option<Box<dyn LlmTool>> {
    all_tools().into_iter().find(|tool| tool.name() == name)
}

fn tool_scope_name(scope: ToolScope) -> &'static str {
    match scope {
        ToolScope::WorkoutSummaryChat => "workout_summary_chat",
        ToolScope::CalendarCoachChat => "calendar_coach_chat",
        ToolScope::TrainingPlanGeneration => "training_plan_generation",
    }
}

fn tool_choice_name(choice: &LlmToolChoice) -> &'static str {
    match choice {
        LlmToolChoice::None => "none",
        LlmToolChoice::Auto => "auto",
        LlmToolChoice::Required => "required",
        LlmToolChoice::Named(_) => "named",
    }
}

fn logged_conversation(conversation: &[LlmChatMessage]) -> String {
    if llm_full_debug_logging_enabled() {
        truncate_logged_body(
            &serde_json::to_string(conversation)
                .unwrap_or_else(|error| format!("(conversation serialization failed: {error})")),
        )
    } else {
        format!("{} messages", conversation.len())
    }
}

fn logged_message(message: &LlmChatMessage) -> String {
    if llm_full_debug_logging_enabled() {
        truncate_logged_body(
            &serde_json::to_string(message)
                .unwrap_or_else(|error| format!("(message serialization failed: {error})")),
        )
    } else {
        format!(
            "role={:?} content_chars={} tool_calls={} tool_call_id_present={}",
            message.role,
            message.content.chars().count(),
            message.tool_calls.len(),
            message.tool_call_id.is_some()
        )
    }
}

/// Tools exposed for a given scope.  Filters the global registry.
fn tools_for_scope(scope: ToolScope) -> Vec<Box<dyn LlmTool>> {
    // When we have tools that are scope-specific, filter here.
    // Currently all tools are available in every tool-enabled scope.
    let _ = scope;
    all_tools()
}

fn available_tools_for_scope(
    scope: ToolScope,
    provider: &LlmProvider,
    tool_context: &ToolExecutionContext,
) -> Vec<Box<dyn LlmTool>> {
    if !provider_supports_tools(provider) {
        return Vec::new();
    }

    tools_for_scope(scope)
        .into_iter()
        .filter(|tool| tool.is_available(tool_context))
        .collect()
}

pub fn tool_definitions_for_scope(
    scope: ToolScope,
    provider: &LlmProvider,
    tool_context: &ToolExecutionContext,
) -> Vec<LlmToolDefinition> {
    available_tools_for_scope(scope, provider, tool_context)
        .into_iter()
        .map(|tool| tool.definition())
        .collect()
}

pub fn with_tool_prompt_guidance(
    system_prompt: &str,
    scope: ToolScope,
    provider: &LlmProvider,
    tool_context: &ToolExecutionContext,
) -> String {
    let guidance = tool_prompt_guidance_for_scope(scope, provider, tool_context);
    if guidance.is_empty() {
        return system_prompt.to_string();
    }

    if system_prompt.trim().is_empty() {
        return guidance;
    }

    format!("{system_prompt}\n\n{guidance}")
}

fn tool_prompt_guidance_for_scope(
    scope: ToolScope,
    provider: &LlmProvider,
    tool_context: &ToolExecutionContext,
) -> String {
    let guidance_lines: Vec<String> = available_tools_for_scope(scope, provider, tool_context)
        .into_iter()
        .filter_map(|tool| {
            tool.prompt_guidance()
                .map(|guidance| format!("- `{}`: {guidance}", tool.name()))
        })
        .collect();

    if guidance_lines.is_empty() {
        return String::new();
    }

    format!(
        "Tool usage guidance: when a tool can provide more specific or up-to-date facts than the packed context, call it instead of guessing. Use these tools deliberately:\n{}",
        guidance_lines.join("\n")
    )
}

fn provider_supports_tools(provider: &LlmProvider) -> bool {
    matches!(provider, LlmProvider::OpenAi | LlmProvider::OpenRouter)
}

pub fn public_tool_call_from_llm(tool_call: &crate::domain::llm::LlmToolCall) -> PublicToolCall {
    PublicToolCall {
        id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        arguments_json: tool_call.arguments_json.clone(),
        arguments_preview: find_tool(&tool_call.name)
            .and_then(|tool| tool.preview_arguments(&tool_call.arguments_json)),
    }
}

fn merge_public_tool_calls(
    mut existing: Vec<PublicToolCall>,
    pending: &[PublicToolCall],
) -> Vec<PublicToolCall> {
    for tool_call in pending {
        if existing
            .iter()
            .any(|existing_call| existing_call.id == tool_call.id)
        {
            continue;
        }
        existing.push(tool_call.clone());
    }

    existing
}

enum ToolExecutionResult {
    Success(String),
    ToolUnavailable(String),
}

async fn execute_available_tool_call(
    available_tools: &[Box<dyn LlmTool>],
    tool_name: &str,
    arguments_json: &str,
    context: &ToolExecutionContext,
) -> ToolExecutionResult {
    match available_tools.iter().find(|tool| tool.name() == tool_name) {
        Some(tool) => ToolExecutionResult::Success(tool.execute(arguments_json, context).await),
        None => ToolExecutionResult::ToolUnavailable(
            serde_json::json!({
                "error": format!("tool not available in this scope: {tool_name}")
            })
            .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        run_tool_loop, with_tool_prompt_guidance, GetSelectedWorkoutDataPort, LlmToolLoopState,
        ToolExecutionContext, ToolScope,
    };
    use crate::domain::{
        llm::{
            LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmError,
            LlmFinishReason, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall,
        },
        training_context::TrainingContext,
    };

    #[test]
    fn tool_loop_state_defaults_empty() {
        let state = LlmToolLoopState::default();
        assert!(state.provider_transcript.is_empty());
        assert!(state.public_tool_calls.is_empty());
        assert_eq!(state.round_count, 0);
    }

    #[test]
    fn prompt_guidance_lists_available_tools_for_supported_provider() {
        let prompt = with_tool_prompt_guidance(
            "Base prompt.",
            ToolScope::CalendarCoachChat,
            &LlmProvider::OpenAi,
            &sample_tool_context(true),
        );

        assert!(prompt.contains("Tool usage guidance"));
        assert!(prompt.contains("`simulate_forward_load`"));
        assert!(prompt.contains("`get_selected_workout`"));
        assert!(prompt.contains("`selected_workout_power_curve`"));
        assert!(prompt.contains("call it instead of guessing"));
    }

    #[test]
    fn prompt_guidance_hides_data_tools_without_data_port() {
        let prompt = with_tool_prompt_guidance(
            "Base prompt.",
            ToolScope::CalendarCoachChat,
            &LlmProvider::OpenAi,
            &sample_tool_context(false),
        );

        assert!(prompt.contains("`simulate_forward_load`"));
        assert!(!prompt.contains("`get_selected_workout`"));
        assert!(!prompt.contains("`selected_workout_power_curve`"));
    }

    #[test]
    fn prompt_guidance_is_not_added_for_provider_without_tool_support() {
        let prompt = with_tool_prompt_guidance(
            "Base prompt.",
            ToolScope::CalendarCoachChat,
            &LlmProvider::Gemini,
            &sample_tool_context(true),
        );

        assert_eq!(prompt, "Base prompt.");
    }

    #[test]
    fn tool_loop_rejects_runtime_calls_for_tools_not_available_in_scope() {
        let response = futures::executor::block_on(run_tool_loop(
            Arc::new(SingleResponseLlmChatPort::tool_call("get_selected_workout")),
            sample_provider_config(),
            LlmChatRequest {
                user_id: "user-1".to_string(),
                conversation: vec![LlmChatMessage::user("hello")],
                ..Default::default()
            },
            ToolScope::CalendarCoachChat,
            sample_tool_context(false),
            None,
        ))
        .expect("tool loop should finish");

        let tool_message = response
            .state
            .provider_transcript
            .iter()
            .find(|message| matches!(message.role, crate::domain::llm::LlmMessageRole::Tool))
            .expect("tool message should be recorded");

        assert!(tool_message
            .content
            .contains("tool not available in this scope"));
        assert!(tool_message.content.contains("get_selected_workout"));
    }

    fn sample_tool_context(with_data_port: bool) -> ToolExecutionContext {
        ToolExecutionContext {
            user_id: "user-1".to_string(),
            training_context: TrainingContext {
                focus_kind: "calendar".to_string(),
                ..TrainingContext::default()
            },
            today: "2026-05-06".to_string(),
            data_port: with_data_port.then(|| {
                Arc::new(NoopGetSelectedWorkoutDataPort) as Arc<dyn GetSelectedWorkoutDataPort>
            }),
        }
    }

    #[derive(Clone)]
    struct NoopGetSelectedWorkoutDataPort;

    #[derive(Clone)]
    struct SingleResponseLlmChatPort {
        response: LlmChatResponse,
    }

    impl SingleResponseLlmChatPort {
        fn tool_call(tool_name: &str) -> Self {
            Self {
                response: LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "gpt-4o-mini".to_string(),
                    message: LlmChatMessage::assistant_with_tool_calls(
                        "",
                        vec![LlmToolCall {
                            id: "tool-1".to_string(),
                            name: tool_name.to_string(),
                            arguments_json: r#"{"date":"2026-05-06"}"#.to_string(),
                        }],
                    ),
                    finish_reason: Some(LlmFinishReason::ToolCalls),
                    provider_request_id: None,
                    usage: LlmTokenUsage::default(),
                    cache: LlmCacheUsage::default(),
                },
            }
        }
    }

    impl crate::domain::llm::LlmChatPort for SingleResponseLlmChatPort {
        fn chat(
            &self,
            _config: LlmProviderConfig,
            _request: LlmChatRequest,
        ) -> crate::domain::llm::BoxFuture<Result<LlmChatResponse, LlmError>> {
            let response = self.response.clone();
            Box::pin(async move { Ok(response) })
        }
    }

    fn sample_provider_config() -> LlmProviderConfig {
        LlmProviderConfig {
            provider: LlmProvider::OpenAi,
            model: "gpt-4o-mini".to_string(),
            api_key: "test-key".to_string(),
        }
    }

    impl GetSelectedWorkoutDataPort for NoopGetSelectedWorkoutDataPort {
        fn list_completed_by_date_range(
            &self,
            _user_id: &str,
            _oldest: &str,
            _newest: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Vec<crate::domain::completed_workouts::CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn list_planned_by_date_range(
            &self,
            _user_id: &str,
            _oldest: &str,
            _newest: &str,
        ) -> crate::domain::planned_workouts::BoxFuture<
            Result<
                Vec<crate::domain::planned_workouts::PlannedWorkout>,
                crate::domain::planned_workouts::PlannedWorkoutError,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn list_races_by_date_range(
            &self,
            _user_id: &str,
            _oldest: &str,
            _newest: &str,
        ) -> crate::domain::races::BoxFuture<
            Result<Vec<crate::domain::races::Race>, crate::domain::races::RaceError>,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn find_summaries_by_workout_ids(
            &self,
            _user_id: &str,
            _workout_ids: Vec<String>,
        ) -> crate::domain::workout_summary::BoxFuture<
            Result<
                Vec<crate::domain::workout_summary::WorkoutSummary>,
                crate::domain::workout_summary::WorkoutSummaryError,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }
    }
}
