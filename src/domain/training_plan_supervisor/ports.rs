use std::{future::Future, pin::Pin};

use crate::domain::training_plan::TrainingPlanError;

use super::TrainingPlanSupervisorOperation;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait TrainingPlanSupervisorOperationRepository: Clone + Send + Sync + 'static {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>;

    fn upsert(
        &self,
        operation: TrainingPlanSupervisorOperation,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>;
}

pub trait TrainingPlanSupervisorScheduler: Clone + Send + Sync + 'static {
    fn initialize_pending_review(
        &self,
        user_id: &str,
        worker_operation_key: &str,
        worker_saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>;
}
