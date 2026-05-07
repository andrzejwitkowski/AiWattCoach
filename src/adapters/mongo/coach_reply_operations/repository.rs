use super::super::error::is_duplicate_key_error;
use super::mapping::{map_document_to_operation, map_operation_to_document};
use super::MongoCoachReplyOperationRepository;
use crate::domain::workout_summary::{
    BoxFuture, CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository,
    CoachReplyOperationStatus, WorkoutSummaryError,
};
use mongodb::bson::doc;

impl CoachReplyOperationRepository for MongoCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "workout_id": &workout_id,
                    "user_message_id": &user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            document.map(map_document_to_operation).transpose()
        })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation);

            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(WorkoutSummaryError::Repository(error.to_string()))
                    }
                })?;

            if inserted {
                return Ok(CoachReplyClaimResult::Claimed(operation));
            }

            let existing_document = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "workout_id": &document.workout_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "claimed coach reply operation disappeared before reload".to_string(),
                    )
                })?;

            let existing = map_document_to_operation(existing_document)?;
            let reclaimable = match existing.status {
                CoachReplyOperationStatus::Pending => existing.is_stale(stale_before_epoch_seconds),
                CoachReplyOperationStatus::Failed => true,
                CoachReplyOperationStatus::Completed => false,
            };

            if !reclaimable {
                return Ok(CoachReplyClaimResult::Existing(existing));
            }

            let fallback_coach_message_id =
                operation.reply_message_id.clone().ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "pending coach reply operation missing reserved coach message id"
                            .to_string(),
                    )
                })?;
            let reclaimed = existing.reclaim(
                fallback_coach_message_id,
                operation.last_attempt_at_epoch_seconds,
            );
            let reclaimed_document = map_operation_to_document(&reclaimed);
            let replaced = collection
                .find_one_and_replace(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                        "attempt_count": i64::from(existing.attempt_count),
                        "updated_at_epoch_seconds": existing.updated_at_epoch_seconds,
                    },
                    &reclaimed_document,
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if replaced.is_some() {
                return Ok(CoachReplyClaimResult::Claimed(reclaimed));
            }

            let latest = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "workout_id": &document.workout_id,
                    "user_message_id": &document.user_message_id,
                })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
                .ok_or_else(|| {
                    WorkoutSummaryError::Repository(
                        "reclaimed coach reply operation disappeared before reload".to_string(),
                    )
                })?;

            Ok(CoachReplyClaimResult::Existing(map_document_to_operation(
                latest,
            )?))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = map_operation_to_document(&operation);

            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "workout_id": &document.workout_id,
                        "user_message_id": &document.user_message_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            Ok(operation)
        })
    }
}
