use std::sync::{Arc, Mutex};

use crate::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
        TrainingPlanSupervisorReview,
    },
};

#[derive(Clone, Default)]
pub(crate) struct InMemorySupervisorOperationRepository {
    stored: Arc<Mutex<Vec<TrainingPlanSupervisorOperation>>>,
}

impl TrainingPlanSupervisorOperationRepository for InMemorySupervisorOperationRepository {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        let stored = self.stored.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("supervisor operation repo mutex poisoned")
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned())
        })
    }

    fn upsert(
        &self,
        operation: TrainingPlanSupervisorOperation,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("supervisor operation repo mutex poisoned");
            stored
                .retain(|existing| existing.worker_operation_key != operation.worker_operation_key);
            stored.push(operation.clone());
            Ok(operation)
        })
    }

    fn complete_review_if_pending(
        &self,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let stored = self.stored.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("supervisor operation repo mutex poisoned");
            let existing = stored
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned()
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;
            let completed = existing.complete_review(review, now_epoch_seconds)?;
            stored
                .retain(|existing| existing.worker_operation_key != completed.worker_operation_key);
            stored.push(completed.clone());
            Ok(completed)
        })
    }
}
