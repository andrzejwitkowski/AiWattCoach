use super::WorkoutSummary;

#[derive(Clone, Default)]
pub struct MockWorkoutCoach;

impl MockWorkoutCoach {
    pub fn reply(&self, summary: &WorkoutSummary, user_message: &str) -> String {
        match summary.rpe {
            Some(rpe) if rpe >= 8 => format!(
                "That sounds like a hard session. With an RPE of {rpe}, what part of the workout drove the most fatigue?"
            ),
            Some(rpe) if rpe <= 4 => format!(
                "That sounds pretty controlled. With an RPE of {rpe}, do you think the workout felt easier than planned?"
            ),
            _ if user_message.to_ascii_lowercase().contains("heavy") => {
                "Heavy legs can come from accumulated fatigue. Did the effort improve after the warmup, or stay flat the whole session?".to_string()
            }
            _ => {
                "Thanks, that helps. What stood out most about how the workout felt compared with the plan?"
                    .to_string()
            }
        }
    }
}
