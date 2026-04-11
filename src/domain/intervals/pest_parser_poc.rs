use serde::{Deserialize, Serialize};

use super::{BoxFuture, IntervalsError, ParsedWorkoutDoc};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PestParserPocDirection {
    Inbound,
    Outbound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PestParserPocOperation {
    ListEvents,
    GetEvent,
    CreateEvent,
    UpdateEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PestParserPocStatus {
    Parsed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PestParserPocSource {
    pub direction: PestParserPocDirection,
    pub operation: PestParserPocOperation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PestParserPocParsedPayload {
    pub normalized_workout: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PestParserPocWorkoutRecord {
    pub user_id: String,
    pub direction: PestParserPocDirection,
    pub operation: PestParserPocOperation,
    pub source_ref: Option<String>,
    pub source_text: String,
    pub parser_version: String,
    pub parsed_at_epoch_seconds: i64,
    pub status: PestParserPocStatus,
    pub normalized_workout: Option<String>,
    pub parsed_payload: Option<PestParserPocParsedPayload>,
    pub legacy_projection: Option<ParsedWorkoutDoc>,
    pub error_message: Option<String>,
    pub error_kind: Option<String>,
    pub intervals_event_id: Option<i64>,
    pub http_sync_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PestParserPocRecordContext {
    pub user_id: String,
    pub source: PestParserPocSource,
    pub source_ref: Option<String>,
    pub source_text: String,
    pub parser_version: String,
    pub parsed_at_epoch_seconds: i64,
}

impl PestParserPocWorkoutRecord {
    pub fn parsed(
        context: PestParserPocRecordContext,
        normalized_workout: String,
        legacy_projection: ParsedWorkoutDoc,
    ) -> Self {
        Self {
            user_id: context.user_id,
            direction: context.source.direction,
            operation: context.source.operation,
            source_ref: context.source_ref,
            source_text: context.source_text,
            parser_version: context.parser_version,
            parsed_at_epoch_seconds: context.parsed_at_epoch_seconds,
            status: PestParserPocStatus::Parsed,
            normalized_workout: Some(normalized_workout.clone()),
            parsed_payload: Some(PestParserPocParsedPayload { normalized_workout }),
            legacy_projection: Some(legacy_projection),
            error_message: None,
            error_kind: None,
            intervals_event_id: None,
            http_sync_status: None,
        }
    }

    pub fn failed(
        context: PestParserPocRecordContext,
        error_message: String,
        error_kind: String,
    ) -> Self {
        Self {
            user_id: context.user_id,
            direction: context.source.direction,
            operation: context.source.operation,
            source_ref: context.source_ref,
            source_text: context.source_text,
            parser_version: context.parser_version,
            parsed_at_epoch_seconds: context.parsed_at_epoch_seconds,
            status: PestParserPocStatus::Failed,
            normalized_workout: None,
            parsed_payload: None,
            legacy_projection: None,
            error_message: Some(error_message),
            error_kind: Some(error_kind),
            intervals_event_id: None,
            http_sync_status: None,
        }
    }
}

pub trait PestParserPocRepositoryPort: Clone + Send + Sync + 'static {
    fn insert(&self, record: PestParserPocWorkoutRecord) -> BoxFuture<Result<(), IntervalsError>>;
}

pub trait PestParserPocWriter: Send + Sync + 'static {
    fn insert_record(
        &self,
        record: PestParserPocWorkoutRecord,
    ) -> BoxFuture<Result<(), IntervalsError>>;
}

impl<T> PestParserPocWriter for T
where
    T: PestParserPocRepositoryPort,
{
    fn insert_record(
        &self,
        record: PestParserPocWorkoutRecord,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        self.insert(record)
    }
}

#[derive(Clone, Debug, Default)]
pub struct NoopPestParserPocRepository;

impl PestParserPocRepositoryPort for NoopPestParserPocRepository {
    fn insert(&self, _record: PestParserPocWorkoutRecord) -> BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { Ok(()) })
    }
}
