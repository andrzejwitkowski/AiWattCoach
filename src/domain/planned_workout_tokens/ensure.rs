use super::{
    build_planned_workout_match_token, format_planned_workout_marker, PlannedWorkoutToken,
    PlannedWorkoutTokenError, PlannedWorkoutTokenRepository,
};

/// Look up or create a planned-workout match token and return the formatted marker.
///
/// Returns the raw [`PlannedWorkoutTokenError`] on repository failure; callers map
/// it to their adapter-specific error type.
pub async fn ensure_planned_workout_marker<Tokens>(
    tokens: &Tokens,
    user_id: &str,
    planned_workout_id: &str,
) -> Result<String, PlannedWorkoutTokenError>
where
    Tokens: PlannedWorkoutTokenRepository,
{
    let match_token = match tokens
        .find_by_planned_workout_id(user_id, planned_workout_id)
        .await?
    {
        Some(token) => token.match_token,
        None => {
            let match_token = build_planned_workout_match_token(planned_workout_id);
            tokens
                .upsert(PlannedWorkoutToken::new(
                    user_id.to_string(),
                    planned_workout_id.to_string(),
                    match_token.clone(),
                ))
                .await?;
            match_token
        }
    };

    Ok(format_planned_workout_marker(&match_token))
}
