use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
    training_plan::{
        BoxFuture, TrainingPlanError, TrainingPlanFailureState, TrainingPlanGenerationClaimResult,
        TrainingPlanGenerationOperation, TrainingPlanGenerationOperationRepository,
    },
};

use super::error::is_duplicate_key_error;

#[derive(Clone)]
pub struct MongoTrainingPlanGenerationOperationRepository {
    collection: Collection<TrainingPlanGenerationOperationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrainingPlanGenerationOperationDocument {
    operation_key: String,
    user_id: String,
    workout_id: String,
    saved_at_epoch_seconds: i64,
    status: String,
    workout_recap_text: Option<String>,
    workout_recap_provider: Option<String>,
    workout_recap_model: Option<String>,
    workout_recap_generated_at_epoch_seconds: Option<i64>,
    projection_persisted_at_epoch_seconds: Option<i64>,
    raw_plan_response: Option<String>,
    raw_correction_response: Option<String>,
    validation_issues: Vec<ValidationIssueDocument>,
    attempts: Vec<AttemptRecordDocument>,
    failure: Option<TrainingPlanFailureStateDocument>,
    started_at_epoch_seconds: i64,
    last_attempt_at_epoch_seconds: i64,
    attempt_count: i64,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AttemptRecordDocument {
    phase: String,
    attempt_number: i64,
    recorded_at_epoch_seconds: i64,
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
            .create_indexes([IndexModel::builder()
                .keys(doc! { "operation_key": 1 })
                .options(
                    IndexOptions::builder()
                        .name(
                            "training_plan_generation_operations_operation_key_unique".to_string(),
                        )
                        .unique(true)
                        .build(),
                )
                .build()])
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
            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(TrainingPlanError::Repository(error.to_string()))
                    }
                })?;

            if inserted {
                return Ok(TrainingPlanGenerationClaimResult::Claimed(operation));
            }

            let existing_document = collection
                .find_one(doc! { "operation_key": &document.operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(
                        "claimed training plan generation operation disappeared before reload"
                            .to_string(),
                    )
                })?;
            let existing = map_document_to_operation(existing_document)?;
            let reclaimable = match existing.status {
                WorkflowStatus::Pending => {
                    existing.last_attempt_at_epoch_seconds <= stale_before_epoch_seconds
                }
                WorkflowStatus::Failed => true,
                WorkflowStatus::Completed => false,
            };

            if !reclaimable {
                return Ok(TrainingPlanGenerationClaimResult::Existing(existing));
            }

            let reclaimed = existing.reclaim(operation.last_attempt_at_epoch_seconds);
            let reclaimed_document = map_operation_to_document(&reclaimed)?;
            let replaced = collection
                .find_one_and_replace(
                    doc! {
                        "operation_key": &document.operation_key,
                        "attempt_count": i64::from(existing.attempt_count),
                        "updated_at_epoch_seconds": existing.updated_at_epoch_seconds,
                    },
                    &reclaimed_document,
                )
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;

            if replaced.is_some() {
                return Ok(TrainingPlanGenerationClaimResult::Claimed(reclaimed));
            }

            let latest = collection
                .find_one(doc! { "operation_key": &document.operation_key })
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    TrainingPlanError::Repository(
                        "reclaimed training plan generation operation disappeared before reload"
                            .to_string(),
                    )
                })?;
            Ok(TrainingPlanGenerationClaimResult::Existing(
                map_document_to_operation(latest)?,
            ))
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
}

fn map_operation_to_document(
    operation: &TrainingPlanGenerationOperation,
) -> Result<TrainingPlanGenerationOperationDocument, TrainingPlanError> {
    Ok(TrainingPlanGenerationOperationDocument {
        operation_key: operation.operation_key.clone(),
        user_id: operation.user_id.clone(),
        workout_id: operation.workout_id.clone(),
        saved_at_epoch_seconds: operation.saved_at_epoch_seconds,
        status: map_status_to_document(&operation.status).to_string(),
        workout_recap_text: operation.workout_recap_text.clone(),
        workout_recap_provider: operation.workout_recap_provider.clone(),
        workout_recap_model: operation.workout_recap_model.clone(),
        workout_recap_generated_at_epoch_seconds: operation
            .workout_recap_generated_at_epoch_seconds,
        projection_persisted_at_epoch_seconds: operation.projection_persisted_at_epoch_seconds,
        raw_plan_response: operation.raw_plan_response.clone(),
        raw_correction_response: operation.raw_correction_response.clone(),
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
        started_at_epoch_seconds: operation.started_at_epoch_seconds,
        last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
        attempt_count: i64::from(operation.attempt_count),
        created_at_epoch_seconds: operation.created_at_epoch_seconds,
        updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
    })
}

fn map_document_to_operation(
    document: TrainingPlanGenerationOperationDocument,
) -> Result<TrainingPlanGenerationOperation, TrainingPlanError> {
    Ok(TrainingPlanGenerationOperation {
        operation_key: document.operation_key,
        user_id: document.user_id,
        workout_id: document.workout_id,
        saved_at_epoch_seconds: document.saved_at_epoch_seconds,
        status: map_document_to_status(&document.status)?,
        workout_recap_text: document.workout_recap_text,
        workout_recap_provider: document.workout_recap_provider,
        workout_recap_model: document.workout_recap_model,
        workout_recap_generated_at_epoch_seconds: document.workout_recap_generated_at_epoch_seconds,
        projection_persisted_at_epoch_seconds: document.projection_persisted_at_epoch_seconds,
        raw_plan_response: document.raw_plan_response,
        raw_correction_response: document.raw_correction_response,
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
        started_at_epoch_seconds: document.started_at_epoch_seconds,
        last_attempt_at_epoch_seconds: document.last_attempt_at_epoch_seconds,
        attempt_count: u32::try_from(document.attempt_count).map_err(|_| {
            TrainingPlanError::Repository("invalid training plan attempt count".to_string())
        })?,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
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
        recorded_at_epoch_seconds: attempt.recorded_at_epoch_seconds,
    })
}

fn map_document_to_attempt(
    document: AttemptRecordDocument,
) -> Result<AttemptRecord, TrainingPlanError> {
    Ok(AttemptRecord {
        phase: map_document_to_phase(&document.phase)?,
        attempt_number: u32::try_from(document.attempt_number)
            .map_err(|_| TrainingPlanError::Repository("invalid attempt number".to_string()))?,
        recorded_at_epoch_seconds: document.recorded_at_epoch_seconds,
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
