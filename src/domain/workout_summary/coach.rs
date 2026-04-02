use crate::domain::llm::{BoxFuture, LlmChatResponse, LlmError};

use super::WorkoutSummary;

pub trait WorkoutCoach: Send + Sync {
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>>;
}

#[derive(Clone, Default)]
pub struct MockWorkoutCoach;

impl WorkoutCoach for MockWorkoutCoach {
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let response = match summary.rpe {
            Some(rpe) if rpe >= 8 => format!(
                "That sounds like a hard session. With an RPE of {rpe}, what part of the workout drove the most fatigue?"
            ),
            Some(rpe) if rpe <= 4 => format!(
                "That sounds pretty controlled. With an RPE of {rpe}, do you think the workout felt easier than planned?"
            ),
            _ if user_message.to_ascii_lowercase().contains("heavy") => {
                "Heavy legs can come from accumulated fatigue. Did the effort improve after the warmup, or stay flat the whole session?".to_string()
            }
            _ => {
                "Thanks, that helps. What stood out most about how the workout felt compared with the plan?"
                    .to_string()
            }
        };
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: crate::domain::llm::LlmProvider::OpenAi,
                model: "mock-workout-coach".to_string(),
                message: response,
                provider_request_id: Some(format!("mock-{user_id}")),
                usage: crate::domain::llm::LlmTokenUsage::default(),
                cache: crate::domain::llm::LlmCacheUsage::default(),
            })
        })
    }
}
