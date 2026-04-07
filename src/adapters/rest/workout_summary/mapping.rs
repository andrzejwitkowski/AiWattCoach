use crate::domain::workout_summary::{
    ConversationMessage, MessageRole, SaveSummaryResult, SaveWorkflowResult, SaveWorkflowStatus,
    SendMessageResult, WorkoutSummary,
};

use super::dto::{
    ConversationMessageDto, SaveWorkflowDto, SaveWorkflowStatusDto, SendMessageResponseDto,
    WorkoutSummaryDto,
};

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

pub(super) fn map_save_summary_result_to_dto(
    result: SaveSummaryResult,
) -> (WorkoutSummaryDto, SaveWorkflowDto) {
    (
        map_summary_to_dto(result.summary),
        SaveWorkflowDto {
            recap_status: map_workflow_status_to_dto(result.workflow.recap_status),
            plan_status: map_workflow_status_to_dto(result.workflow.plan_status),
            messages: result.workflow.messages,
        },
    )
}

pub(super) fn unchanged_save_summary_result(summary: WorkoutSummary) -> SaveSummaryResult {
    SaveSummaryResult {
        summary,
        workflow: SaveWorkflowResult {
            recap_status: SaveWorkflowStatus::Unchanged,
            plan_status: SaveWorkflowStatus::Unchanged,
            messages: Vec::new(),
        },
    }
}

fn map_workflow_status_to_dto(status: SaveWorkflowStatus) -> SaveWorkflowStatusDto {
    match status {
        SaveWorkflowStatus::Generated => SaveWorkflowStatusDto::Generated,
        SaveWorkflowStatus::Skipped => SaveWorkflowStatusDto::Skipped,
        SaveWorkflowStatus::Failed => SaveWorkflowStatusDto::Failed,
        SaveWorkflowStatus::Unchanged => SaveWorkflowStatusDto::Unchanged,
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
