use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::special_days::{
    BoxFuture as SpecialDayBoxFuture, SpecialDay, SpecialDayError, SpecialDayKind,
    SpecialDayRepository,
};

#[derive(Clone)]
pub struct MongoSpecialDayRepository {
    collection: Collection<SpecialDayDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpecialDayDocument {
    user_id: String,
    special_day_id: String,
    date: String,
    kind: String,
    title: Option<String>,
    description: Option<String>,
}

impl MongoSpecialDayRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("special_days"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), SpecialDayError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "special_day_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("special_days_user_special_day_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("special_days_user_date".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| SpecialDayError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl SpecialDayRepository for MongoSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! { "user_id": &user_id })
                .sort(doc! { "date": 1, "special_day_id": 1 })
                .await
                .map_err(|error| SpecialDayError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| SpecialDayError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_special_day)
                .collect()
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> SpecialDayBoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": {
                        "$gte": &oldest,
                        "$lte": &newest,
                    },
                })
                .sort(doc! { "date": 1, "special_day_id": 1 })
                .await
                .map_err(|error| SpecialDayError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| SpecialDayError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_special_day)
                .collect()
        })
    }

    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> SpecialDayBoxFuture<Result<SpecialDay, SpecialDayError>> {
        let collection = self.collection.clone();
        let document = map_special_day_to_document(&special_day);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "special_day_id": &document.special_day_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| SpecialDayError::Repository(error.to_string()))?;
            Ok(special_day)
        })
    }
}

fn map_special_day_to_document(special_day: &SpecialDay) -> SpecialDayDocument {
    SpecialDayDocument {
        user_id: special_day.user_id.clone(),
        special_day_id: special_day.special_day_id.clone(),
        date: special_day.date.clone(),
        kind: map_kind_to_str(&special_day.kind).to_string(),
        title: special_day.title.clone(),
        description: special_day.description.clone(),
    }
}

fn map_document_to_special_day(
    document: SpecialDayDocument,
) -> Result<SpecialDay, SpecialDayError> {
    SpecialDay::new(
        document.special_day_id,
        document.user_id,
        document.date,
        map_kind_from_str(&document.kind)?,
        document.title,
        document.description,
    )
}

fn map_kind_to_str(kind: &SpecialDayKind) -> &'static str {
    match kind {
        SpecialDayKind::Illness => "illness",
        SpecialDayKind::Travel => "travel",
        SpecialDayKind::Blocked => "blocked",
        SpecialDayKind::Note => "note",
        SpecialDayKind::Other => "other",
    }
}

fn map_kind_from_str(value: &str) -> Result<SpecialDayKind, SpecialDayError> {
    match value {
        "illness" => Ok(SpecialDayKind::Illness),
        "travel" => Ok(SpecialDayKind::Travel),
        "blocked" => Ok(SpecialDayKind::Blocked),
        "note" => Ok(SpecialDayKind::Note),
        "other" => Ok(SpecialDayKind::Other),
        other => Err(SpecialDayError::Repository(format!(
            "unknown special day kind: {other}"
        ))),
    }
}
