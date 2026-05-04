use crate::{
    adapters::mongo::time::{
        optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds,
        resolve_required_epoch_seconds,
    },
    domain::workout_summary::{
        ConversationMessage, MessageRole, WorkoutSummary, WorkoutSummaryError,
    },
};

use super::document::{ConversationMessageDocument, WorkoutSummaryDocument};

pub(super) fn map_document_to_domain(
    document: WorkoutSummaryDocument,
) -> Result<WorkoutSummary, WorkoutSummaryError> {
    Ok(WorkoutSummary {
        id: document.summary_id,
        user_id: document.user_id,
        workout_id: document.workout_id,
        rpe: document.rpe.map(map_rpe_to_domain).transpose()?,
        messages: document
            .messages
            .into_iter()
            .map(map_message_to_domain)
            .collect::<Result<Vec<_>, _>>()?,
        hidden_transcript: document.hidden_transcript,
        saved_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.saved_at,
            document.saved_at_epoch_seconds,
        ),
        workout_recap_text: document.workout_recap_text,
        workout_recap_provider: document.workout_recap_provider,
        workout_recap_model: document.workout_recap_model,
        workout_recap_generated_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.workout_recap_generated_at,
            document.workout_recap_generated_at_epoch_seconds,
        ),
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
    })
}

pub(super) fn map_domain_to_document(summary: &WorkoutSummary) -> WorkoutSummaryDocument {
    WorkoutSummaryDocument {
        id: None,
        summary_id: summary.id.clone(),
        user_id: summary.user_id.clone(),
        workout_id: summary.workout_id.clone(),
        rpe: summary.rpe.map(i32::from),
        messages: summary
            .messages
            .iter()
            .cloned()
            .map(map_message_to_document)
            .collect(),
        hidden_transcript: summary.hidden_transcript.clone(),
        saved_at_epoch_seconds: summary.saved_at_epoch_seconds,
        saved_at: optional_epoch_seconds_to_bson_datetime(
            summary.saved_at_epoch_seconds,
            "saved_at",
        )
        .expect("saved_at should fit BSON DateTime"),
        workout_recap_text: summary.workout_recap_text.clone(),
        workout_recap_provider: summary.workout_recap_provider.clone(),
        workout_recap_model: summary.workout_recap_model.clone(),
        workout_recap_generated_at_epoch_seconds: summary.workout_recap_generated_at_epoch_seconds,
        workout_recap_generated_at: optional_epoch_seconds_to_bson_datetime(
            summary.workout_recap_generated_at_epoch_seconds,
            "workout_recap_generated_at",
        )
        .expect("workout_recap_generated_at should fit BSON DateTime"),
        created_at_epoch_seconds: Some(summary.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: Some(summary.updated_at_epoch_seconds),
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
    }
}

pub(super) fn map_message_to_document(message: ConversationMessage) -> ConversationMessageDocument {
    ConversationMessageDocument {
        id: message.id,
        role: match message.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Coach => "coach".to_string(),
            MessageRole::Tool => "tool".to_string(),
        },
        content: message.content,
        tool_call: message.tool_call,
        created_at_epoch_seconds: Some(message.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(message.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
    }
}

pub(super) fn map_message_to_domain(
    message: ConversationMessageDocument,
) -> Result<ConversationMessage, WorkoutSummaryError> {
    let role = match message.role.as_str() {
        "user" => MessageRole::User,
        "coach" => MessageRole::Coach,
        "tool" => MessageRole::Tool,
        other => {
            return Err(WorkoutSummaryError::Repository(format!(
                "unknown message role: {other}"
            )))
        }
    };

    Ok(ConversationMessage {
        id: message.id,
        role,
        content: message.content,
        tool_call: message.tool_call,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            message.created_at,
            message.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
    })
}

fn map_rpe_to_domain(value: i32) -> Result<u8, WorkoutSummaryError> {
    u8::try_from(value)
        .ok()
        .filter(|value| (1..=10).contains(value))
        .ok_or_else(|| {
            WorkoutSummaryError::Repository(format!("invalid workout summary rpe: {value}"))
        })
}
