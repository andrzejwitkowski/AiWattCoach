use std::sync::{Arc, Mutex};

use aiwattcoach::{
    adapters::llm::training_plan_generator::TrainingPlanLlmGenerator,
    domain::ai_workflow::ValidationIssue,
    domain::llm::{
        BoxFuture as LlmBoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmError, LlmFinishReason, LlmProvider, LlmProviderConfig, LlmTokenUsage, LlmToolCall,
    },
    domain::llm_tools::LlmToolLoopOutput,
    domain::training_context::{
        IntervalsStatusContext, RenderedTrainingContext, TrainingContext,
        TrainingContextBuildResult, TrainingContextBuilder, ATHLETE_SUMMARY_FOCUS_ID,
        CALENDAR_OVERVIEW_FOCUS_ID,
    },
    domain::training_plan::{
        TrainingPlanConversationMessage, TrainingPlanConversationRole, TrainingPlanGenerator,
        TrainingPlanPlanningContext, TrainingPlanToolLoopCheckpoint,
    },
    domain::workout_summary::WorkoutRecap,
};

use crate::support::{
    CapturingChatPort, FixedClock, FixedGeminiConfigProvider, FixedOpenAiConfigProvider,
    StubTrainingContextBuilder,
};

#[derive(Clone)]
struct LargeContextTrainingContextBuilder;

#[derive(Clone)]
struct UnconfiguredAvailabilityTrainingContextBuilder;

impl TrainingContextBuilder for LargeContextTrainingContextBuilder {
    fn build(
        &self,
        _user_id: &str,
        workout_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            Ok(TrainingContextBuildResult {
                context: TrainingContext {
                    generated_at_epoch_seconds: 1_700_000_000,
                    focus_workout_id: Some(workout_id),
                    focus_kind: "activity".to_string(),
                    intervals_status: IntervalsStatusContext {
                        activities: "ok".to_string(),
                        events: "ok".to_string(),
                    },
                    profile: Default::default(),
                    races: Vec::new(),
                    future_events: Vec::new(),
                    history: Default::default(),
                    recent_days: Vec::new(),
                    upcoming_days: Vec::new(),
                    projected_days: Vec::new(),
                },
                rendered: RenderedTrainingContext {
                    stable_context: "s".repeat(4_000_000),
                    volatile_context: "v".repeat(4_000_000),
                    approximate_tokens: 2_000_000,
                },
            })
        })
    }

    fn build_calendar_overview_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", CALENDAR_OVERVIEW_FOCUS_ID)
    }

    fn build_athlete_summary_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", ATHLETE_SUMMARY_FOCUS_ID)
    }
}

impl TrainingContextBuilder for UnconfiguredAvailabilityTrainingContextBuilder {
    fn build(
        &self,
        _user_id: &str,
        workout_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            Ok(TrainingContextBuildResult {
                context: TrainingContext {
                    generated_at_epoch_seconds: 1_700_000_000,
                    focus_workout_id: Some(workout_id),
                    focus_kind: "activity".to_string(),
                    intervals_status: IntervalsStatusContext {
                        activities: "ok".to_string(),
                        events: "ok".to_string(),
                    },
                    profile: aiwattcoach::domain::training_context::AthleteProfileContext {
                        availability_configured: false,
                        ..Default::default()
                    },
                    races: Vec::new(),
                    future_events: Vec::new(),
                    history: Default::default(),
                    recent_days: Vec::new(),
                    upcoming_days: Vec::new(),
                    projected_days: Vec::new(),
                },
                rendered: RenderedTrainingContext {
                    stable_context: "{\"stable\":true}".to_string(),
                    volatile_context: "{\"volatile\":true}".to_string(),
                    approximate_tokens: 100,
                },
            })
        })
    }

    fn build_calendar_overview_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", CALENDAR_OVERVIEW_FOCUS_ID)
    }

    fn build_athlete_summary_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", ATHLETE_SUMMARY_FOCUS_ID)
    }
}

fn sample_planning_context() -> TrainingPlanPlanningContext {
    TrainingPlanPlanningContext {
        rpe: Some(7),
        messages: vec![
            TrainingPlanConversationMessage {
                role: TrainingPlanConversationRole::Coach,
                content: "I am planning a recovery week with easy endurance only and no hard sessions unless they become truly necessary.".to_string(),
            },
            TrainingPlanConversationMessage {
                role: TrainingPlanConversationRole::User,
                content: "Please keep it light because I feel stale.".to_string(),
            },
        ],
    }
}

#[tokio::test]
async fn training_plan_generator_builds_workout_recap_request_from_training_context() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let recap = generator
        .generate_workout_recap("user-1", "workout-1", 1_699_999_000)
        .await
        .unwrap();

    assert_eq!(recap.text, "Gemini coach reply");
    assert_eq!(recap.provider, "gemini");
    assert_eq!(recap.model, "gemini-3.1-pro");
    assert_eq!(recap.generated_at_epoch_seconds, 1_700_000_000);

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .system_prompt
        .contains("completed workout recap"));
    assert!(requests[0]
        .stable_context
        .contains("training_plan_source_stable={\"stable\":true}"));
    assert!(!requests[0]
        .stable_context
        .contains("planning_conversation="));
    assert!(requests[0]
        .volatile_context
        .contains("training_plan_source_volatile={\"volatile\":true}"));
    assert_eq!(requests[0].conversation.len(), 1);
    assert!(requests[0].conversation[0]
        .content
        .contains("Generate a concise workout recap"));
}

#[tokio::test]
async fn training_plan_generator_describes_packed_context_legend_in_system_prompts() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    generator
        .generate_workout_recap("user-1", "workout-1", 1_699_999_000)
        .await
        .unwrap();

    let prompt = &chat_port.requests()[0].system_prompt;
    assert!(prompt.contains("Packed context legend"));
    assert!(prompt.contains("v=schema version"));
    assert!(prompt.contains("rc=race calendar"));
    assert!(prompt.contains("fe=future planned calendar events"));
    assert!(prompt.contains("fx=focus"));
    assert!(prompt.contains("rd=recent days"));
    assert!(prompt.contains("ud=upcoming days"));
    assert!(prompt.contains("pd=projected days"));
    assert!(prompt.contains("pc"));
    assert!(prompt.contains("level:seconds"));
    assert!(prompt.contains("rounded to the nearest 10W bucket"));
    assert!(prompt.contains("round((watts / ftp)^2.5 * 100)"));
    assert!(prompt.contains("same encoded level are run-length encoded"));
}

#[tokio::test]
async fn training_plan_generator_explains_dated_output_grammar_in_plan_prompts() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    generator
        .generate_initial_plan_window(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
        )
        .await
        .unwrap();

    let initial_prompt = &chat_port.requests()[0].system_prompt;
    assert!(initial_prompt.contains("strict syntax generator"));
    assert!(initial_prompt.contains("Output ONLY the raw workout text"));
    assert!(initial_prompt
        .contains("Every actionable workout step MUST begin with a hyphen followed by a space"));
    assert!(initial_prompt.contains("Output grammar"));
    assert!(initial_prompt.contains("YYYY-MM-DD"));
    assert!(initial_prompt.contains("One dated section per day"));
    assert!(initial_prompt.contains("Rest Day"));
    assert!(initial_prompt.contains("Rest Day: <reason>"));
    assert!(initial_prompt.contains("- [Duration] [Target]"));
    assert!(initial_prompt.contains("- [Duration] ramp [Start Target]-[End Target]"));
    assert!(initial_prompt.contains("Supported durations"));
    assert!(initial_prompt.contains("Supported targets"));
    assert!(initial_prompt.contains("backend can persist the reason"));
    assert!(initial_prompt.contains("Do not use cadence"));
    assert!(initial_prompt.contains("- 45m 65%"));

    generator
        .correct_invalid_days(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
            "2026-04-05\n- 10m nonsense",
            vec![ValidationIssue {
                scope: "2026-04-05".to_string(),
                message: "invalid planned workout step".to_string(),
            }],
        )
        .await
        .unwrap();

    let correction_prompt = &chat_port.requests()[1].system_prompt;
    assert!(correction_prompt.contains("strict syntax generator"));
    assert!(correction_prompt.contains("Output ONLY the raw workout text"));
    assert!(correction_prompt.contains("Output grammar"));
    assert!(correction_prompt.contains("YYYY-MM-DD"));
    assert!(correction_prompt.contains("One dated section per day"));
    assert!(correction_prompt.contains("Only output corrected dated sections"));
    assert!(correction_prompt
        .contains("Earlier assistant-role messages are your own earlier coach statements"));
    assert!(
        correction_prompt.contains("Do not plan all 14 days from one static CTL/ATL/TSB snapshot.")
    );
}

#[tokio::test]
async fn training_plan_generator_builds_initial_window_request_with_recap() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let response = generator
        .generate_initial_plan_window(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
        )
        .await
        .unwrap();

    assert_eq!(response.raw_response, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .system_prompt
        .contains("14-day internal cycling plan window"));
    assert!(requests[0]
        .stable_context
        .contains("workout_recap={\"text\":\"Recovered well and handled threshold steadily\""));
    assert!(requests[0].stable_context.contains("planning_rpe=7"));
    assert!(!requests[0]
        .stable_context
        .contains("planning_conversation="));
    assert_eq!(requests[0].conversation.len(), 3);
    assert_eq!(
        requests[0].conversation[0].role,
        aiwattcoach::domain::llm::LlmMessageRole::Assistant
    );
    assert_eq!(
        requests[0].conversation[1].role,
        aiwattcoach::domain::llm::LlmMessageRole::User
    );
    assert!(requests[0].conversation[0]
        .content
        .contains("I am planning a recovery week with easy endurance only"));
    assert!(requests[0].conversation[2]
        .content
        .contains("Generate the next 14 dated days"));
}

#[tokio::test]
async fn training_plan_generator_uses_unconfigured_availability_guidance_when_needed() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(UnconfiguredAvailabilityTrainingContextBuilder),
        FixedClock,
    );

    generator
        .generate_initial_plan_window(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
        )
        .await
        .unwrap();

    let prompt = &chat_port.requests()[0].system_prompt;
    assert!(prompt.contains("Weekly availability is not configured in this context."));
    assert!(!prompt.contains("Weekly availability is mandatory and must be respected"));
}

#[tokio::test]
async fn training_plan_generator_builds_correction_request_with_issues_and_invalid_days_only() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let response = generator
        .correct_invalid_days(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
            "2026-04-05\n- 10m nonsense",
            vec![ValidationIssue {
                scope: "2026-04-05".to_string(),
                message: "invalid planned workout step".to_string(),
            }],
        )
        .await
        .unwrap();

    assert_eq!(response.raw_response, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .system_prompt
        .contains("correct invalid dated workout sections"));
    assert!(requests[0]
        .stable_context
        .contains("workout_recap={\"text\":\"Recovered well and handled threshold steadily\""));
    assert!(requests[0].stable_context.contains("planning_rpe=7"));
    assert!(!requests[0]
        .stable_context
        .contains("planning_conversation="));
    assert_eq!(requests[0].conversation.len(), 3);
    assert_eq!(
        requests[0].conversation[0].role,
        aiwattcoach::domain::llm::LlmMessageRole::Assistant
    );
    assert!(requests[0].conversation[2]
        .content
        .contains("2026-04-05\n- 10m nonsense"));
    assert!(requests[0].conversation[2]
        .content
        .contains("invalid planned workout step"));
}

#[tokio::test]
async fn training_plan_generator_does_not_reject_large_context_before_calling_chat_port() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(LargeContextTrainingContextBuilder),
        FixedClock,
    );

    let response = generator
        .generate_workout_recap("user-1", "workout-1", 1_700_000_000)
        .await;

    assert!(response.is_ok(), "unexpected error: {response:?}");
    assert_eq!(chat_port.requests().len(), 1);
}

#[derive(Clone, Default)]
struct BlankAssistantChatPort;

#[derive(Clone, Default)]
struct ToolCallingChatPort {
    requests: Arc<std::sync::Mutex<Vec<LlmChatRequest>>>,
}

impl ToolCallingChatPort {
    fn requests(&self) -> Vec<LlmChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl LlmChatPort for ToolCallingChatPort {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> LlmBoxFuture<Result<LlmChatResponse, LlmError>> {
        let call_index = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        Box::pin(async move {
            if call_index == 1 {
                return Ok(LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "gpt-4o-mini".to_string(),
                    message: LlmChatMessage::assistant_with_tool_calls(
                        "",
                        vec![LlmToolCall {
                            id: "tool-1".to_string(),
                            name: "simulate_forward_load".to_string(),
                            arguments_json: serde_json::json!({
                                "dated_workout_text": "2023-11-15\nEndurance\n- 45m 65%"
                            })
                            .to_string(),
                        }],
                    ),
                    finish_reason: Some(LlmFinishReason::ToolCalls),
                    provider_request_id: Some("req-tool-1".to_string()),
                    usage: LlmTokenUsage::default(),
                    cache: Default::default(),
                });
            }

            Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                message: LlmChatMessage::assistant("2023-11-15\nRest Day"),
                finish_reason: Some(LlmFinishReason::Stop),
                provider_request_id: Some("req-tool-2".to_string()),
                usage: LlmTokenUsage::default(),
                cache: Default::default(),
            })
        })
    }
}

impl LlmChatPort for BlankAssistantChatPort {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        _request: LlmChatRequest,
    ) -> LlmBoxFuture<Result<LlmChatResponse, LlmError>> {
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::Gemini,
                model: "gemini-3.1-pro".to_string(),
                message: LlmChatMessage::assistant("  "),
                finish_reason: None,
                provider_request_id: Some("req-blank".to_string()),
                usage: LlmTokenUsage::default(),
                cache: Default::default(),
            })
        })
    }
}

#[tokio::test]
async fn training_plan_generator_fails_when_llm_returns_blank_assistant_text() {
    let generator = TrainingPlanLlmGenerator::new(
        Arc::new(BlankAssistantChatPort),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let error = generator
        .generate_workout_recap("user-1", "workout-1", 1_699_999_000)
        .await
        .expect_err("blank assistant text should fail");

    assert_eq!(
        error,
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "LLM returned no assistant text".to_string(),
        )
    );
}

#[tokio::test]
async fn training_plan_generator_checkpoints_final_no_tool_response_before_returning() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );
    let checkpoints = Arc::new(Mutex::new(Vec::new()));
    let checkpoint: TrainingPlanToolLoopCheckpoint = Arc::new({
        let checkpoints = checkpoints.clone();
        move |state| {
            let checkpoints = checkpoints.clone();
            Box::pin(async move {
                checkpoints.lock().unwrap().push(state);
                Ok(())
            })
        }
    });

    let response = generator
        .generate_initial_plan_window_with_state(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
            None,
            Some(checkpoint),
        )
        .await
        .unwrap();

    let checkpoints = checkpoints.lock().unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(
        checkpoints[0]
            .completed_response
            .as_ref()
            .map(|response| response.message.content.as_str()),
        Some(response.raw_response.as_str())
    );
    assert_eq!(chat_port.requests().len(), 1);
}

#[tokio::test]
async fn training_plan_generator_returns_error_when_final_checkpoint_fails() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );
    let checkpoint: TrainingPlanToolLoopCheckpoint = Arc::new(|_| {
        Box::pin(async {
            Err(
                aiwattcoach::domain::training_plan::TrainingPlanError::Repository(
                    "checkpoint write failed".to_string(),
                ),
            )
        })
    });

    let error = generator
        .generate_initial_plan_window_with_state(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
            None,
            Some(checkpoint),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "checkpoint write failed".to_string()
        )
    );
    assert_eq!(chat_port.requests().len(), 1);
}

#[tokio::test]
async fn training_plan_generator_reuses_completed_tool_loop_state_without_second_chat_call() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );
    let restored_state = LlmToolLoopOutput::from_response(LlmChatResponse {
        provider: LlmProvider::Gemini,
        model: "gemini-3.1-pro".to_string(),
        message: LlmChatMessage::assistant("2023-11-15\nRest Day"),
        finish_reason: Some(LlmFinishReason::Stop),
        provider_request_id: Some("req-restored".to_string()),
        usage: LlmTokenUsage::default(),
        cache: Default::default(),
    })
    .state;

    let response = generator
        .generate_initial_plan_window_with_state(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "gemini",
                "gemini-3.1-pro",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
            Some(restored_state),
            None,
        )
        .await
        .unwrap();

    assert_eq!(response.raw_response, "2023-11-15\nRest Day");
    assert_eq!(response.tool_loop_state.round_count, 1);
    assert!(chat_port.requests().is_empty());
}

#[tokio::test]
async fn training_plan_generator_runs_shared_tool_loop_for_openai_plan_generation() {
    let chat_port = Arc::new(ToolCallingChatPort::default());
    let generator = TrainingPlanLlmGenerator::new(
        chat_port.clone(),
        Arc::new(FixedOpenAiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let response = generator
        .generate_initial_plan_window(
            "user-1",
            "workout-1",
            1_700_000_000,
            &WorkoutRecap::generated(
                "Recovered well and handled threshold steadily",
                "openai",
                "gpt-4o-mini",
                1_700_000_000,
            ),
            Some(&sample_planning_context()),
        )
        .await
        .unwrap();

    assert_eq!(response.raw_response, "2023-11-15\nRest Day");
    assert_eq!(response.tool_loop_state.round_count, 2);
    assert_eq!(response.tool_loop_state.public_tool_calls.len(), 1);
    assert_eq!(
        response.tool_loop_state.public_tool_calls[0]
            .arguments_preview
            .as_deref(),
        Some("1 dated day from 2023-11-15 to 2023-11-15")
    );

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].tools.len(), 1);
    let tool_names: Vec<String> = requests[0].tools.iter().map(|t| t.name.clone()).collect();
    assert!(tool_names.contains(&"simulate_forward_load".to_string()));
    assert!(!tool_names.contains(&"get_selected_workout".to_string()));
    assert_eq!(
        requests[1].conversation.last().unwrap().role,
        aiwattcoach::domain::llm::LlmMessageRole::Tool
    );
}
