use std::{future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{
    llm::{
        merge_provider_transcript_entries, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmError, LlmFinishReason, LlmProvider, LlmProviderConfig, LlmToolChoice,
        LlmToolDefinition,
    },
    training_context::TrainingContext,
    workout_summary::PublicToolCall,
};

pub const TOOL_LOOP_MAX_ROUNDS: u32 = 6;
const SIMULATE_FORWARD_LOAD_TOOL_NAME: &str = "simulate_forward_load";

mod simulate_forward_load;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolScope {
    WorkoutSummaryChat,
    CalendarCoachChat,
    TrainingPlanGeneration,
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

#[derive(Clone, Debug)]
pub struct ToolExecutionContext {
    pub training_context: TrainingContext,
    pub today: String,
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

        let tools = tool_definitions_for_scope(scope, &config.provider);
        let tool_choice = if tools.is_empty() {
            LlmToolChoice::None
        } else {
            LlmToolChoice::Auto
        };

        for _ in state.round_count..TOOL_LOOP_MAX_ROUNDS {
            request.conversation = conversation.clone();
            request.tools = tools.clone();
            request.tool_choice = tool_choice.clone();

            let response = chat_port.chat(config.clone(), request.clone()).await?;
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
                let tool_message = LlmChatMessage::tool(
                    tool_call.id.clone(),
                    execute_tool_call(
                        tool_call.name.as_str(),
                        tool_call.arguments_json.as_str(),
                        &tool_context,
                    ),
                );
                provider_transcript.push(tool_message.clone());
                conversation.push(tool_message);
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

        Err(LlmError::InvalidResponse(format!(
            "tool loop exceeded {TOOL_LOOP_MAX_ROUNDS} rounds"
        )))
    })
}

pub fn tool_definitions_for_scope(
    scope: ToolScope,
    provider: &LlmProvider,
) -> Vec<LlmToolDefinition> {
    if !provider_supports_tools(provider) {
        return Vec::new();
    }

    match scope {
        ToolScope::WorkoutSummaryChat
        | ToolScope::CalendarCoachChat
        | ToolScope::TrainingPlanGeneration => vec![LlmToolDefinition {
            name: SIMULATE_FORWARD_LOAD_TOOL_NAME.to_string(),
            description: "Simulate 14 days of forward training load from today using dated workout text and return per-day CTL ATL TSB and planned load estimates.".to_string(),
            input_schema_json: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dated_workout_text": {
                        "type": "string",
                        "description": "Raw dated workout text in the existing YYYY-MM-DD plus workout-builder format."
                    }
                },
                "required": ["dated_workout_text"]
            })
            .to_string(),
        }],
    }
}

fn provider_supports_tools(provider: &LlmProvider) -> bool {
    matches!(provider, LlmProvider::OpenAi | LlmProvider::OpenRouter)
}

pub fn public_tool_call_from_llm(tool_call: &crate::domain::llm::LlmToolCall) -> PublicToolCall {
    PublicToolCall {
        id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        arguments_json: tool_call.arguments_json.clone(),
        arguments_preview: simulate_forward_load::preview_tool_arguments(
            &tool_call.name,
            &tool_call.arguments_json,
        ),
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

fn execute_tool_call(
    tool_name: &str,
    arguments_json: &str,
    context: &ToolExecutionContext,
) -> String {
    match tool_name {
        SIMULATE_FORWARD_LOAD_TOOL_NAME => {
            simulate_forward_load::simulate_forward_load(arguments_json, context)
        }
        _ => json!({
            "error": format!("unknown tool: {tool_name}")
        })
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::LlmToolLoopState;

    #[test]
    fn tool_loop_state_defaults_empty() {
        let state = LlmToolLoopState::default();
        assert!(state.provider_transcript.is_empty());
        assert!(state.public_tool_calls.is_empty());
        assert_eq!(state.round_count, 0);
    }
}
