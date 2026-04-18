use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::training_load::{
    BoxFuture, FtpHistoryEntry, FtpHistoryRepository, FtpSource, TrainingLoadError,
};

#[derive(Clone)]
pub struct MongoFtpHistoryRepository {
    collection: Collection<FtpHistoryDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FtpHistoryDocument {
    user_id: String,
    effective_from_date: String,
    ftp_watts: i32,
    source: String,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoFtpHistoryRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client.database(database.as_ref()).collection("ftp_history"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TrainingLoadError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "effective_from_date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("ftp_history_user_effective_from_date_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "created_at_epoch_seconds": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("ftp_history_user_created_at".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl FtpHistoryRepository for MongoFtpHistoryRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            collection
                .find(doc! { "user_id": &user_id })
                .sort(doc! { "effective_from_date": 1 })
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .into_iter()
                .map(map_document_to_domain)
                .collect()
        })
    }

    fn find_effective_for_date(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "effective_from_date": { "$lte": &date },
                })
                .sort(doc! { "effective_from_date": -1 })
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&entry);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "effective_from_date": &document.effective_from_date,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;
            Ok(entry)
        })
    }
}

fn map_domain_to_document(entry: &FtpHistoryEntry) -> FtpHistoryDocument {
    FtpHistoryDocument {
        user_id: entry.user_id.clone(),
        effective_from_date: entry.effective_from_date.clone(),
        ftp_watts: entry.ftp_watts,
        source: match entry.source {
            FtpSource::Settings => "settings".to_string(),
            FtpSource::Provider => "provider".to_string(),
        },
        created_at_epoch_seconds: entry.created_at_epoch_seconds,
        updated_at_epoch_seconds: entry.updated_at_epoch_seconds,
    }
}

fn map_document_to_domain(
    document: FtpHistoryDocument,
) -> Result<FtpHistoryEntry, TrainingLoadError> {
    Ok(FtpHistoryEntry {
        user_id: document.user_id,
        effective_from_date: document.effective_from_date,
        ftp_watts: document.ftp_watts,
        source: match document.source.as_str() {
            "settings" => FtpSource::Settings,
            "provider" => FtpSource::Provider,
            value => {
                return Err(TrainingLoadError::Repository(format!(
                    "unsupported ftp history source '{value}'"
                )))
            }
        },
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
