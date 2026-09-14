use crate::domain::{training_plan::TrainingPlanPlanningContext, workout_summary::WorkoutRecap};

pub(super) struct GenerationIdentity<'a> {
    pub user_id: &'a str,
    pub workout_id: &'a str,
    pub saved_at_epoch_seconds: i64,
    pub recap: &'a WorkoutRecap,
}

pub(super) struct GenerationPlanning<'a> {
    pub planning_context: &'a mut Option<TrainingPlanPlanningContext>,
    pub planning_context_loaded: &'a mut bool,
}
