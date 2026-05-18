use mongodb::{
    bson::doc,
    options::{IndexOptions, ReturnDocument},
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorDecision, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReplacementApplyResult,
        TrainingPlanSupervisorReview, TrainingPlanSupervisorStatus,
    },
};

#[derive(Clone)]
pub struct MongoTrainingPlanSupervisorOperationRepository {
    collection: Collection<TrainingPlanSupervisorOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanSupervisorOperationDocument {
    worker_operation_key: String,
    user_id: String,
    worker_saved_at_epoch_seconds: i64,
    model: String,
    #[serde(default)]
    batch_name: Option<String>,
    #[serde(default)]
    batch_submitted_at_epoch_seconds: Option<i64>,
    status: String,
    #[serde(default)]
    review: Option<TrainingPlanSupervisorReviewDocument>,
    #[serde(default)]
    replacement_apply_result: Option<TrainingPlanSupervisorReplacementApplyResultDocument>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanSupervisorReviewDocument {
    decision: String,
    reason: String,
    #[serde(default)]
    plan: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanSupervisorReplacementApplyResultDocument {
    applied_dates: Vec<String>,
    #[serde(default)]
    skipped_dates: Vec<String>,
    skipped_synced_dates: Vec<String>,
    applied_at_epoch_seconds: i64,
}

impl MongoTrainingPlanSupervisorOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_plan_supervisor_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TrainingPlanError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "worker_operation_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "training_plan_supervisor_operations_worker_operation_key_unique"
                                    .to_string(),
                            )
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "worker_saved_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_plan_supervisor_operations_user_saved_at".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl TrainingPlanSupervisorOperationRepository for MongoTrainingPlanSupervisorOperationRepository {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "worker_operation_key": &worker_operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn upsert(
        &self,
        operation: TrainingPlanSupervisorOperation,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation);
            collection
                .replace_one(
                    doc! { "worker_operation_key": &document.worker_operation_key },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            Ok(operation)
        })
    }

    fn complete_review_if_pending(
        &self,
        worker_operation_key: &str,
        review: TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let collection = self.collection.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            review.validate()?;
            let status = review.decision.terminal_status().as_str().to_string();
            let review_document = map_review_to_document(&review);

            if let Some(updated) = collection
                .find_one_and_update(
                    doc! {
                        "worker_operation_key": &worker_operation_key,
                        "status": TrainingPlanSupervisorStatus::Pending.as_str(),
                    },
                    doc! {
                        "$set": {
                            "status": &status,
                            "review": mongodb::bson::to_bson(&review_document)
                                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?,
                            "updated_at_epoch_seconds": now_epoch_seconds,
                        },
                    },
                )
                .return_document(ReturnDocument::After)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
            {
                return map_document_to_operation(updated);
            }

            let existing = collection
                .find_one(doc! { "worker_operation_key": &worker_operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;

            map_document_to_operation(existing)?.complete_review(review, now_epoch_seconds)
        })
    }
}

fn map_operation_to_document(
    operation: &TrainingPlanSupervisorOperation,
) -> TrainingPlanSupervisorOperationDocument {
    TrainingPlanSupervisorOperationDocument {
        worker_operation_key: operation.worker_operation_key.clone(),
        user_id: operation.user_id.clone(),
        worker_saved_at_epoch_seconds: operation.worker_saved_at_epoch_seconds,
        model: operation.model.clone(),
        batch_name: operation.batch_name.clone(),
        batch_submitted_at_epoch_seconds: operation.batch_submitted_at_epoch_seconds,
        status: operation.status.as_str().to_string(),
        review: operation.review.as_ref().map(map_review_to_document),
        replacement_apply_result: operation
            .replacement_apply_result
            .as_ref()
            .map(map_replacement_apply_result_to_document),
        created_at_epoch_seconds: operation.created_at_epoch_seconds,
        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
    }
}

fn map_document_to_operation(
    document: TrainingPlanSupervisorOperationDocument,
) -> Result<TrainingPlanSupervisorOperation, TrainingPlanError> {
    Ok(TrainingPlanSupervisorOperation {
        worker_operation_key: document.worker_operation_key,
        user_id: document.user_id,
        worker_saved_at_epoch_seconds: document.worker_saved_at_epoch_seconds,
        model: document.model,
        batch_name: document.batch_name,
        batch_submitted_at_epoch_seconds: document.batch_submitted_at_epoch_seconds,
        status: TrainingPlanSupervisorStatus::try_from(document.status.as_str())
            .map_err(TrainingPlanError::Repository)?,
        review: document.review.map(map_document_to_review).transpose()?,
        replacement_apply_result: document
            .replacement_apply_result
            .map(map_document_to_replacement_apply_result),
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}

fn map_review_to_document(
    review: &TrainingPlanSupervisorReview,
) -> TrainingPlanSupervisorReviewDocument {
    TrainingPlanSupervisorReviewDocument {
        decision: review.decision.as_str().to_string(),
        reason: review.reason.clone(),
        plan: review.plan.clone(),
    }
}

fn map_document_to_review(
    document: TrainingPlanSupervisorReviewDocument,
) -> Result<TrainingPlanSupervisorReview, TrainingPlanError> {
    Ok(TrainingPlanSupervisorReview {
        decision: TrainingPlanSupervisorDecision::try_from(document.decision.as_str())
            .map_err(TrainingPlanError::Repository)?,
        reason: document.reason,
        plan: document.plan,
    })
}

fn map_replacement_apply_result_to_document(
    result: &TrainingPlanSupervisorReplacementApplyResult,
) -> TrainingPlanSupervisorReplacementApplyResultDocument {
    TrainingPlanSupervisorReplacementApplyResultDocument {
        applied_dates: result.applied_dates.clone(),
        skipped_dates: result.skipped_dates.clone(),
        skipped_synced_dates: result.skipped_synced_dates.clone(),
        applied_at_epoch_seconds: result.applied_at_epoch_seconds,
    }
}

fn map_document_to_replacement_apply_result(
    document: TrainingPlanSupervisorReplacementApplyResultDocument,
) -> TrainingPlanSupervisorReplacementApplyResult {
    TrainingPlanSupervisorReplacementApplyResult {
        applied_dates: document.applied_dates,
        skipped_dates: document.skipped_dates,
        skipped_synced_dates: document.skipped_synced_dates,
        applied_at_epoch_seconds: document.applied_at_epoch_seconds,
    }
}
