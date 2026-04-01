use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct WorkoutSummaryPath {
    pub workout_id: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct ListWorkoutSummariesQuery {
    // Keep alias for backward compatibility during transition.
    #[serde(rename = "workoutIds", alias = "eventIds")]
    pub workout_ids: String,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct UpdateRpeRequest {
    pub rpe: u8,
}

#[derive(Serialize)]
pub(super) struct WorkoutSummaryStateResponse {
    pub summary: WorkoutSummaryDto,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct SetSavedStateRequest {
    pub saved: bool,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct SendMessageRequest {
    pub content: String,
}

#[derive(Serialize)]
pub(super) struct WorkoutSummaryDto {
    pub id: String,
    #[serde(rename = "workoutId")]
    pub workout_id: String,
    pub rpe: Option<u8>,
    pub messages: Vec<ConversationMessageDto>,
    #[serde(rename = "savedAtEpochSeconds")]
    pub saved_at_epoch_seconds: Option<i64>,
    #[serde(rename = "createdAtEpochSeconds")]
    pub created_at_epoch_seconds: i64,
    #[serde(rename = "updatedAtEpochSeconds")]
    pub updated_at_epoch_seconds: i64,
}

#[derive(Serialize)]
pub(super) struct ConversationMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(rename = "createdAtEpochSeconds")]
    pub created_at_epoch_seconds: i64,
}

#[derive(Serialize)]
pub(super) struct SendMessageResponseDto {
    pub summary: WorkoutSummaryDto,
    #[serde(rename = "userMessage")]
    pub user_message: ConversationMessageDto,
    #[serde(rename = "coachMessage")]
    pub coach_message: ConversationMessageDto,
}

#[derive(Deserialize)]
pub(super) struct ClientWsMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub content: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ServerWsMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ConversationMessageDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<WorkoutSummaryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub(super) fn coach_typing_message() -> ServerWsMessage {
    ServerWsMessage {
        message_type: "coach_typing".to_string(),
        message: None,
        summary: None,
        error: None,
    }
}

pub(super) fn coach_message(
    message: ConversationMessageDto,
    summary: WorkoutSummaryDto,
) -> ServerWsMessage {
    ServerWsMessage {
        message_type: "coach_message".to_string(),
        message: Some(message),
        summary: Some(summary),
        error: None,
    }
}

pub(super) fn error_message(message: impl Into<String>) -> ServerWsMessage {
    ServerWsMessage {
        message_type: "error".to_string(),
        message: None,
        summary: None,
        error: Some(message.into()),
    }
}
