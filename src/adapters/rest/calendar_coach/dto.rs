use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct CalendarCoachConversationPath {
    pub conversation_id: String,
}

#[derive(Serialize)]
pub(super) struct CoachConversationDto {
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    pub surface: String,
    pub status: String,
    pub focus: String,
    #[serde(rename = "createdAtEpochSeconds")]
    pub created_at_epoch_seconds: i64,
    #[serde(rename = "updatedAtEpochSeconds")]
    pub updated_at_epoch_seconds: i64,
}

#[derive(Serialize)]
pub(super) struct ToolCallDto {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "argumentsJson")]
    pub arguments_json: String,
    #[serde(rename = "argumentsPreview", skip_serializing_if = "Option::is_none")]
    pub arguments_preview: Option<String>,
}

#[derive(Serialize)]
pub(super) struct CoachConversationMessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(rename = "toolCall", skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<ToolCallDto>,
    #[serde(rename = "createdAtEpochSeconds")]
    pub created_at_epoch_seconds: i64,
}

#[derive(Serialize)]
pub(super) struct CalendarCoachConversationResponseDto {
    pub conversation: CoachConversationDto,
    pub messages: Vec<CoachConversationMessageDto>,
}

#[derive(Deserialize)]
pub(in crate::adapters::rest) struct SendMessageRequest {
    pub content: String,
}

#[derive(Serialize)]
pub(super) struct SendMessageResponseDto {
    pub conversation: CoachConversationDto,
    pub messages: Vec<CoachConversationMessageDto>,
    #[serde(rename = "userMessage")]
    pub user_message: CoachConversationMessageDto,
    #[serde(rename = "coachMessage")]
    pub coach_message: CoachConversationMessageDto,
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
    pub message: Option<CoachConversationMessageDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation: Option<CoachConversationDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<CoachConversationMessageDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub(super) fn coach_typing_message() -> ServerWsMessage {
    ServerWsMessage {
        message_type: "coach_typing".to_string(),
        message: None,
        content: None,
        conversation: None,
        messages: None,
        error: None,
    }
}

pub(super) fn coach_message(
    message: CoachConversationMessageDto,
    conversation: CoachConversationDto,
    messages: Vec<CoachConversationMessageDto>,
) -> ServerWsMessage {
    ServerWsMessage {
        message_type: "coach_message".to_string(),
        message: Some(message),
        content: None,
        conversation: Some(conversation),
        messages: Some(messages),
        error: None,
    }
}

pub(super) fn tool_message(message: CoachConversationMessageDto) -> ServerWsMessage {
    ServerWsMessage {
        message_type: "tool_message".to_string(),
        message: Some(message),
        content: None,
        conversation: None,
        messages: None,
        error: None,
    }
}

pub(super) fn error_message(message: impl Into<String>) -> ServerWsMessage {
    ServerWsMessage {
        message_type: "error".to_string(),
        message: None,
        content: None,
        conversation: None,
        messages: None,
        error: Some(message.into()),
    }
}

pub(super) fn coach_thinking_message() -> ServerWsMessage {
    ServerWsMessage {
        message_type: "coach_thinking".to_string(),
        message: None,
        content: None,
        conversation: None,
        messages: None,
        error: None,
    }
}
