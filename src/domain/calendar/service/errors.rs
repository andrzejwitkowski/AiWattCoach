use crate::domain::{
    calendar_view::CalendarEntryViewError, completed_workouts::CompletedWorkoutError,
    intervals::IntervalsError, planned_workout_tokens::PlannedWorkoutTokenError,
    training_plan::TrainingPlanError,
};

use crate::domain::calendar::CalendarError;

pub(super) fn map_intervals_error(error: IntervalsError) -> CalendarError {
    match error {
        IntervalsError::NotFound => CalendarError::NotFound,
        IntervalsError::Unauthenticated => CalendarError::Unauthenticated,
        IntervalsError::CredentialsNotConfigured => CalendarError::CredentialsNotConfigured,
        IntervalsError::ApiError(message)
        | IntervalsError::ConnectionError(message)
        | IntervalsError::Internal(message) => CalendarError::Unavailable(message),
    }
}

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
    }
}

pub(super) fn map_completed_workout_error(error: CompletedWorkoutError) -> CalendarError {
    match error {
        CompletedWorkoutError::Repository(message) => CalendarError::Internal(message),
    }
}
