use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
    llm_tools::LlmToolLoopState,
    training_plan::{
        BoxFuture, TrainingPlanError, TrainingPlanFailureState, TrainingPlanGenerationClaimResult,
        TrainingPlanGenerationOperation, TrainingPlanGenerationOperationRepository,
    },
};

use super::durable_ops::{mongo_claim_pending, ClaimInput, ClaimOutcome, OpMetadata};
use super::time::{
    optional_epoch_seconds_to_bson_datetime, required_epoch_seconds_to_bson_datetime,
    resolve_optional_epoch_seconds, resolve_required_epoch_seconds,
};

#[derive(Clone)]
pub struct MongoTrainingPlanGenerationOperationRepository {
    collection: Collection<TrainingPlanGenerationOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanGenerationOperationDocument {
    operation_key: String,
    user_id: String,
    workout_id: String,
    saved_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    saved_at: Option<DateTime>,
    status: String,
    workout_recap_text: Option<String>,
    workout_recap_provider: Option<String>,
    workout_recap_model: Option<String>,
    workout_recap_generated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    workout_recap_generated_at: Option<DateTime>,
    projection_persisted_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    projection_persisted_at: Option<DateTime>,
    raw_plan_response: Option<String>,
    raw_plan_description: Option<String>,
    #[serde(default)]
    initial_plan_tool_loop_state: Option<LlmToolLoopState>,
    raw_correction_response: Option<String>,
    raw_correction_description: Option<String>,
    #[serde(default)]
    correction_tool_loop_state: Option<LlmToolLoopState>,
    #[serde(default)]
    validation_issues: Vec<ValidationIssueDocument>,
    #[serde(default)]
    attempts: Vec<AttemptRecordDocument>,
    failure: Option<TrainingPlanFailureStateDocument>,
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
    #[serde(default)]
    recorded_at: Option<DateTime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ValidationIssueDocument {
    scope: String,
    message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanFailureStateDocument {
    phase: String,
    message: String,
}

impl MongoTrainingPlanGenerationOperationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("training_plan_generation_operations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), TrainingPlanError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "operation_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "training_plan_generation_operations_operation_key_unique"
                                    .to_string(),
                            )
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "status": 1, "updated_at_epoch_seconds": -1 })
                    .options(
                        IndexOptions::builder()
                            .name(
                                "training_plan_generation_operations_user_status_updated"
                                    .to_string(),
                            )
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl TrainingPlanGenerationOperationRepository for MongoTrainingPlanGenerationOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "operation_key": &operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<TrainingPlanGenerationClaimResult, TrainingPlanError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation)?;
            let operation_key = document.operation_key.clone();

            mongo_claim_pending(
                ClaimInput {
                    collection,
                    document,
                    operation,
                    stale_before_epoch_seconds,
                },
                || doc! { "operation_key": &operation_key },
                |doc| map_document_to_operation(doc).map_err(|e| e.to_string()),
                |op, s| {
                    matches!(op.status, WorkflowStatus::Pending)
                        && op.last_attempt_at_epoch_seconds <= s
                        || matches!(op.status, WorkflowStatus::Failed)
                },
                |op| OpMetadata {
                    attempt_count: i64::from(op.attempt_count),
                    updated_at_epoch_seconds: op.updated_at_epoch_seconds,
                    last_attempt_at_epoch_seconds: op.last_attempt_at_epoch_seconds,
                },
                |existing, _pending, now| {
                    let reclaimed = existing.reclaim(now);
                    let doc = map_operation_to_document(&reclaimed).map_err(|e| e.to_string())?;
                    Ok((reclaimed, doc))
                },
            )
            .await
            .map_err(TrainingPlanError::Repository)
            .map(|outcome| match outcome {
                ClaimOutcome::Claimed(op) => TrainingPlanGenerationClaimResult::Claimed(op),
                ClaimOutcome::Existing(op) => TrainingPlanGenerationClaimResult::Existing(op),
            })
        })
    }

    fn upsert(
        &self,
        operation: TrainingPlanGenerationOperation,
    ) -> BoxFuture<Result<TrainingPlanGenerationOperation, TrainingPlanError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation)?;
            collection
                .replace_one(doc! { "operation_key": &document.operation_key }, &document)
                .upsert(true)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            Ok(operation)
        })
    }

    fn find_latest_completed_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "status": "completed",
                })
                .sort(doc! { "updated_at_epoch_seconds": -1 })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }
}

fn map_operation_to_document(
    operation: &TrainingPlanGenerationOperation,
) -> Result<TrainingPlanGenerationOperationDocument, TrainingPlanError> {
    Ok(TrainingPlanGenerationOperationDocument {
        operation_key: operation.operation_key.clone(),
        user_id: operation.user_id.clone(),
        workout_id: operation.workout_id.clone(),
        saved_at_epoch_seconds: Some(operation.saved_at_epoch_seconds),
        saved_at: required_epoch_seconds_to_bson_datetime(
            operation.saved_at_epoch_seconds,
            "saved_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        status: map_status_to_document(&operation.status).to_string(),
        workout_recap_text: operation.workout_recap_text.clone(),
        workout_recap_provider: operation.workout_recap_provider.clone(),
        workout_recap_model: operation.workout_recap_model.clone(),
        workout_recap_generated_at_epoch_seconds: operation
            .workout_recap_generated_at_epoch_seconds,
        workout_recap_generated_at: optional_epoch_seconds_to_bson_datetime(
            operation.workout_recap_generated_at_epoch_seconds,
            "workout_recap_generated_at",
        )
        .expect("workout_recap_generated_at should fit BSON DateTime"),
        projection_persisted_at_epoch_seconds: operation.projection_persisted_at_epoch_seconds,
        projection_persisted_at: optional_epoch_seconds_to_bson_datetime(
            operation.projection_persisted_at_epoch_seconds,
            "projection_persisted_at",
        )
        .expect("projection_persisted_at should fit BSON DateTime"),
        raw_plan_response: operation.raw_plan_response.clone(),
        raw_plan_description: operation.raw_plan_description.clone(),
        initial_plan_tool_loop_state: operation.initial_plan_tool_loop_state.clone(),
        raw_correction_response: operation.raw_correction_response.clone(),
        raw_correction_description: operation.raw_correction_description.clone(),
        correction_tool_loop_state: operation.correction_tool_loop_state.clone(),
        validation_issues: operation
            .validation_issues
            .iter()
            .map(map_issue_to_document)
            .collect(),
        attempts: operation
            .attempts
            .iter()
            .map(map_attempt_to_document)
            .collect::<Result<Vec<_>, _>>()?,
        failure: operation.failure.as_ref().map(map_failure_to_document),
        started_at_epoch_seconds: Some(operation.started_at_epoch_seconds),
        started_at: required_epoch_seconds_to_bson_datetime(
            operation.started_at_epoch_seconds,
            "started_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        last_attempt_at_epoch_seconds: Some(operation.last_attempt_at_epoch_seconds),
        last_attempt_at: required_epoch_seconds_to_bson_datetime(
            operation.last_attempt_at_epoch_seconds,
            "last_attempt_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        attempt_count: i64::from(operation.attempt_count),
        created_at_epoch_seconds: Some(operation.created_at_epoch_seconds),
        created_at: required_epoch_seconds_to_bson_datetime(
            operation.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        updated_at_epoch_seconds: Some(operation.updated_at_epoch_seconds),
        updated_at: required_epoch_seconds_to_bson_datetime(
            operation.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(TrainingPlanError::Repository)?,
    })
}

fn map_document_to_operation(
    document: TrainingPlanGenerationOperationDocument,
) -> Result<TrainingPlanGenerationOperation, TrainingPlanError> {
    Ok(TrainingPlanGenerationOperation {
        operation_key: document.operation_key,
        user_id: document.user_id,
        workout_id: document.workout_id,
        saved_at_epoch_seconds: resolve_required_epoch_seconds(
            document.saved_at,
            document.saved_at_epoch_seconds,
            "saved_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        status: map_document_to_status(&document.status)?,
        workout_recap_text: document.workout_recap_text,
        workout_recap_provider: document.workout_recap_provider,
        workout_recap_model: document.workout_recap_model,
        workout_recap_generated_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.workout_recap_generated_at,
            document.workout_recap_generated_at_epoch_seconds,
        ),
        projection_persisted_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.projection_persisted_at,
            document.projection_persisted_at_epoch_seconds,
        ),
        raw_plan_response: document.raw_plan_response,
        raw_plan_description: document.raw_plan_description,
        initial_plan_tool_loop_state: document.initial_plan_tool_loop_state,
        raw_correction_response: document.raw_correction_response,
        raw_correction_description: document.raw_correction_description,
        correction_tool_loop_state: document.correction_tool_loop_state,
        validation_issues: document
            .validation_issues
            .into_iter()
            .map(map_document_to_issue)
            .collect(),
        attempts: document
            .attempts
            .into_iter()
            .map(map_document_to_attempt)
            .collect::<Result<Vec<_>, _>>()?,
        failure: document.failure.map(map_document_to_failure).transpose()?,
        started_at_epoch_seconds: resolve_required_epoch_seconds(
            document.started_at,
            document.started_at_epoch_seconds,
            "started_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        last_attempt_at_epoch_seconds: resolve_required_epoch_seconds(
            document.last_attempt_at,
            document.last_attempt_at_epoch_seconds,
            "last_attempt_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            TrainingPlanError::Repository("invalid training plan attempt count".to_string())
        })?,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(TrainingPlanError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(TrainingPlanError::Repository)?,
    })
}

fn map_issue_to_document(issue: &ValidationIssue) -> ValidationIssueDocument {
    ValidationIssueDocument {
        scope: issue.scope.clone(),
        message: issue.message.clone(),
    }
}

fn map_document_to_issue(document: ValidationIssueDocument) -> ValidationIssue {
    ValidationIssue {
        scope: document.scope,
        message: document.message,
    }
}

fn map_attempt_to_document(
    attempt: &AttemptRecord,
) -> Result<AttemptRecordDocument, TrainingPlanError> {
    Ok(AttemptRecordDocument {
        phase: map_phase_to_document(&attempt.phase).to_string(),
        attempt_number: i64::from(attempt.attempt_number),
        recorded_at_epoch_seconds: Some(attempt.recorded_at_epoch_seconds),
        recorded_at: required_epoch_seconds_to_bson_datetime(
            attempt.recorded_at_epoch_seconds,
            "recorded_at",
        )
        .map_err(TrainingPlanError::Repository)?,
    })
}

fn map_document_to_attempt(
    document: AttemptRecordDocument,
) -> Result<AttemptRecord, TrainingPlanError> {
    Ok(AttemptRecord {
        phase: map_document_to_phase(&document.phase)?,
        attempt_number: u32::try_from(document.attempt_number)
            .map_err(|_| TrainingPlanError::Repository("invalid attempt number".to_string()))?,
        recorded_at_epoch_seconds: resolve_required_epoch_seconds(
            document.recorded_at,
            document.recorded_at_epoch_seconds,
            "recorded_at",
        )
        .map_err(TrainingPlanError::Repository)?,
    })
}

fn map_failure_to_document(failure: &TrainingPlanFailureState) -> TrainingPlanFailureStateDocument {
    TrainingPlanFailureStateDocument {
        phase: map_phase_to_document(&failure.phase).to_string(),
        message: failure.message.clone(),
    }
}

fn map_document_to_failure(
    document: TrainingPlanFailureStateDocument,
) -> Result<TrainingPlanFailureState, TrainingPlanError> {
    Ok(TrainingPlanFailureState {
        phase: map_document_to_phase(&document.phase)?,
        message: document.message,
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

fn map_document_to_phase(value: &str) -> Result<WorkflowPhase, TrainingPlanError> {
    match value {
        "workout_recap" => Ok(WorkflowPhase::WorkoutRecap),
        "initial_generation" => Ok(WorkflowPhase::InitialGeneration),
        "correction" => Ok(WorkflowPhase::Correction),
        "projection_update" => Ok(WorkflowPhase::ProjectionUpdate),
        other => Err(TrainingPlanError::Repository(format!(
            "unknown training plan workflow phase: {other}"
        ))),
    }
}

fn map_status_to_document(status: &WorkflowStatus) -> &'static str {
    match status {
        WorkflowStatus::Pending => "pending",
        WorkflowStatus::Completed => "completed",
        WorkflowStatus::Failed => "failed",
    }
}

fn map_document_to_status(value: &str) -> Result<WorkflowStatus, TrainingPlanError> {
    match value {
        "pending" => Ok(WorkflowStatus::Pending),
        "completed" => Ok(WorkflowStatus::Completed),
        "failed" => Ok(WorkflowStatus::Failed),
        other => Err(TrainingPlanError::Repository(format!(
            "unknown training plan operation status: {other}"
        ))),
    }
}
