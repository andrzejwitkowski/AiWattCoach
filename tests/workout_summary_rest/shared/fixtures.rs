use aiwattcoach::domain::workout_summary::WorkoutSummary;

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    sample_summary_for_user("user-1", workout_id)
}

pub(crate) fn existing_summary() -> WorkoutSummary {
    sample_summary("workout-1")
}

pub(crate) fn sample_summary_with_updated_at(
    workout_id: &str,
    updated_at_epoch_seconds: i64,
) -> WorkoutSummary {
    let mut summary = sample_summary(workout_id);
    summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
    summary
}

pub(crate) fn sample_summary_for_user(user_id: &str, workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}
