use crate::domain::{llm::BoxFuture, llm::LlmError, llm_tools::LlmToolLoopOutput};

use super::WorkoutSummary;

pub trait WorkoutCoach: Send + Sync {
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
        athlete_summary_text: Option<&str>,
        power_chart_base64: Option<&str>,
    ) -> BoxFuture<Result<LlmToolLoopOutput, LlmError>>;
}

#[derive(Clone, Default)]
pub struct MockWorkoutCoach;

impl WorkoutCoach for MockWorkoutCoach {
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
        _athlete_summary_text: Option<&str>,
        _power_chart_base64: Option<&str>,
    ) -> BoxFuture<Result<LlmToolLoopOutput, LlmError>> {
        let response = match summary.rpe {
            Some(rpe) if rpe >= 8 => format!(
                r#"{{"summary":"That looked like a hard session at RPE {rpe}. Give me a concise read on the limiter so I can judge how aggressive the next block should be.","questions":[{{"question":"What limited you most today?","answers":["Legs","Breathing","Fueling","Pain","Other"],"freeTextLabel":"Add details if useful"}},{{"question":"How ready are you for the next 2-3 days?","answers":["Ready for quality","Easy only","Need recovery"],"freeTextLabel":"Anything else I should account for?"}}]}}"#
            ),
            Some(rpe) if rpe <= 4 => format!(
                r#"{{"summary":"That sounded controlled for an RPE of {rpe}. I want to know whether the session was genuinely easy or just executed efficiently.","questions":[{{"question":"How did the workout feel against the plan?","answers":["Easier than planned","On target","Harder than planned","Could have done more"],"freeTextLabel":"Add context if needed"}},{{"question":"What should I prioritize next?","answers":["More quality","More volume","Keep it steady","Freshen up"],"freeTextLabel":"Any short note for the next 14 days?"}}]}}"#
            ),
            _ if user_message.to_ascii_lowercase().contains("heavy") => {
                r#"{"summary":"Heavy legs can come from accumulated fatigue, poor fueling, or a bad day of recovery. Help me separate those before we touch the next plan.","questions":[{"question":"What was the main limiter?","answers":["Legs","Breathing","Fueling","Sleep/stress","Pain"],"freeTextLabel":"Add details if useful"},{"question":"How ready are you for the next few days?","answers":["Ready for quality","Moderate only","Need recovery"],"freeTextLabel":"Anything I should avoid in the next block?"}]}"#.to_string()
            }
            _ => {
                r#"{"summary":"Thanks, that helps. I want one clean pass over what mattered so the next plan is based on signal rather than guesswork.","questions":[{"question":"What stood out most about this workout?","answers":["Better than expected","On target","Harder than expected","Poor legs","Poor fueling"],"freeTextLabel":"Add context if useful"},{"question":"What do you want from the next 14 days?","answers":["Build fitness","Absorb training","Sharpen for racing","Stay flexible","Not sure yet"],"freeTextLabel":"Any race, nutrition, or strategy note to consider?"}]}"#.to_string()
            }
        };
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(LlmToolLoopOutput::from_response(
                crate::domain::llm::LlmChatResponse {
                    provider: crate::domain::llm::LlmProvider::OpenAi,
                    model: "mock-workout-coach".to_string(),
                    message: crate::domain::llm::LlmChatMessage::assistant(response),
                    finish_reason: None,
                    provider_request_id: Some(format!("mock-{user_id}")),
                    usage: crate::domain::llm::LlmTokenUsage::default(),
                    cache: crate::domain::llm::LlmCacheUsage::default(),
                },
            ))
        })
    }
}
