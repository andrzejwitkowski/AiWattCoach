use std::sync::Arc;

use aiwattcoach::{
    adapters::llm::training_plan_generator::TrainingPlanLlmGenerator,
    domain::ai_workflow::ValidationIssue,
    domain::llm::{BoxFuture as LlmBoxFuture, LlmError},
    domain::training_context::{
        IntervalsStatusContext, RenderedTrainingContext, TrainingContext,
        TrainingContextBuildResult, TrainingContextBuilder,
    },
    domain::training_plan::TrainingPlanGenerator,
    domain::workout_summary::WorkoutRecap,
};

use crate::support::{
    CapturingChatPort, FixedClock, FixedGeminiConfigProvider, StubTrainingContextBuilder,
};

#[derive(Clone)]
struct LargeContextTrainingContextBuilder;

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

    fn build_athlete_summary_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", "athlete-summary")
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
    assert!(requests[0]
        .volatile_context
        .contains("training_plan_source_volatile={\"volatile\":true}"));
    assert_eq!(requests[0].conversation.len(), 1);
    assert!(requests[0].conversation[0]
        .content
        .contains("Generate a concise workout recap"));
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
        )
        .await
        .unwrap();

    assert_eq!(response, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .system_prompt
        .contains("14-day internal cycling plan window"));
    assert!(requests[0]
        .stable_context
        .contains("workout_recap={\"text\":\"Recovered well and handled threshold steadily\""));
    assert!(requests[0].conversation[0]
        .content
        .contains("Generate the next 14 dated days"));
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
            "2026-04-05\n- 10m nonsense",
            vec![ValidationIssue {
                scope: "2026-04-05".to_string(),
                message: "invalid planned workout step".to_string(),
            }],
        )
        .await
        .unwrap();

    assert_eq!(response, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .system_prompt
        .contains("correct invalid dated workout sections"));
    assert!(requests[0]
        .stable_context
        .contains("workout_recap={\"text\":\"Recovered well and handled threshold steadily\""));
    assert!(requests[0].conversation[0]
        .content
        .contains("2026-04-05\n- 10m nonsense"));
    assert!(requests[0].conversation[0]
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
