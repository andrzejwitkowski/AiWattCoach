use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::intervals::{
    BoxFuture, IntervalsError, ParsedWorkoutDoc, PestParserPocDirection, PestParserPocOperation,
    PestParserPocParsedPayload, PestParserPocRepositoryPort, PestParserPocStatus,
    PestParserPocWorkoutRecord, WorkoutIntervalDefinition, WorkoutSegment, WorkoutSummary,
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
    parsed_payload: Option<PestParserPocParsedPayloadDocument>,
    legacy_projection: Option<ParsedWorkoutDocDocument>,
    error_message: Option<String>,
    error_kind: Option<String>,
    intervals_event_id: Option<i64>,
    http_sync_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PestParserPocParsedPayloadDocument {
    normalized_workout: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ParsedWorkoutDocDocument {
    intervals: Vec<WorkoutIntervalDefinitionDocument>,
    segments: Vec<WorkoutSegmentDocument>,
    summary: WorkoutSummaryDocument,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorkoutIntervalDefinitionDocument {
    definition: String,
    repeat_count: usize,
    duration_seconds: Option<i32>,
    target_percent_ftp: Option<f64>,
    min_target_percent_ftp: Option<f64>,
    max_target_percent_ftp: Option<f64>,
    zone_id: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorkoutSegmentDocument {
    order: usize,
    label: String,
    duration_seconds: i32,
    start_offset_seconds: i32,
    end_offset_seconds: i32,
    target_percent_ftp: Option<f64>,
    min_target_percent_ftp: Option<f64>,
    max_target_percent_ftp: Option<f64>,
    zone_id: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorkoutSummaryDocument {
    total_segments: usize,
    total_duration_seconds: i32,
    estimated_normalized_power_watts: Option<i32>,
    estimated_average_power_watts: Option<i32>,
    estimated_intensity_factor: Option<f64>,
    estimated_training_stress_score: Option<f64>,
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
        parsed_payload: record.parsed_payload.map(map_parsed_payload_to_document),
        legacy_projection: record
            .legacy_projection
            .map(map_legacy_projection_to_document),
        error_message: record.error_message,
        error_kind: record.error_kind,
        intervals_event_id: record.intervals_event_id,
        http_sync_status: record.http_sync_status,
    }
}

fn map_parsed_payload_to_document(
    payload: PestParserPocParsedPayload,
) -> PestParserPocParsedPayloadDocument {
    PestParserPocParsedPayloadDocument {
        normalized_workout: payload.normalized_workout,
    }
}

fn map_legacy_projection_to_document(parsed: ParsedWorkoutDoc) -> ParsedWorkoutDocDocument {
    ParsedWorkoutDocDocument {
        intervals: parsed
            .intervals
            .into_iter()
            .map(map_interval_definition_to_document)
            .collect(),
        segments: parsed
            .segments
            .into_iter()
            .map(map_segment_to_document)
            .collect(),
        summary: map_summary_to_document(parsed.summary),
    }
}

fn map_interval_definition_to_document(
    interval: WorkoutIntervalDefinition,
) -> WorkoutIntervalDefinitionDocument {
    WorkoutIntervalDefinitionDocument {
        definition: interval.definition,
        repeat_count: interval.repeat_count,
        duration_seconds: interval.duration_seconds,
        target_percent_ftp: interval.target_percent_ftp,
        min_target_percent_ftp: interval.min_target_percent_ftp,
        max_target_percent_ftp: interval.max_target_percent_ftp,
        zone_id: interval.zone_id,
    }
}

fn map_segment_to_document(segment: WorkoutSegment) -> WorkoutSegmentDocument {
    WorkoutSegmentDocument {
        order: segment.order,
        label: segment.label,
        duration_seconds: segment.duration_seconds,
        start_offset_seconds: segment.start_offset_seconds,
        end_offset_seconds: segment.end_offset_seconds,
        target_percent_ftp: segment.target_percent_ftp,
        min_target_percent_ftp: segment.min_target_percent_ftp,
        max_target_percent_ftp: segment.max_target_percent_ftp,
        zone_id: segment.zone_id,
    }
}

fn map_summary_to_document(summary: WorkoutSummary) -> WorkoutSummaryDocument {
    WorkoutSummaryDocument {
        total_segments: summary.total_segments,
        total_duration_seconds: summary.total_duration_seconds,
        estimated_normalized_power_watts: summary.estimated_normalized_power_watts,
        estimated_average_power_watts: summary.estimated_average_power_watts,
        estimated_intensity_factor: summary.estimated_intensity_factor,
        estimated_training_stress_score: summary.estimated_training_stress_score,
    }
}
