use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::planned_completed_links::{
    BoxFuture as PlannedCompletedWorkoutLinkBoxFuture, PlannedCompletedWorkoutLink,
    PlannedCompletedWorkoutLinkError, PlannedCompletedWorkoutLinkMatchSource,
    PlannedCompletedWorkoutLinkRepository,
};

#[derive(Clone)]
pub struct MongoPlannedCompletedWorkoutLinkRepository {
    collection: Collection<PlannedCompletedWorkoutLinkDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PlannedCompletedWorkoutLinkDocument {
    user_id: String,
    planned_workout_id: String,
    completed_workout_id: String,
    match_source: String,
    matched_at_epoch_seconds: i64,
}

impl MongoPlannedCompletedWorkoutLinkRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("planned_completed_workout_links"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), PlannedCompletedWorkoutLinkError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "planned_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_completed_links_user_planned_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "completed_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_completed_links_user_completed_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl PlannedCompletedWorkoutLinkRepository for MongoPlannedCompletedWorkoutLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> PlannedCompletedWorkoutLinkBoxFuture<
        Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>,
    > {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "planned_workout_id": &planned_workout_id,
                })
                .await
                .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn find_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> PlannedCompletedWorkoutLinkBoxFuture<
        Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>,
    > {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "completed_workout_id": &completed_workout_id,
                })
                .await
                .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> PlannedCompletedWorkoutLinkBoxFuture<
        Result<PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError>,
    > {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&link);
        Box::pin(async move {
            collection
                .delete_many(doc! {
                    "user_id": &document.user_id,
                    "completed_workout_id": &document.completed_workout_id,
                    "planned_workout_id": { "$ne": &document.planned_workout_id },
                })
                .await
                .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "planned_workout_id": &document.planned_workout_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;
            Ok(link)
        })
    }

    fn delete_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> PlannedCompletedWorkoutLinkBoxFuture<Result<(), PlannedCompletedWorkoutLinkError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            collection
                .delete_many(doc! {
                    "user_id": &user_id,
                    "completed_workout_id": &completed_workout_id,
                })
                .await
                .map_err(|error| PlannedCompletedWorkoutLinkError::Repository(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_domain_to_document(
    link: &PlannedCompletedWorkoutLink,
) -> PlannedCompletedWorkoutLinkDocument {
    PlannedCompletedWorkoutLinkDocument {
        user_id: link.user_id.clone(),
        planned_workout_id: link.planned_workout_id.clone(),
        completed_workout_id: link.completed_workout_id.clone(),
        match_source: match_source_as_str(&link.match_source).to_string(),
        matched_at_epoch_seconds: link.matched_at_epoch_seconds,
    }
}

fn map_document_to_domain(
    document: PlannedCompletedWorkoutLinkDocument,
) -> Result<PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError> {
    Ok(PlannedCompletedWorkoutLink::new(
        document.user_id,
        document.planned_workout_id,
        document.completed_workout_id,
        match_source_from_str(&document.match_source)?,
        document.matched_at_epoch_seconds,
    ))
}

fn match_source_as_str(source: &PlannedCompletedWorkoutLinkMatchSource) -> &'static str {
    match source {
        PlannedCompletedWorkoutLinkMatchSource::Explicit => "explicit",
        PlannedCompletedWorkoutLinkMatchSource::Token => "token",
        PlannedCompletedWorkoutLinkMatchSource::Heuristic => "heuristic",
    }
}

fn match_source_from_str(
    value: &str,
) -> Result<PlannedCompletedWorkoutLinkMatchSource, PlannedCompletedWorkoutLinkError> {
    match value {
        "explicit" => Ok(PlannedCompletedWorkoutLinkMatchSource::Explicit),
        "token" => Ok(PlannedCompletedWorkoutLinkMatchSource::Token),
        "heuristic" => Ok(PlannedCompletedWorkoutLinkMatchSource::Heuristic),
        other => Err(PlannedCompletedWorkoutLinkError::Repository(format!(
            "unknown planned/completed match source: {other}",
        ))),
    }
}
