use crate::domain::{
    calendar_view::CalendarEntryViewError, completed_workouts::CompletedWorkoutError,
    external_sync::ExternalSyncRepositoryError, intervals::IntervalsError,
    planned_workout_tokens::PlannedWorkoutTokenError, settings::SettingsError,
    training_plan::TrainingPlanError, wahoo::WahooError,
};

use crate::domain::calendar::CalendarError;

pub(super) fn map_training_plan_error(error: TrainingPlanError) -> CalendarError {
    match error {
        TrainingPlanError::Validation(message) => CalendarError::Validation(message),
        TrainingPlanError::Unavailable(message) => CalendarError::Unavailable(message),
        TrainingPlanError::Repository(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_planned_workout_token_error(error: PlannedWorkoutTokenError) -> CalendarError {
    match error {
        PlannedWorkoutTokenError::Repository(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_calendar_entry_view_error(error: CalendarEntryViewError) -> CalendarError {
    match error {
        CalendarEntryViewError::Repository(message) => CalendarError::Internal(message),
        CalendarEntryViewError::InvariantViolation(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_completed_workout_error(error: CompletedWorkoutError) -> CalendarError {
    match error {
        CompletedWorkoutError::Repository(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_intervals_error(error: IntervalsError) -> CalendarError {
    match error {
        IntervalsError::Unauthenticated => CalendarError::Unauthenticated,
        IntervalsError::CredentialsNotConfigured => CalendarError::CredentialsNotConfigured,
        IntervalsError::ApiError(message) | IntervalsError::ConnectionError(message) => {
            CalendarError::Unavailable(message)
        }
        IntervalsError::NotFound => CalendarError::NotFound,
        IntervalsError::Internal(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_wahoo_error(error: WahooError) -> CalendarError {
    match error {
        WahooError::Unauthenticated => CalendarError::Unauthenticated,
        WahooError::NotConnected => CalendarError::CredentialsNotConfigured,
        WahooError::NotFound => CalendarError::NotFound,
        WahooError::InvalidConnectState => {
            CalendarError::Unavailable("Wahoo connect state is invalid or expired".to_string())
        }
        WahooError::Repository(message) => CalendarError::Internal(message),
        WahooError::External(message) => CalendarError::Unavailable(message),
    }
}

pub(super) fn map_external_sync_error(error: ExternalSyncRepositoryError) -> CalendarError {
    match error {
        ExternalSyncRepositoryError::Storage(message)
        | ExternalSyncRepositoryError::CorruptData(message) => CalendarError::Internal(message),
    }
}

pub(super) fn map_settings_error(error: SettingsError) -> CalendarError {
    match error {
        SettingsError::Unauthenticated => CalendarError::Unauthenticated,
        SettingsError::Repository(message) => CalendarError::Internal(message),
        SettingsError::Validation(message) => CalendarError::Validation(message),
    }
}
