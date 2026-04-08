use std::{future::Future, pin::Pin};

use crate::domain::ai_workflow::ValidationIssue;
use crate::domain::workout_summary::WorkoutRecap;

use super::{
    TrainingPlanError, TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
    TrainingPlanProjectedDay, TrainingPlanSnapshot,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

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
    ) -> BoxFuture<Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>>;
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
}

pub trait TrainingPlanGenerator: Send + Sync + 'static {
    fn generate_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>>;

    fn generate_initial_plan_window(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
    ) -> BoxFuture<Result<String, TrainingPlanError>>;

    fn correct_invalid_days(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
        workout_recap: &WorkoutRecap,
        invalid_day_sections: &str,
        issues: Vec<ValidationIssue>,
    ) -> BoxFuture<Result<String, TrainingPlanError>>;
}

pub trait TrainingPlanWorkoutSummaryPort: Send + Sync + 'static {
    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<(), TrainingPlanError>>;
}
