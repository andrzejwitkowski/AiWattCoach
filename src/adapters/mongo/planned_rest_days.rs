use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::{
    intervals::DateRange,
    planned_rest_days::{
        BoxFuture as PlannedRestDayBoxFuture, PlannedRestDay, PlannedRestDayError,
        PlannedRestDayRepository,
    },
};

#[derive(Clone)]
pub struct MongoPlannedRestDayRepository {
    collection: Collection<PlannedRestDayDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PlannedRestDayDocument {
    user_id: String,
    planned_rest_day_id: String,
    start_date: String,
    end_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoPlannedRestDayRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("planned_rest_days"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), PlannedRestDayError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "planned_rest_day_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_rest_days_user_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "start_date": 1, "end_date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("planned_rest_days_user_date_range".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?;

        Ok(())
    }
}

impl PlannedRestDayRepository for MongoPlannedRestDayRepository {
    fn list_intersecting_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> PlannedRestDayBoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "start_date": { "$lte": &range.newest },
                    "end_date": { "$gte": &range.oldest },
                })
                .sort(doc! { "start_date": 1, "planned_rest_day_id": 1 })
                .await
                .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?
                .into_iter()
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn find_by_id(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> PlannedRestDayBoxFuture<Result<Option<PlannedRestDay>, PlannedRestDayError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "planned_rest_day_id": &planned_rest_day_id,
                })
                .await
                .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn upsert(
        &self,
        entry: PlannedRestDay,
    ) -> PlannedRestDayBoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&entry);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &entry.user_id,
                        "planned_rest_day_id": &entry.planned_rest_day_id,
                    },
                    document,
                )
                .with_options(
                    mongodb::options::ReplaceOptions::builder()
                        .upsert(true)
                        .build(),
                )
                .await
                .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?;

            Ok(entry)
        })
    }

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> PlannedRestDayBoxFuture<Result<(), PlannedRestDayError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! {
                    "user_id": &user_id,
                    "planned_rest_day_id": &planned_rest_day_id,
                })
                .await
                .map_err(|error| PlannedRestDayError::Internal(error.to_string()))?;

            Ok(())
        })
    }
}

fn map_document_to_domain(
    document: PlannedRestDayDocument,
) -> Result<PlannedRestDay, PlannedRestDayError> {
    PlannedRestDay::new(
        document.planned_rest_day_id,
        document.user_id,
        document.start_date,
        document.end_date,
        document.title,
        document.note,
        document.created_at_epoch_seconds,
        document.updated_at_epoch_seconds,
    )
}

fn map_domain_to_document(entry: &PlannedRestDay) -> PlannedRestDayDocument {
    PlannedRestDayDocument {
        user_id: entry.user_id.clone(),
        planned_rest_day_id: entry.planned_rest_day_id.clone(),
        start_date: entry.start_date.clone(),
        end_date: entry.end_date.clone(),
        title: entry.title.clone(),
        note: entry.note.clone(),
        created_at_epoch_seconds: entry.created_at_epoch_seconds,
        updated_at_epoch_seconds: entry.updated_at_epoch_seconds,
    }
}
