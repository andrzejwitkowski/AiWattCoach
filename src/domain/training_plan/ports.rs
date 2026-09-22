use std::{future::Future, pin::Pin, sync::Arc};

use crate::domain::workout_summary::WorkoutRecap;
use crate::domain::{ai_workflow::ValidationIssue, llm_tools::LlmToolLoopState};

use super::{
    PlanQualityEvaluation, PlanQualityEvaluationInput, TrainingPlanError,
    TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation, TrainingPlanPhaseOutput,
    TrainingPlanPlanningContext, TrainingPlanProjectedDay, TrainingPlanReplacementResult,
    TrainingPlanSnapshot,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type TrainingPlanToolLoopCheckpoint =
    Arc<dyn Fn(LlmToolLoopState) -> BoxFuture<Result<(), TrainingPlanError>> + Send + Sync>;

pub trait TrainingPlanSnapshotRepository: Send + Sync + 'static {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSnapshot>, TrainingPlanError>>;
}

pub trait TrainingPlanProjectionRepository: Send + Sync + 'static {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>>;

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>>;

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>>;

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>>;

    fn supersede_active_dates(
        &self,
        _user_id: &str,
        _dates: &[String],
        _superseded_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<(String, String)>, TrainingPlanError>> {
        Box::pin(async move { Ok(None) })
    }

    /// Moves an active projected day from `from_date` to `to_date`.
    /// ponytail: keeps sync by operation_key+date working after calendar move.
    fn relocate_active_date(
        &self,
        _user_id: &str,
        _from_date: &str,
        _to_date: &str,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async move { Ok(None) })
    }
}

pub trait TrainingPlanGenerationOperationRepository: Send + Sync + 'static {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>>;

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanGenerationClaimResult, TrainingPlanError>>;

    fn upsert(
        &self,
        operation: TrainingPlanGenerationOperation,
    ) -> BoxFuture<Result<TrainingPlanGenerationOperation, TrainingPlanError>>;

    fn find_latest_completed_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>>;
}

pub trait TrainingPlanGenerator: Send + Sync + 'static {
    fn generate_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>>;

    #[expect(
        clippy::too_many_arguments,
        reason = "training plan initial generation needs workout identity, recap context, planning context, restore state, checkpoint callback, and optional quality feedback together"
    )]
    fn generate_initial_plan_window_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
        quality_feedback: Option<&str>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>>;

    fn generate_initial_plan_window(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        self.generate_initial_plan_window_with_state(
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            workout_recap,
            planning_context,
            None,
            None,
            None,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "training plan correction needs workout identity, recap context, planning context, and validation payload together"
    )]
    fn correct_invalid_days_with_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<TrainingPlanToolLoopCheckpoint>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>>;

    #[expect(
        clippy::too_many_arguments,
        reason = "training plan correction needs workout identity, recap context, planning context, and validation payload together"
    )]
    fn correct_invalid_days(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        planning_context: Option<&TrainingPlanPlanningContext>,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
    ) -> BoxFuture<Result<TrainingPlanPhaseOutput, TrainingPlanError>> {
        self.correct_invalid_days_with_state(
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            workout_recap,
            planning_context,
            invalid_day_sections,
            issues,
            None,
            None,
        )
    }

    fn evaluate_plan_quality(
        &self,
        input: PlanQualityEvaluationInput<'_>,
    ) -> BoxFuture<Result<PlanQualityEvaluation, TrainingPlanError>>;

    fn plan_quality_availability_summary(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> BoxFuture<Result<String, TrainingPlanError>> {
        Box::pin(async move { Ok("availability: not configured".to_string()) })
    }

    fn plan_target_event_requirement(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> BoxFuture<
        Result<Option<crate::domain::training_plan::TargetEventRequirement>, TrainingPlanError>,
    > {
        Box::pin(async move { Ok(None) })
    }
}

pub trait TrainingPlanWorkoutSummaryPort: Send + Sync + 'static {
    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<(), TrainingPlanError>>;

    fn get_planning_context(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanPlanningContext>, TrainingPlanError>>;
}

pub trait WorkoutPlanningLlmConfigPort: Send + Sync + 'static {
    fn get_workout_planning_config(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<crate::domain::llm::LlmProviderConfig, TrainingPlanError>>;
}

pub trait PlanQualityEvaluatorLlmConfigPort: Send + Sync + 'static {
    fn get_plan_quality_evaluator_config(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<crate::domain::llm::LlmProviderConfig, TrainingPlanError>>;

    fn get_plan_quality_max_loops(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<u32, TrainingPlanError>>;

    fn get_plan_quality_pass_score(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<u8, TrainingPlanError>>;
}

pub trait PlanQualityProgressPort: Send + Sync + 'static {
    fn on_quality_progress(&self, user_id: &str, workout_id: &str, message: String);
}
