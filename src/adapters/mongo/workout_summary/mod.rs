use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};

use crate::domain::workout_summary::WorkoutSummaryError;

mod document;
mod lookup;
mod mapping;
mod repository;

use document::WorkoutSummaryDocument;

#[derive(Clone)]
pub struct MongoWorkoutSummaryRepository {
    collection: Collection<WorkoutSummaryDocument>,
}

impl MongoWorkoutSummaryRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("workout_summaries"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WorkoutSummaryError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("workout_summaries_user_workout_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "event_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("workout_summaries_user_event".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
