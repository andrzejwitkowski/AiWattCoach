use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::training_load::{
    BoxFuture, FtpSource, TrainingLoadDailySnapshot, TrainingLoadDailySnapshotRepository,
    TrainingLoadError, TrainingLoadSnapshotRange,
};

#[derive(Clone)]
pub struct MongoTrainingLoadDailySnapshotRepository {
    collection: Collection<TrainingLoadDailySnapshotDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingLoadDailySnapshotDocument {
    user_id: String,
    date: String,
    daily_tss: Option<i32>,
    rolling_tss_7d: Option<f64>,
    rolling_tss_28d: Option<f64>,
    ctl: Option<f64>,
    atl: Option<f64>,
    tsb: Option<f64>,
    average_if_28d: Option<f64>,
    average_ef_28d: Option<f64>,
    ftp_effective_watts: Option<i32>,
    ftp_source: Option<String>,
    recomputed_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

impl MongoTrainingLoadDailySnapshotRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_load_daily_snapshots"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TrainingLoadError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "date": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_load_daily_snapshots_user_date_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "updated_at_epoch_seconds": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("training_load_daily_snapshots_user_updated_at".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl TrainingLoadDailySnapshotRepository for MongoTrainingLoadDailySnapshotRepository {
    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &TrainingLoadSnapshotRange,
    ) -> BoxFuture<Result<Vec<TrainingLoadDailySnapshot>, TrainingLoadError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "date": { "$gte": &oldest, "$lte": &newest },
                })
                .sort(doc! { "date": 1 })
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

    fn upsert(
        &self,
        snapshot: TrainingLoadDailySnapshot,
    ) -> BoxFuture<Result<TrainingLoadDailySnapshot, TrainingLoadError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&snapshot);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "date": &document.date,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;
            Ok(snapshot)
        })
    }

    fn find_oldest_date_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<String>, TrainingLoadError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let oldest = collection
                .find(doc! {
                    "user_id": &user_id,
                })
                .sort(doc! { "date": 1 })
                .limit(1)
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .try_next()
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?
                .map(|document| document.date);
            Ok(oldest)
        })
    }

    fn delete_by_user_id_from_date(
        &self,
        user_id: &str,
        from_date: &str,
    ) -> BoxFuture<Result<(), TrainingLoadError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let from_date = from_date.to_string();
        Box::pin(async move {
            collection
                .delete_many(doc! {
                    "user_id": &user_id,
                    "date": { "$gte": &from_date },
                })
                .await
                .map_err(|error| TrainingLoadError::Repository(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_domain_to_document(
    snapshot: &TrainingLoadDailySnapshot,
) -> TrainingLoadDailySnapshotDocument {
    TrainingLoadDailySnapshotDocument {
        user_id: snapshot.user_id.clone(),
        date: snapshot.date.clone(),
        daily_tss: snapshot.daily_tss,
        rolling_tss_7d: snapshot.rolling_tss_7d,
        rolling_tss_28d: snapshot.rolling_tss_28d,
        ctl: snapshot.ctl,
        atl: snapshot.atl,
        tsb: snapshot.tsb,
        average_if_28d: snapshot.average_if_28d,
        average_ef_28d: snapshot.average_ef_28d,
        ftp_effective_watts: snapshot.ftp_effective_watts,
        ftp_source: snapshot.ftp_source.as_ref().map(|source| match source {
            FtpSource::Settings => "settings".to_string(),
            FtpSource::Provider => "provider".to_string(),
        }),
        recomputed_at_epoch_seconds: snapshot.recomputed_at_epoch_seconds,
        created_at_epoch_seconds: snapshot.created_at_epoch_seconds,
        updated_at_epoch_seconds: snapshot.updated_at_epoch_seconds,
    }
}

fn map_document_to_domain(
    document: TrainingLoadDailySnapshotDocument,
) -> Result<TrainingLoadDailySnapshot, TrainingLoadError> {
    Ok(TrainingLoadDailySnapshot {
        user_id: document.user_id,
        date: document.date,
        daily_tss: document.daily_tss,
        rolling_tss_7d: document.rolling_tss_7d,
        rolling_tss_28d: document.rolling_tss_28d,
        ctl: document.ctl,
        atl: document.atl,
        tsb: document.tsb,
        average_if_28d: document.average_if_28d,
        average_ef_28d: document.average_ef_28d,
        ftp_effective_watts: document.ftp_effective_watts,
        ftp_source: match document.ftp_source.as_deref() {
            Some("settings") => Some(FtpSource::Settings),
            Some("provider") => Some(FtpSource::Provider),
            Some(value) => {
                return Err(TrainingLoadError::Repository(format!(
                    "unsupported training load ftp source '{value}'"
                )))
            }
            None => None,
        },
        recomputed_at_epoch_seconds: document.recomputed_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}
