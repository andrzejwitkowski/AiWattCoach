use serde::Serialize;

use crate::domain::llm::{
    LlmChatMessage, LlmToolChoice, LlmToolDefinition, PreviewProviderMessage,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminPromptPreviewSurface {
    PostWorkout,
    CalendarCoach,
}

impl AdminPromptPreviewSurface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PostWorkout => "post_workout",
            Self::CalendarCoach => "calendar_coach",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPromptPreviewMeta {
    pub user_id: String,
    pub date: String,
    pub surface: String,
    pub provider: String,
    pub model: String,
    pub focus_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_workout_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_score: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPromptPreviewRequestBody {
    pub system_prompt: String,
    pub stable_context: String,
    pub volatile_context: String,
    pub conversation: Vec<LlmChatMessage>,
    pub tools: Vec<LlmToolDefinition>,
    pub tool_choice: LlmToolChoice,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPromptPreviewResponse {
    pub meta: AdminPromptPreviewMeta,
    pub request: AdminPromptPreviewRequestBody,
    pub provider_messages: Vec<PreviewProviderMessage>,
}
