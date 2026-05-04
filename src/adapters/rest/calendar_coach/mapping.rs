use crate::domain::coach_conversation::{
    CoachConversation, CoachConversationMessage, CoachConversationMessageRole,
    CoachConversationStatus,
};

use super::dto::{
    CalendarCoachConversationResponseDto, CoachConversationDto, CoachConversationMessageDto,
    SendMessageResponseDto, ToolCallDto,
};
use crate::domain::coach_conversation::SendConversationMessageResult;

pub(super) fn map_conversation_response(
    conversation: CoachConversation,
    messages: Vec<CoachConversationMessage>,
) -> CalendarCoachConversationResponseDto {
    CalendarCoachConversationResponseDto {
        conversation: map_conversation_to_dto(conversation),
        messages: messages.into_iter().map(map_message_to_dto).collect(),
    }
}

pub(super) fn map_send_message_result(
    result: SendConversationMessageResult,
) -> SendMessageResponseDto {
    SendMessageResponseDto {
        conversation: map_conversation_to_dto(result.conversation),
        messages: result
            .messages
            .into_iter()
            .map(map_message_to_dto)
            .collect(),
        user_message: map_message_to_dto(result.user_message),
        coach_message: map_message_to_dto(result.coach_message),
    }
}

pub(super) fn map_conversation_to_dto(conversation: CoachConversation) -> CoachConversationDto {
    CoachConversationDto {
        conversation_id: conversation.conversation_id,
        surface: conversation.surface.as_str().to_string(),
        status: match conversation.status {
            CoachConversationStatus::Active => "active".to_string(),
            CoachConversationStatus::Archived => "archived".to_string(),
        },
        focus: conversation.focus.kind().to_string(),
        created_at_epoch_seconds: conversation.created_at_epoch_seconds,
        updated_at_epoch_seconds: conversation.updated_at_epoch_seconds,
    }
}

pub(super) fn map_message_to_dto(message: CoachConversationMessage) -> CoachConversationMessageDto {
    CoachConversationMessageDto {
        id: message.id,
        role: match message.role {
            CoachConversationMessageRole::User => "user".to_string(),
            CoachConversationMessageRole::Coach => "coach".to_string(),
            CoachConversationMessageRole::System => "system".to_string(),
            CoachConversationMessageRole::Tool => "tool".to_string(),
        },
        content: message.content,
        tool_call: message.tool_call.map(|tool_call| ToolCallDto {
            id: tool_call.id,
            name: tool_call.name,
            arguments_json: tool_call.arguments_json,
            arguments_preview: tool_call.arguments_preview,
        }),
        created_at_epoch_seconds: message.created_at_epoch_seconds,
    }
}
