use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::{
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
        TrainingPlanSupervisorStatus,
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
    status: String,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
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
}

fn map_operation_to_document(
    operation: &TrainingPlanSupervisorOperation,
) -> TrainingPlanSupervisorOperationDocument {
    TrainingPlanSupervisorOperationDocument {
        worker_operation_key: operation.worker_operation_key.clone(),
        user_id: operation.user_id.clone(),
        worker_saved_at_epoch_seconds: operation.worker_saved_at_epoch_seconds,
        model: operation.model.clone(),
        status: operation.status.as_str().to_string(),
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
        status: TrainingPlanSupervisorStatus::try_from(document.status.as_str())
            .map_err(TrainingPlanError::Repository)?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
