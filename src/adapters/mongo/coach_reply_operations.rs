use super::error::is_duplicate_key_error;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::workout_summary::{
    BoxFuture, CoachReplyOperation, CoachReplyOperationClaimResult, CoachReplyOperationRepository,
    WorkoutSummaryError,
};

#[derive(Clone)]
pub struct MongoCoachReplyOperationRepository {
    collection: Collection<CoachReplyOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CoachReplyOperationDocument {
    user_id: String,
    workout_id: String,
    user_message_id: String,
    payload: CoachReplyOperation,
}

impl MongoCoachReplyOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("coach_reply_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WorkoutSummaryError> {
        self.collection
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1, "workout_id": 1, "user_message_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("coach_reply_operations_user_workout_message_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl CoachReplyOperationRepository for MongoCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "workout_id": &workout_id,
                    "user_message_id": &user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            Ok(document.map(|document| document.payload))
        })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyOperationClaimResult, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = CoachReplyOperationDocument {
                user_id: operation.user_id.clone(),
                workout_id: operation.workout_id.clone(),
                user_message_id: operation.user_message_id.clone(),
                payload: operation.clone(),
            };

            let reclaimed = collection
                .find_one_and_replace(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                        "payload.status": "Failed",
                    },
                    &document,
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if reclaimed.is_some() {
                return Ok(CoachReplyOperationClaimResult::Claimed(operation));
            }

            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(WorkoutSummaryError::Repository(error.to_string()))
                    }
                })?;

            if inserted {
                return Ok(CoachReplyOperationClaimResult::Claimed(operation));
            }

            let existing = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "workout_id": &document.workout_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "claimed coach reply operation disappeared before reload".to_string(),
                    )
                })?;

            Ok(CoachReplyOperationClaimResult::Existing(existing.payload))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = CoachReplyOperationDocument {
                user_id: operation.user_id.clone(),
                workout_id: operation.workout_id.clone(),
                user_message_id: operation.user_message_id.clone(),
                payload: operation.clone(),
            };

            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            Ok(operation)
        })
    }
}
