use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};

use crate::domain::workout_summary::WorkoutSummaryError;

mod document;
mod mapping;
mod repository;

use document::CoachReplyOperationDocument;

#[derive(Clone)]
pub struct MongoCoachReplyOperationRepository {
    collection: Collection<CoachReplyOperationDocument>,
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
