use std::sync::{Arc, Mutex};

use crate::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorBatchRequest,
        TrainingPlanSupervisorBatchSubmission, TrainingPlanSupervisorReview,
    },
};

#[derive(Clone, Default)]
pub(crate) struct RecordingBatchPort {
    requests: Arc<Mutex<Vec<TrainingPlanSupervisorBatchRequest>>>,
}

impl RecordingBatchPort {
    pub(crate) fn requests(&self) -> Vec<TrainingPlanSupervisorBatchRequest> {
        self.requests
            .lock()
            .expect("batch requests mutex poisoned")
            .clone()
    }
}

impl TrainingPlanSupervisorBatchPort for RecordingBatchPort {
    fn submit_review(
        &self,
        _api_key: &str,
        request: TrainingPlanSupervisorBatchRequest,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<TrainingPlanSupervisorBatchSubmission, TrainingPlanError>,
    > {
        let requests = self.requests.clone();
        Box::pin(async move {
            requests
                .lock()
                .expect("batch requests mutex poisoned")
                .push(request);
            Ok(TrainingPlanSupervisorBatchSubmission {
                batch_name: "batches/supervisor-1".to_string(),
            })
        })
    }

    fn download_result(
        &self,
        _api_key: &str,
        _batch_name: &str,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<TrainingPlanSupervisorReview, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Unavailable(
                "download_result not implemented in test".to_string(),
            ))
        })
    }
}
