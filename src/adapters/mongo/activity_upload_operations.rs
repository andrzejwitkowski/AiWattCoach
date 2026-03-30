use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    ActivityUploadOperation, ActivityUploadOperationRepositoryPort, BoxFuture, IntervalsError,
};

#[derive(Clone)]
pub struct MongoActivityUploadOperationRepository {
    collection: Collection<ActivityUploadOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivityUploadOperationDocument {
    user_id: String,
    operation_key: String,
    payload: ActivityUploadOperation,
}

impl MongoActivityUploadOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("intervals_activity_upload_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IntervalsError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1, "operation_key": 1 })
                .options(
                    IndexOptions::builder()
                        .name("intervals_activity_upload_operations_user_key_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?;
        Ok(())
    }
}

impl ActivityUploadOperationRepositoryPort for MongoActivityUploadOperationRepository {
    fn find_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<ActivityUploadOperation>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let result = collection
                .find_one(doc! { "user_id": &user_id, "operation_key": &operation_key })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(result.map(|document| document.payload))
        })
    }

    fn upsert(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperation, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = ActivityUploadOperationDocument {
                user_id: user_id.clone(),
                operation_key: operation.operation_key.clone(),
                payload: operation.clone(),
            };
            collection
                .replace_one(
                    doc! { "user_id": &user_id, "operation_key": &document.operation_key },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(operation)
        })
    }
}
