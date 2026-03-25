use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    Activity, ActivityRepositoryPort, BoxFuture, DateRange, IntervalsError,
};

#[derive(Clone)]
pub struct MongoActivityRepository {
    collection: Collection<ActivityDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ActivityDocument {
    user_id: String,
    activity_id: String,
    start_date_local: String,
    payload: Activity,
}

impl MongoActivityRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("intervals_activities"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IntervalsError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "activity_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_activity_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "start_date_local": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("intervals_activities_user_start_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?;
        Ok(())
    }
}

impl ActivityRepositoryPort for MongoActivityRepository {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = ActivityDocument {
                user_id: user_id.clone(),
                activity_id: activity.id.clone(),
                start_date_local: activity.start_date_local.clone(),
                payload: activity.clone(),
            };
            collection
                .replace_one(
                    doc! { "user_id": &user_id, "activity_id": &document.activity_id },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = Vec::with_capacity(activities.len());
            for activity in activities {
                stored.push(repository.upsert(&user_id, activity).await?);
            }
            Ok(stored)
        })
    }

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let mut cursor = collection
                .find(doc! {
                    "user_id": &user_id,
                    "start_date_local": { "$gte": &range.oldest, "$lte": &range.newest }
                })
                .sort(doc! { "start_date_local": -1 })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;

            let mut activities = Vec::new();
            while cursor
                .advance()
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?
            {
                let document = cursor
                    .deserialize_current()
                    .map_err(|error| IntervalsError::Internal(error.to_string()))?;
                activities.push(document.payload);
            }
            Ok(activities)
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let result = collection
                .find_one(doc! { "user_id": &user_id, "activity_id": &activity_id })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(result.map(|document| document.payload))
        })
    }

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "user_id": &user_id, "activity_id": &activity_id })
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}
