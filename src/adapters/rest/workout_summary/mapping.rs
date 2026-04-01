use crate::domain::workout_summary::{
    ConversationMessage, MessageRole, SendMessageResult, WorkoutSummary,
};

use super::dto::{ConversationMessageDto, SendMessageResponseDto, WorkoutSummaryDto};

pub(super) fn map_summary_to_dto(summary: WorkoutSummary) -> WorkoutSummaryDto {
    WorkoutSummaryDto {
        id: summary.id,
        workout_id: summary.workout_id,
        rpe: summary.rpe,
        messages: summary
            .messages
            .into_iter()
            .map(map_message_to_dto)
            .collect(),
        saved_at_epoch_seconds: summary.saved_at_epoch_seconds,
        created_at_epoch_seconds: summary.created_at_epoch_seconds,
        updated_at_epoch_seconds: summary.updated_at_epoch_seconds,
    }
}

pub(super) fn map_send_message_result_to_dto(result: SendMessageResult) -> SendMessageResponseDto {
    SendMessageResponseDto {
        summary: map_summary_to_dto(result.summary),
        user_message: map_message_to_dto(result.user_message),
        coach_message: map_message_to_dto(result.coach_message),
    }
}

pub(super) fn map_message_to_dto(message: ConversationMessage) -> ConversationMessageDto {
    ConversationMessageDto {
        id: message.id,
        role: match message.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Coach => "coach".to_string(),
        },
        content: message.content,
        created_at_epoch_seconds: message.created_at_epoch_seconds,
    }
}
