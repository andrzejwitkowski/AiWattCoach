use mongodb::{
    bson::{doc, spec::BinarySubtype, Binary},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::wahoo_fit_files::{
    BoxFuture, WahooFitFile, WahooFitFileError, WahooFitFileRepository, WahooFitFileStage,
};

#[derive(Clone)]
pub struct MongoWahooFitFileRepository {
    collection: Collection<WahooFitFileDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WahooFitFileDocument {
    user_id: String,
    completed_workout_id: String,
    wahoo_workout_id: i64,
    stage: String,
    file_url: Option<String>,
    file_hash_sha256: Option<String>,
    raw_fit_bytes: Option<Binary>,
    downloaded_at_epoch_seconds: Option<i64>,
    stored_at_epoch_seconds: Option<i64>,
    parsed_at_epoch_seconds: Option<i64>,
    enriched_at_epoch_seconds: Option<i64>,
    updated_at_epoch_seconds: i64,
}

impl MongoWahooFitFileRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("wahoo_fit_files"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WahooFitFileError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "completed_workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("wahoo_fit_files_user_completed_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "stage": 1, "updated_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("wahoo_fit_files_stage_lookup".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| WahooFitFileError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl WahooFitFileRepository for MongoWahooFitFileRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<WahooFitFile>, WahooFitFileError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "completed_workout_id": &completed_workout_id,
                })
                .await
                .map_err(|error| WahooFitFileError::Repository(error.to_string()))?
                .map(map_document_to_domain)
                .transpose()
        })
    }

    fn upsert(&self, fit_file: WahooFitFile) -> BoxFuture<Result<WahooFitFile, WahooFitFileError>> {
        let collection = self.collection.clone();
        let document = map_fit_file_to_document(&fit_file);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "completed_workout_id": &document.completed_workout_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| WahooFitFileError::Repository(error.to_string()))?;
            Ok(fit_file)
        })
    }
}

fn map_fit_file_to_document(fit_file: &WahooFitFile) -> WahooFitFileDocument {
    WahooFitFileDocument {
        user_id: fit_file.user_id.clone(),
        completed_workout_id: fit_file.completed_workout_id.clone(),
        wahoo_workout_id: fit_file.wahoo_workout_id,
        stage: stage_as_str(&fit_file.stage).to_string(),
        file_url: fit_file.file_url.clone(),
        file_hash_sha256: fit_file.file_hash_sha256.clone(),
        raw_fit_bytes: fit_file.raw_fit_bytes.clone().map(|bytes| Binary {
            subtype: BinarySubtype::Generic,
            bytes,
        }),
        downloaded_at_epoch_seconds: fit_file.downloaded_at_epoch_seconds,
        stored_at_epoch_seconds: fit_file.stored_at_epoch_seconds,
        parsed_at_epoch_seconds: fit_file.parsed_at_epoch_seconds,
        enriched_at_epoch_seconds: fit_file.enriched_at_epoch_seconds,
        updated_at_epoch_seconds: fit_file.updated_at_epoch_seconds,
    }
}

fn map_document_to_domain(
    document: WahooFitFileDocument,
) -> Result<WahooFitFile, WahooFitFileError> {
    Ok(WahooFitFile {
        user_id: document.user_id,
        completed_workout_id: document.completed_workout_id,
        wahoo_workout_id: document.wahoo_workout_id,
        stage: stage_from_str(&document.stage)?,
        file_url: document.file_url,
        file_hash_sha256: document.file_hash_sha256,
        raw_fit_bytes: document.raw_fit_bytes.map(|binary| binary.bytes),
        downloaded_at_epoch_seconds: document.downloaded_at_epoch_seconds,
        stored_at_epoch_seconds: document.stored_at_epoch_seconds,
        parsed_at_epoch_seconds: document.parsed_at_epoch_seconds,
        enriched_at_epoch_seconds: document.enriched_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}

fn stage_as_str(stage: &WahooFitFileStage) -> &'static str {
    match stage {
        WahooFitFileStage::Queued => "queued",
        WahooFitFileStage::Downloaded => "downloaded",
        WahooFitFileStage::Stored => "stored",
        WahooFitFileStage::Parsed => "parsed",
        WahooFitFileStage::Enriched => "enriched",
    }
}

fn stage_from_str(value: &str) -> Result<WahooFitFileStage, WahooFitFileError> {
    match value {
        "queued" => Ok(WahooFitFileStage::Queued),
        "downloaded" => Ok(WahooFitFileStage::Downloaded),
        "stored" => Ok(WahooFitFileStage::Stored),
        "parsed" => Ok(WahooFitFileStage::Parsed),
        "enriched" => Ok(WahooFitFileStage::Enriched),
        other => Err(WahooFitFileError::Repository(format!(
            "unknown Wahoo FIT file stage: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_file_document_round_trip_preserves_binary_payload() {
        let fit_file =
            WahooFitFile::new("user-1".to_string(), "wahoo-workout:1".to_string(), 1, 100)
                .mark_stored(
                    "https://example.test/file.fit".to_string(),
                    "hash-1".to_string(),
                    vec![1, 2, 3],
                    120,
                )
                .mark_parsed(130)
                .mark_enriched(140);

        let mapped = map_document_to_domain(map_fit_file_to_document(&fit_file))
            .expect("fit file should round-trip through document mapping");

        assert_eq!(mapped, fit_file);
    }
}
