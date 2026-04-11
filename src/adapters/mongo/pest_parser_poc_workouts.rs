use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    BoxFuture, IntervalsError, PestParserPocDirection, PestParserPocOperation,
    PestParserPocParsedPayload, PestParserPocRepositoryPort, PestParserPocStatus,
    PestParserPocWorkoutRecord,
};

#[derive(Clone)]
pub struct MongoPestParserPocWorkoutRepository {
    collection: Collection<PestParserPocWorkoutDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PestParserPocWorkoutDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    user_id: String,
    direction: String,
    operation: String,
    source_ref: Option<String>,
    source_text: String,
    parser_version: String,
    parsed_at_epoch_seconds: i64,
    status: String,
    normalized_workout: Option<String>,
    parsed_payload: Option<PestParserPocParsedPayload>,
    legacy_projection: Option<crate::domain::intervals::ParsedWorkoutDoc>,
    error_message: Option<String>,
    error_kind: Option<String>,
    intervals_event_id: Option<i64>,
    http_sync_status: Option<String>,
}

impl MongoPestParserPocWorkoutRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("pest_parser_poc_workout"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IntervalsError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "parsed_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("pest_parser_poc_workout_user_time".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "status": 1, "parsed_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("pest_parser_poc_workout_status_time".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "operation": 1, "source_ref": 1, "parsed_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("pest_parser_poc_workout_operation_source_time".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| IntervalsError::Internal(error.to_string()))?;
        Ok(())
    }
}

impl PestParserPocRepositoryPort for MongoPestParserPocWorkoutRepository {
    fn insert(&self, record: PestParserPocWorkoutRecord) -> BoxFuture<Result<(), IntervalsError>> {
        let collection = self.collection.clone();
        let document = map_record_to_document(record);
        Box::pin(async move {
            collection
                .insert_one(document)
                .await
                .map_err(|error| IntervalsError::Internal(error.to_string()))?;
            Ok(())
        })
    }
}

fn map_record_to_document(record: PestParserPocWorkoutRecord) -> PestParserPocWorkoutDocument {
    PestParserPocWorkoutDocument {
        id: None,
        user_id: record.user_id,
        direction: match record.direction {
            PestParserPocDirection::Inbound => "inbound",
            PestParserPocDirection::Outbound => "outbound",
        }
        .to_string(),
        operation: match record.operation {
            PestParserPocOperation::ListEvents => "list_events",
            PestParserPocOperation::GetEvent => "get_event",
            PestParserPocOperation::CreateEvent => "create_event",
            PestParserPocOperation::UpdateEvent => "update_event",
        }
        .to_string(),
        source_ref: record.source_ref,
        source_text: record.source_text,
        parser_version: record.parser_version,
        parsed_at_epoch_seconds: record.parsed_at_epoch_seconds,
        status: match record.status {
            PestParserPocStatus::Parsed => "parsed",
            PestParserPocStatus::Failed => "failed",
        }
        .to_string(),
        normalized_workout: record.normalized_workout,
        parsed_payload: record.parsed_payload,
        legacy_projection: record.legacy_projection,
        error_message: record.error_message,
        error_kind: record.error_kind,
        intervals_event_id: record.intervals_event_id,
        http_sync_status: record.http_sync_status,
    }
}
