use std::sync::{Arc, Mutex};

use crate::shared_support::tracing_capture::capture_tracing_logs;
use aiwattcoach::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse, LlmFinishReason,
    LlmMessageRole, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall,
};
use aiwattcoach::domain::llm_tools::{
    run_tool_loop, GetSelectedWorkoutDataPort, ToolExecutionContext, ToolScope,
};
use aiwattcoach::domain::training_context::TrainingContext;
use aiwattcoach::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutError},
    planned_workouts::{PlannedWorkout, PlannedWorkoutError},
    races::{Race, RaceError},
    workout_summary::{WorkoutSummary, WorkoutSummaryError},
};

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
async fn tool_loop_hides_get_selected_workout_without_data_port_and_stops_with_scope_error() {
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
            planned_workout_update_port: None,
        },
        None,
    )
    .await
    .expect("tool loop should finish after the hidden tool is rejected");

    assert_eq!(result.state.round_count, 1);
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
    assert_eq!(requests.len(), 1);
    let first_tool_names = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"simulate_forward_load"));
    assert!(!first_tool_names.contains(&"get_selected_workout"));

    let recorded_tool_result = result
        .state
        .provider_transcript
        .iter()
        .find(|message| matches!(message.role, LlmMessageRole::Tool))
        .expect("tool result should be recorded in the transcript");
    assert_eq!(
        recorded_tool_result.tool_call_id.as_deref(),
        Some("call_get_workout_1")
    );
    assert_eq!(
        recorded_tool_result.content,
        r#"{"error":"tool not available in this scope: get_selected_workout"}"#
    );
}

#[derive(Clone, Default)]
struct EmptyDataPort;

impl GetSelectedWorkoutDataPort for EmptyDataPort {
    fn list_completed_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<Vec<CompletedWorkout>, CompletedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_planned_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::planned_workouts::BoxFuture<
        Result<Vec<PlannedWorkout>, PlannedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_races_by_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::races::BoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_summaries_by_workout_ids(
        &self,
        _user_id: &str,
        _workout_ids: Vec<String>,
    ) -> aiwattcoach::domain::workout_summary::BoxFuture<
        Result<Vec<WorkoutSummary>, WorkoutSummaryError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn load_selected_workout_data_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> aiwattcoach::domain::workout_summary::BoxFuture<
        Result<aiwattcoach::domain::llm_tools::SelectedWorkoutData, WorkoutSummaryError>,
    > {
        Box::pin(async {
            Ok(aiwattcoach::domain::llm_tools::SelectedWorkoutData {
                completed: Vec::new(),
                planned: Vec::new(),
                races: Vec::new(),
                summaries: Vec::new(),
            })
        })
    }
}

#[tokio::test]
async fn tool_loop_sends_get_selected_workout_when_data_port_is_available() {
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
            data_port: Some(Arc::new(EmptyDataPort)),
            planned_workout_update_port: None,
        },
        None,
    )
    .await
    .expect("tool loop should finish after the tool result is replayed");

    assert_eq!(result.state.round_count, 2);

    let requests = port.requests();
    assert_eq!(requests.len(), 2);
    let first_tool_names = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(first_tool_names.contains(&"simulate_forward_load"));
    assert!(first_tool_names.contains(&"get_selected_workout"));

    let replayed_tool_result = requests[1]
        .conversation
        .iter()
        .find(|message| matches!(message.role, LlmMessageRole::Tool))
        .expect("second request should include tool result");
    let replayed_tool_result_json: serde_json::Value =
        serde_json::from_str(replayed_tool_result.content.as_str())
            .expect("tool result should be valid json");
    assert_eq!(
        replayed_tool_result_json,
        serde_json::json!({
            "date": "2026-05-05",
            "races": [],
            "workouts": [],
        })
    );
}

#[tokio::test]
async fn tool_loop_logs_round_trip_and_tool_execution_details() {
    let port = RecordingLlmPort::default();

    let (_, logs) = capture_tracing_logs(|| async {
        run_tool_loop(
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
                data_port: Some(Arc::new(EmptyDataPort)),
                planned_workout_update_port: None,
            },
            None,
        )
        .await
        .expect("tool loop should finish after the tool result is replayed")
    })
    .await;

    assert!(logs.contains("starting llm tool loop"), "logs were: {logs}");
    assert!(
        logs.contains("sending llm tool loop round"),
        "logs were: {logs}"
    );
    assert!(
        logs.contains("received llm tool loop response"),
        "logs were: {logs}"
    );
    assert!(
        logs.contains("executing llm tool call"),
        "logs were: {logs}"
    );
    assert!(
        logs.contains("completed llm tool call"),
        "logs were: {logs}"
    );
    assert!(
        logs.contains("llm tool loop finished without tool calls"),
        "logs were: {logs}"
    );
    assert!(logs.contains("get_selected_workout"), "logs were: {logs}");
    assert!(
        logs.contains(r#"{\"date\":\"2026-05-05\"}"#) || logs.contains(r#"{"date":"2026-05-05"}"#),
        "logs were: {logs}"
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
        recent_workout_recaps: Vec::new(),
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
