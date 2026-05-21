use std::{future::Future, pin::Pin};

use crate::domain::training_plan::TrainingPlanError;

use super::{TrainingPlanSupervisorOperation, TrainingPlanSupervisorReview};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorBatchRequest {
    pub user_id: String,
    pub worker_operation_key: String,
    pub model: String,
    pub original_plan: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorBatchSubmission {
    pub batch_name: String,
}

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait TrainingPlanSupervisorOperationRepository: Clone + Send + Sync + 'static {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>;

    fn complete_review_if_pending(
        &self,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>>;

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
        original_plan: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>;
}

pub trait TrainingPlanSupervisorBatchPort: Clone + Send + Sync + 'static {
    fn submit_review(
        &self,
        api_key: &str,
        request: TrainingPlanSupervisorBatchRequest,
    ) -> BoxFuture<Result<TrainingPlanSupervisorBatchSubmission, TrainingPlanError>>;

    fn download_result(
        &self,
        api_key: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<TrainingPlanSupervisorReview, TrainingPlanError>>;
}

impl<T> TrainingPlanSupervisorBatchPort for std::sync::Arc<T>
where
    T: TrainingPlanSupervisorBatchPort,
{
    fn submit_review(
        &self,
        api_key: &str,
        request: TrainingPlanSupervisorBatchRequest,
    ) -> BoxFuture<Result<TrainingPlanSupervisorBatchSubmission, TrainingPlanError>> {
        self.as_ref().submit_review(api_key, request)
    }

    fn download_result(
        &self,
        api_key: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<TrainingPlanSupervisorReview, TrainingPlanError>> {
        self.as_ref().download_result(api_key, batch_name)
    }
}
