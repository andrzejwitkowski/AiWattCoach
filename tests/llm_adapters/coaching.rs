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
        .reply("user-1", &sample_summary(), "How did I do?", None)
        .await
        .unwrap();

    assert_eq!(response.message, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].reusable_cache_id, None);
    assert_eq!(
        requests[0].volatile_context,
        "training_context_volatile={\"volatile\":true}"
    );
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
            .reply("user-1", &sample_summary(), "How did I do?", None)
            .await
            .unwrap()
    })
    .await;

    assert!(logs.contains("prepared workout summary llm request"));
    assert!(logs.contains("estimated_request_tokens"));
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
        )
        .await
        .unwrap();

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .stable_context
        .contains("athlete_summary_text=Athlete is durable, handles load well"));
}

#[tokio::test]
async fn llm_workout_coach_describes_power_compression_in_system_prompt() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        Arc::new(StubTrainingContextBuilder),
        FixedClock,
    );

    coach
        .reply("user-1", &sample_summary(), "How did I do?", None)
        .await
        .unwrap();

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].system_prompt.contains("pc"));
    assert!(requests[0]
        .system_prompt
        .contains("Be direct, adult, and concise."));
    assert!(requests[0]
        .system_prompt
        .contains("Do not flatter, hedge, or act like a yes-man."));
    assert!(requests[0]
        .system_prompt
        .contains("ready to regenerate workouts"));
    assert!(requests[0].system_prompt.contains("level:seconds"));
    assert!(requests[0]
        .system_prompt
        .contains("round((watts / ftp)^2.5 * 100)"));
    assert!(requests[0].system_prompt.contains("90-110"));
    assert!(requests[0]
        .system_prompt
        .contains("isolated 1-second spike or dip"));
    assert!(requests[0].system_prompt.contains("v=schema version"));
    assert!(requests[0].system_prompt.contains("fx=focus"));
    assert!(requests[0].system_prompt.contains("rd=recent days"));
    assert!(requests[0].system_prompt.contains("ud=upcoming days"));
    assert!(requests[0].system_prompt.contains("sd=start_date_local"));
    assert!(requests[0].system_prompt.contains("ifv=intensity_factor"));
    assert!(requests[0].system_prompt.contains("bl=interval blocks"));
    assert!(requests[0]
        .system_prompt
        .contains("c5=cadence values in 5-second buckets"));
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
