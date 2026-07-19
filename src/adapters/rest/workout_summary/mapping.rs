use crate::domain::workout_summary::{
    ConversationMessage, MessageRole, SaveSummaryResult, SaveWorkflowResult, SaveWorkflowStatus,
    SendMessageResult, WorkoutSummary,
};

use super::dto::{
    CoachQuestionDto, ConversationMessageDto, SaveWorkflowDto, SaveWorkflowStatusDto,
    SendMessageResponseDto, ToolCallDto, WorkoutSummaryDto,
};

pub(super) fn map_summary_to_dto(summary: WorkoutSummary) -> WorkoutSummaryDto {
    WorkoutSummaryDto {
        id: summary.id,
        workout_id: summary.workout_id,
        rpe: summary.rpe,
        has_coach_message: None,
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

pub(super) fn map_summary_metadata_to_dto(summary: WorkoutSummary) -> WorkoutSummaryDto {
    let has_coach_message = summary
        .messages
        .iter()
        .any(|message| message.role == MessageRole::Coach);
    WorkoutSummaryDto {
        id: summary.id,
        workout_id: summary.workout_id,
        rpe: summary.rpe,
        has_coach_message: Some(has_coach_message),
        messages: Vec::new(),
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

pub(super) fn map_workflow_status_to_dto(status: SaveWorkflowStatus) -> SaveWorkflowStatusDto {
    match status {
        SaveWorkflowStatus::Generated => SaveWorkflowStatusDto::Generated,
        SaveWorkflowStatus::Processing => SaveWorkflowStatusDto::Processing,
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
            MessageRole::Tool => "tool".to_string(),
        },
        content: message.content,
        tool_call: message.tool_call.map(|tool_call| ToolCallDto {
            id: tool_call.id,
            name: tool_call.name,
            arguments_json: tool_call.arguments_json,
            arguments_preview: tool_call.arguments_preview,
        }),
        questions: message
            .questions
            .into_iter()
            .map(|question| CoachQuestionDto {
                id: question.id,
                question: question.question,
                answers: question.answers,
                free_text_label: question.free_text_label,
            })
            .collect(),
        created_at_epoch_seconds: message.created_at_epoch_seconds,
        image_url: message.image_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workout_summary::{CoachQuestion, MessageRole};

    fn user_message_no_questions() -> ConversationMessage {
        ConversationMessage {
            id: "msg-1".to_string(),
            role: MessageRole::User,
            content: "How was the ride?".to_string(),
            tool_call: None,
            questions: vec![],
            created_at_epoch_seconds: 1_000,
            image_url: None,
        }
    }

    fn coach_message_with_questions() -> ConversationMessage {
        ConversationMessage {
            id: "msg-2".to_string(),
            role: MessageRole::Coach,
            content: "Great effort today!".to_string(),
            tool_call: None,
            image_url: None,
            questions: vec![
                CoachQuestion {
                    id: "q1".to_string(),
                    question: "What limited you?".to_string(),
                    answers: vec!["Legs".to_string(), "Breathing".to_string()],
                    free_text_label: None,
                },
                CoachQuestion {
                    id: "q2".to_string(),
                    question: "How were your legs?".to_string(),
                    answers: vec!["Fresh".to_string(), "Heavy".to_string()],
                    free_text_label: Some("Other".to_string()),
                },
            ],
            created_at_epoch_seconds: 2_000,
        }
    }

    #[test]
    fn map_message_to_dto_maps_user_role_and_empty_questions() {
        let dto = map_message_to_dto(user_message_no_questions());

        assert_eq!(dto.id, "msg-1");
        assert_eq!(dto.role, "user");
        assert_eq!(dto.content, "How was the ride?");
        assert!(dto.tool_call.is_none());
        assert!(dto.questions.is_empty());
        assert_eq!(dto.created_at_epoch_seconds, 1_000);
    }

    #[test]
    fn map_message_to_dto_maps_coach_questions_with_and_without_free_text_label() {
        let dto = map_message_to_dto(coach_message_with_questions());

        assert_eq!(dto.role, "coach");
        assert_eq!(dto.questions.len(), 2);

        let q1 = &dto.questions[0];
        assert_eq!(q1.id, "q1");
        assert_eq!(q1.question, "What limited you?");
        assert_eq!(q1.answers, vec!["Legs", "Breathing"]);
        assert!(q1.free_text_label.is_none());

        let q2 = &dto.questions[1];
        assert_eq!(q2.id, "q2");
        assert_eq!(q2.question, "How were your legs?");
        assert_eq!(q2.answers, vec!["Fresh", "Heavy"]);
        assert_eq!(q2.free_text_label.as_deref(), Some("Other"));
    }

    #[test]
    fn map_message_to_dto_serializes_questions_absent_when_empty() {
        let dto = map_message_to_dto(user_message_no_questions());
        let json = serde_json::to_value(&dto).unwrap();
        assert!(
            json.get("questions").is_none(),
            "questions field should be omitted when empty"
        );
    }

    #[test]
    fn map_message_to_dto_serializes_free_text_label_absent_when_none() {
        let dto = map_message_to_dto(coach_message_with_questions());
        let json = serde_json::to_value(&dto).unwrap();
        let q1_json = &json["questions"][0];
        assert!(
            q1_json.get("freeTextLabel").is_none(),
            "freeTextLabel should be omitted when None"
        );
        assert_eq!(json["questions"][1]["freeTextLabel"], "Other");
    }
}
