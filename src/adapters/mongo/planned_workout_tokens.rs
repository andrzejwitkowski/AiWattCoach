use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::planned_workout_tokens::{
    BoxFuture as PlannedWorkoutTokenBoxFuture, PlannedWorkoutToken, PlannedWorkoutTokenError,
    PlannedWorkoutTokenRepository,
};

#[derive(Clone)]
pub struct MongoPlannedWorkoutTokenRepository {
    collection: Collection<PlannedWorkoutTokenDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PlannedWorkoutTokenDocument {
    user_id: String,
    planned_workout_id: String,
    match_token: String,
}

impl MongoPlannedWorkoutTokenRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("planned_workout_tokens"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), PlannedWorkoutTokenError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "planned_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_tokens_user_planned_workout_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "match_token": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_workout_tokens_user_match_token_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| PlannedWorkoutTokenError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl PlannedWorkoutTokenRepository for MongoPlannedWorkoutTokenRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> PlannedWorkoutTokenBoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>>
    {
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
                .map_err(|error| PlannedWorkoutTokenError::Repository(error.to_string()))?;
            Ok(document.map(map_document_to_domain))
        })
    }

    fn find_by_match_token(
        &self,
        user_id: &str,
        match_token: &str,
    ) -> PlannedWorkoutTokenBoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>>
    {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let match_token = match_token.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "match_token": &match_token,
                })
                .await
                .map_err(|error| PlannedWorkoutTokenError::Repository(error.to_string()))?;
            Ok(document.map(map_document_to_domain))
        })
    }

    fn upsert(
        &self,
        token: PlannedWorkoutToken,
    ) -> PlannedWorkoutTokenBoxFuture<Result<PlannedWorkoutToken, PlannedWorkoutTokenError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&token);
        Box::pin(async move {
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
                .map_err(|error| PlannedWorkoutTokenError::Repository(error.to_string()))?;
            Ok(token)
        })
    }
}

fn map_domain_to_document(token: &PlannedWorkoutToken) -> PlannedWorkoutTokenDocument {
    PlannedWorkoutTokenDocument {
        user_id: token.user_id.clone(),
        planned_workout_id: token.planned_workout_id.clone(),
        match_token: token.match_token.clone(),
    }
}

fn map_document_to_domain(document: PlannedWorkoutTokenDocument) -> PlannedWorkoutToken {
    PlannedWorkoutToken::new(
        document.user_id,
        document.planned_workout_id,
        document.match_token,
    )
}
