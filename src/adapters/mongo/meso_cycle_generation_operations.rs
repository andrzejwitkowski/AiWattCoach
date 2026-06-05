use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::durable_ops::{mongo_claim_pending, ClaimInput, ClaimOutcome, OpMetadata};
use super::time::{
    optional_epoch_seconds_to_bson_datetime, required_epoch_seconds_to_bson_datetime,
    resolve_required_epoch_seconds,
};
use crate::domain::{
    ai_workflow::{AttemptRecord, WorkflowPhase, WorkflowStatus},
    llm_tools::LlmToolLoopState,
    meso_cycle::{
        BoxFuture, MesoCycleError, MesoCycleFailureState, MesoCycleGenerationClaimResult,
        MesoCycleGenerationOperation, MesoCycleGenerationOperationRepository,
    },
};

#[derive(Clone)]
pub struct MongoMesoCycleGenerationOperationRepository {
    collection: Collection<MesoCycleGenerationOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MesoCycleGenerationOperationDocument {
    operation_key: String,
    user_id: String,
    requested_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    requested_at: Option<DateTime>,
    meso_start: Option<String>,
    meso_end: Option<String>,
    status: String,
    raw_plan_response: Option<String>,
    raw_plan_description: Option<String>,
    #[serde(default)]
    tool_loop_state: Option<LlmToolLoopState>,
    projection_persisted_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    projection_persisted_at: Option<DateTime>,
    #[serde(default)]
    attempts: Vec<AttemptRecordDocument>,
    failure: Option<MesoCycleFailureStateDocument>,
    started_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    started_at: Option<DateTime>,
    last_attempt_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_attempt_at: Option<DateTime>,
    attempt_count: i64,
    created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    updated_at: Option<DateTime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AttemptRecordDocument {
    phase: String,
    attempt_number: i64,
    recorded_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MesoCycleFailureStateDocument {
    message: String,
}

impl MongoMesoCycleGenerationOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("meso_cycle_generation_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), MesoCycleError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "operation_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "meso_cycle_generation_operations_operation_key_unique".to_string(),
                            )
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("meso_cycle_generation_operations_user_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "status": 1, "updated_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "meso_cycle_generation_operations_user_status_updated".to_string(),
                            )
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl MesoCycleGenerationOperationRepository for MongoMesoCycleGenerationOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "operation_key": &operation_key })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn find_by_operation_key_for_user(
        &self,
        operation_key: &str,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "operation_key": &operation_key,
                    "user_id": &user_id,
                })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id })
                .sort(doc! { "updated_at_epoch_seconds": -1 })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn find_pending_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id, "status": "pending" })
                .sort(doc! { "updated_at_epoch_seconds": -1 })
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: MesoCycleGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<MesoCycleGenerationClaimResult, MesoCycleError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation)?;
            let user_id = document.user_id.clone();

            mongo_claim_pending(
                ClaimInput {
                    collection,
                    document,
                    operation,
                    stale_before_epoch_seconds,
                },
                || doc! { "user_id": &user_id },
                |doc| map_document_to_operation(doc).map_err(|e| e.to_string()),
                |op, s| {
                    matches!(op.status, WorkflowStatus::Completed)
                        || matches!(op.status, WorkflowStatus::Failed)
                        || (matches!(op.status, WorkflowStatus::Pending)
                            && op.last_attempt_at_epoch_seconds <= s)
                },
                |op| OpMetadata {
                    attempt_count: i64::from(op.attempt_count),
                    updated_at_epoch_seconds: op.updated_at_epoch_seconds,
                    last_attempt_at_epoch_seconds: op.last_attempt_at_epoch_seconds,
                },
                |existing, pending, now| {
                    let reclaimed = if matches!(existing.status, WorkflowStatus::Completed) {
                        existing.reclaim_for_new_generation(now)
                    } else {
                        let mut reclaimed = existing.reclaim(now);
                        reclaimed.requested_at_epoch_seconds = pending.requested_at_epoch_seconds;
                        reclaimed
                    };
                    let doc = map_operation_to_document(&reclaimed).map_err(|e| e.to_string())?;
                    Ok((reclaimed, doc))
                },
            )
            .await
            .map_err(MesoCycleError::Repository)
            .map(|outcome| match outcome {
                ClaimOutcome::Claimed(op) => MesoCycleGenerationClaimResult::Claimed(op),
                ClaimOutcome::Existing(op) => match op.status {
                    WorkflowStatus::Pending => MesoCycleGenerationClaimResult::AlreadyPending,
                    WorkflowStatus::Completed => {
                        MesoCycleGenerationClaimResult::AlreadyCompleted(op)
                    }
                    WorkflowStatus::Failed => MesoCycleGenerationClaimResult::AlreadyPending,
                },
            })
        })
    }

    fn upsert(
        &self,
        operation: MesoCycleGenerationOperation,
    ) -> BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation)?;
            collection
                .replace_one(doc! { "user_id": &document.user_id }, &document)
                .upsert(true)
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;
            Ok(operation)
        })
    }
}

fn map_operation_to_document(
    operation: &MesoCycleGenerationOperation,
) -> Result<MesoCycleGenerationOperationDocument, MesoCycleError> {
    Ok(MesoCycleGenerationOperationDocument {
        operation_key: operation.operation_key.clone(),
        user_id: operation.user_id.clone(),
        requested_at_epoch_seconds: Some(operation.requested_at_epoch_seconds),
        requested_at: required_epoch_seconds_to_bson_datetime(
            operation.requested_at_epoch_seconds,
            "requested_at",
        )
        .map_err(MesoCycleError::Repository)?,
        meso_start: operation.meso_start.clone(),
        meso_end: operation.meso_end.clone(),
        status: map_status_to_document(&operation.status).to_string(),
        raw_plan_response: operation.raw_plan_response.clone(),
        raw_plan_description: operation.raw_plan_description.clone(),
        tool_loop_state: operation.tool_loop_state.clone(),
        projection_persisted_at_epoch_seconds: operation.projection_persisted_at_epoch_seconds,
        projection_persisted_at: optional_epoch_seconds_to_bson_datetime(
            operation.projection_persisted_at_epoch_seconds,
            "projection_persisted_at",
        )
        .map_err(MesoCycleError::Repository)?,
        attempts: operation
            .attempts
            .iter()
            .map(map_attempt_to_document)
            .collect::<Result<Vec<_>, _>>()?,
        failure: operation
            .failure
            .as_ref()
            .map(|failure| MesoCycleFailureStateDocument {
                message: failure.message.clone(),
            }),
        started_at_epoch_seconds: Some(operation.started_at_epoch_seconds),
        started_at: required_epoch_seconds_to_bson_datetime(
            operation.started_at_epoch_seconds,
            "started_at",
        )
        .map_err(MesoCycleError::Repository)?,
        last_attempt_at_epoch_seconds: Some(operation.last_attempt_at_epoch_seconds),
        last_attempt_at: required_epoch_seconds_to_bson_datetime(
            operation.last_attempt_at_epoch_seconds,
            "last_attempt_at",
        )
        .map_err(MesoCycleError::Repository)?,
        attempt_count: i64::from(operation.attempt_count),
        created_at_epoch_seconds: Some(operation.created_at_epoch_seconds),
        created_at: required_epoch_seconds_to_bson_datetime(
            operation.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(MesoCycleError::Repository)?,
        updated_at_epoch_seconds: Some(operation.updated_at_epoch_seconds),
        updated_at: required_epoch_seconds_to_bson_datetime(
            operation.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(MesoCycleError::Repository)?,
    })
}

fn map_attempt_to_document(
    attempt: &AttemptRecord,
) -> Result<AttemptRecordDocument, MesoCycleError> {
    Ok(AttemptRecordDocument {
        phase: map_phase_to_document(&attempt.phase).to_string(),
        attempt_number: i64::from(attempt.attempt_number),
        recorded_at_epoch_seconds: Some(attempt.recorded_at_epoch_seconds),
    })
}

fn map_document_to_attempt(
    document: AttemptRecordDocument,
) -> Result<AttemptRecord, MesoCycleError> {
    Ok(AttemptRecord {
        phase: map_document_to_phase(&document.phase)?,
        attempt_number: u32::try_from(document.attempt_number).map_err(|_| {
            MesoCycleError::Repository("invalid meso cycle attempt number".to_string())
        })?,
        recorded_at_epoch_seconds: resolve_required_epoch_seconds(
            None,
            document.recorded_at_epoch_seconds,
            "recorded_at",
        )
        .map_err(MesoCycleError::Repository)?,
    })
}

fn map_phase_to_document(phase: &WorkflowPhase) -> &'static str {
    match phase {
        WorkflowPhase::WorkoutRecap => "workout_recap",
        WorkflowPhase::InitialGeneration => "initial_generation",
        WorkflowPhase::Correction => "correction",
        WorkflowPhase::ProjectionUpdate => "projection_update",
    }
}

fn map_document_to_phase(phase: &str) -> Result<WorkflowPhase, MesoCycleError> {
    match phase {
        "workout_recap" => Ok(WorkflowPhase::WorkoutRecap),
        "initial_generation" => Ok(WorkflowPhase::InitialGeneration),
        "correction" => Ok(WorkflowPhase::Correction),
        "projection_update" => Ok(WorkflowPhase::ProjectionUpdate),
        _ => Err(MesoCycleError::Repository(format!(
            "invalid meso cycle attempt phase: {phase}"
        ))),
    }
}

fn map_document_to_operation(
    document: MesoCycleGenerationOperationDocument,
) -> Result<MesoCycleGenerationOperation, MesoCycleError> {
    Ok(MesoCycleGenerationOperation {
        operation_key: document.operation_key,
        user_id: document.user_id,
        requested_at_epoch_seconds: resolve_required_epoch_seconds(
            document.requested_at,
            document.requested_at_epoch_seconds,
            "requested_at",
        )
        .map_err(MesoCycleError::Repository)?,
        meso_start: document.meso_start,
        meso_end: document.meso_end,
        status: map_document_to_status(&document.status)?,
        raw_plan_response: document.raw_plan_response,
        raw_plan_description: document.raw_plan_description,
        tool_loop_state: document.tool_loop_state,
        projection_persisted_at_epoch_seconds: document.projection_persisted_at_epoch_seconds,
        attempts: document
            .attempts
            .into_iter()
            .map(map_document_to_attempt)
            .collect::<Result<Vec<_>, _>>()?,
        failure: document.failure.map(|failure| MesoCycleFailureState {
            message: failure.message,
        }),
        started_at_epoch_seconds: resolve_required_epoch_seconds(
            document.started_at,
            document.started_at_epoch_seconds,
            "started_at",
        )
        .map_err(MesoCycleError::Repository)?,
        last_attempt_at_epoch_seconds: resolve_required_epoch_seconds(
            document.last_attempt_at,
            document.last_attempt_at_epoch_seconds,
            "last_attempt_at",
        )
        .map_err(MesoCycleError::Repository)?,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            MesoCycleError::Repository(
                "invalid meso cycle generation operation attempt count".to_string(),
            )
        })?,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(MesoCycleError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(MesoCycleError::Repository)?,
    })
}

fn map_status_to_document(status: &WorkflowStatus) -> &'static str {
    match status {
        WorkflowStatus::Pending => "pending",
        WorkflowStatus::Completed => "completed",
        WorkflowStatus::Failed => "failed",
    }
}

fn map_document_to_status(value: &str) -> Result<WorkflowStatus, MesoCycleError> {
    match value {
        "pending" => Ok(WorkflowStatus::Pending),
        "completed" => Ok(WorkflowStatus::Completed),
        "failed" => Ok(WorkflowStatus::Failed),
        other => Err(MesoCycleError::Repository(format!(
            "unknown meso cycle operation status: {other}"
        ))),
    }
}
