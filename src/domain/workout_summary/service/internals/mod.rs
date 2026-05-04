use super::*;

mod messaging;
mod presentation;
mod recovery;
mod target_resolution;

pub(super) struct AppendMessageInput {
    role: MessageRole,
    content: String,
    message_id: Option<String>,
    tool_call: Option<crate::domain::workout_summary::PublicToolCall>,
    require_open_summary: bool,
}

impl AppendMessageInput {
    pub(super) fn coach(content: String, message_id: String) -> Self {
        Self {
            role: MessageRole::Coach,
            content,
            message_id: Some(message_id),
            tool_call: None,
            require_open_summary: false,
        }
    }
}

fn map_settings_error(error: crate::domain::settings::SettingsError) -> WorkoutSummaryError {
    match error {
        crate::domain::settings::SettingsError::Repository(message) => {
            WorkoutSummaryError::Repository(message)
        }
        crate::domain::settings::SettingsError::Unauthenticated => {
            WorkoutSummaryError::Validation("authentication is required".to_string())
        }
        crate::domain::settings::SettingsError::Validation(message) => {
            WorkoutSummaryError::Validation(message)
        }
    }
}

fn push_unique_workout_id(workout_ids: &mut Vec<String>, workout_id: String) {
    if !workout_ids.contains(&workout_id) {
        workout_ids.push(workout_id);
    }
}
