use std::sync::Arc;

use aiwattcoach::{
    adapters::llm::workout_summary_coach::LlmWorkoutCoach, domain::workout_summary::WorkoutCoach,
};

use crate::shared_support::tracing_capture::capture_tracing_logs;
use crate::support::{
    sample_request, sample_summary, CapturingChatPort, FailingReusableCacheRepository, FixedClock,
    FixedGeminiConfigProvider, StubTrainingContextBuilder,
};

#[tokio::test]
async fn llm_workout_coach_does_not_fail_when_gemini_cache_lookup_errors() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    )
    .with_context_cache_repository(Arc::new(FailingReusableCacheRepository));

    let response = coach
        .reply("user-1", &sample_summary(), "How did I do?", None, None)
        .await
        .unwrap();

    assert_eq!(
        response.response.assistant_text(),
        Some("Gemini coach reply")
    );

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].reusable_cache_id, None);
    assert!(requests[0]
        .volatile_context
        .contains("currentConversationDatetime"));
    assert!(requests[0]
        .volatile_context
        .contains("training_context_volatile={\"volatile\":true}"));
}

#[tokio::test]
async fn llm_workout_coach_logs_redacted_builder_request_metadata_only() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    let (_, logs) = capture_tracing_logs(|| async {
        coach
            .reply("user-1", &sample_summary(), "How did I do?", None, None)
            .await
            .unwrap()
    })
    .await;

    assert!(logs.contains("prepared workout summary llm request"));
    assert!(logs.contains("system_prompt_chars"));
    assert!(logs.contains("stable_context_chars"));
    assert!(logs.contains("volatile_context_chars"));
    assert!(logs.contains("conversation_messages"));
    assert!(!logs.contains("logging full workout summary llm request"));
    assert!(!logs.contains("training_context_stable="));
    assert!(!logs.contains("training_context_volatile="));
    assert!(!logs.contains("How did I do?"));

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].stable_context.contains("\"saved\":"));
}

#[tokio::test]
async fn llm_workout_coach_includes_athlete_summary_in_stable_context() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    coach
        .reply(
            "user-1",
            &sample_summary(),
            "How did I do?",
            Some("Athlete is durable, handles load well, but fades on repeated anaerobic work."),
            None,
        )
        .await
        .unwrap();

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .stable_context
        .contains("athlete_summary_guidance="));
    assert!(requests[0]
        .stable_context
        .contains("athlete_summary_text=Athlete is durable, handles load well"));
}

#[tokio::test]
async fn llm_workout_coach_includes_current_workout_recap_in_stable_context() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );
    let mut summary = sample_summary();
    summary.workout_recap_text = Some("Finished 12th in the masters field.".to_string());

    coach
        .reply("user-1", &summary, "How did I do?", None, None)
        .await
        .unwrap();

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .stable_context
        .contains("current_workout_recap=Finished 12th in the masters field."));
}

#[tokio::test]
async fn llm_workout_coach_describes_aligned_intervals_in_system_prompt() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    coach
        .reply("user-1", &sample_summary(), "How did I do?", None, None)
        .await
        .unwrap();

    let prompt = &chat_port.requests()[0].system_prompt;
    assert!(prompt.contains("aligned_intervals"));
    assert!(prompt.contains("coasting_stop"));
    assert!(prompt.contains("normalized_power"));
    assert!(prompt.contains("get_selected_workout"));
    assert!(!prompt.contains("ps=power"));
    assert!(!prompt.contains("cs=cadence"));
    assert!(!prompt.contains("header-mapped"));
    assert!(!prompt.contains("p3=power watts"));
    assert!(!prompt.contains("c5=cadence values"));
    assert!(!prompt.contains("level:seconds"));
}

#[test]
fn llm_debug_output_redacts_secrets_and_prompt_contents() {
    let config = aiwattcoach::domain::llm::LlmProviderConfig {
        provider: aiwattcoach::domain::llm::LlmProvider::OpenAi,
        model: "gpt-4o-mini".to_string(),
        api_key: "sk-secret-value".to_string(),
    };
    let request = sample_request();

    let config_debug = format!("{config:?}");
    let request_debug = format!("{request:?}");

    assert!(!config_debug.contains("sk-secret-value"));
    assert!(config_debug.contains("<redacted:"));
    assert!(!request_debug.contains("How did I do?"));
    assert!(!request_debug.contains("stable_context: \"stable\""));
    assert!(!request_debug.contains("system_prompt: \"system\""));
    assert!(request_debug.contains("conversation_len"));
}
