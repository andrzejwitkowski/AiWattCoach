use std::sync::{Arc, Mutex};

use aiwattcoach::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall,
};
use aiwattcoach::domain::llm_tools::{run_tool_loop, ToolExecutionContext, ToolScope};
use aiwattcoach::domain::training_context::TrainingContext;

#[derive(Clone, Default)]
struct RecordingLlmPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
}

impl RecordingLlmPort {
    fn requests(&self) -> Vec<LlmChatRequest> {
        self.requests
            .lock()
            .expect("requests mutex poisoned")
            .clone()
    }
}

impl LlmChatPort for RecordingLlmPort {
    fn chat(
        &self,
        config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> aiwattcoach::domain::llm::BoxFuture<
        Result<LlmChatResponse, aiwattcoach::domain::llm::LlmError>,
    > {
        let requests = self.requests.clone();
        Box::pin(async move {
            let has_tool_result = request
                .conversation
                .iter()
                .any(|message| matches!(message.role, LlmMessageRole::Tool));
            requests
                .lock()
                .expect("requests mutex poisoned")
                .push(request);

            Ok(if has_tool_result {
                LlmChatResponse {
                    provider: config.provider,
                    model: config.model,
                    message: LlmChatMessage::assistant(
                        "I found the selected workout data.".to_string(),
                    ),
                    finish_reason: Some(LlmFinishReason::Stop),
                    provider_request_id: Some("mock-req-2".to_string()),
                    usage: usage(),
                    cache: cache(),
                }
            } else {
                LlmChatResponse {
                    provider: config.provider,
                    model: config.model,
                    message: LlmChatMessage::assistant_with_tool_calls(
                        "I need to look up your workout data for that date.".to_string(),
                        vec![LlmToolCall {
                            id: "call_get_workout_1".to_string(),
                            name: "get_selected_workout".to_string(),
                            arguments_json: r#"{"date":"2026-05-05"}"#.to_string(),
                        }],
                    ),
                    finish_reason: Some(LlmFinishReason::ToolCalls),
                    provider_request_id: Some("mock-req-1".to_string()),
                    usage: usage(),
                    cache: cache(),
                }
            })
        })
    }
}

#[tokio::test]
async fn tool_loop_sends_get_selected_workout_definition_and_replays_result() {
    let port = RecordingLlmPort::default();
    let result = run_tool_loop(
        Arc::new(port.clone()),
        LlmProviderConfig {
            provider: LlmProvider::OpenRouter,
            model: "google/gemini-3.1-pro".to_string(),
            api_key: "test-key".to_string(),
        },
        LlmChatRequest {
            user_id: "user-123".to_string(),
            system_prompt: "You are an AI cycling coach.".to_string(),
            stable_context: "Athlete profile".to_string(),
            volatile_context: "Today is 2026-05-05".to_string(),
            conversation: vec![LlmChatMessage::user("What was my workout on May 5th?")],
            ..Default::default()
        },
        ToolScope::CalendarCoachChat,
        ToolExecutionContext {
            user_id: "user-123".to_string(),
            training_context: empty_training_context(),
            today: "2026-05-05".to_string(),
            data_port: None,
        },
        None,
    )
    .await
    .expect("tool loop should finish after the tool result is replayed");

    assert_eq!(result.state.round_count, 2);
    assert_eq!(result.state.public_tool_calls.len(), 1);
    assert_eq!(
        result.state.public_tool_calls[0].name,
        "get_selected_workout"
    );
    assert_eq!(
        result.state.public_tool_calls[0]
            .arguments_preview
            .as_deref(),
        Some("date 2026-05-05")
    );

    let requests = port.requests();
    assert_eq!(requests.len(), 2);
    let first_tool_names = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"simulate_forward_load"));
    assert!(first_tool_names.contains(&"get_selected_workout"));

    let selected_tool = requests[0]
        .tools
        .iter()
        .find(|tool| tool.name == "get_selected_workout")
        .expect("get_selected_workout tool should be exposed");
    assert!(selected_tool
        .description
        .contains("raw power/cadence/heart-rate streams"));
    assert!(selected_tool
        .input_schema_json
        .contains(r#""required":["date"]"#));

    let replayed_tool_result = requests[1]
        .conversation
        .iter()
        .find(|message| matches!(message.role, LlmMessageRole::Tool))
        .expect("second request should include tool result");
    assert_eq!(
        replayed_tool_result.tool_call_id.as_deref(),
        Some("call_get_workout_1")
    );
    assert_eq!(
        replayed_tool_result.content,
        r#"{"error":"data port not available"}"#
    );
}

fn empty_training_context() -> TrainingContext {
    TrainingContext {
        generated_at_epoch_seconds: 1,
        focus_workout_id: None,
        focus_kind: "calendar".to_string(),
        intervals_status: Default::default(),
        profile: Default::default(),
        races: Vec::new(),
        future_events: Vec::new(),
        history: Default::default(),
        recent_days: Vec::new(),
        upcoming_days: Vec::new(),
        projected_days: Vec::new(),
    }
}

fn usage() -> LlmTokenUsage {
    LlmTokenUsage {
        input_tokens: Some(1),
        output_tokens: Some(1),
        total_tokens: Some(2),
    }
}

fn cache() -> LlmCacheUsage {
    LlmCacheUsage {
        cached_read_tokens: None,
        cache_write_tokens: None,
        cache_hit: false,
        cache_discount: None,
        provider_cache_id: None,
        provider_cache_key: None,
        cache_expires_at_epoch_seconds: None,
    }
}
