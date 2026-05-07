use mongodb::bson::doc;

use super::super::durable_ops::{mongo_claim_pending, ClaimOutcome};
use super::mapping::{map_document_to_operation, map_operation_to_document};
use super::MongoLlmReplyOperationRepository;
use crate::domain::coach_conversation::{
    BoxFuture as CoachConversationBoxFuture, CoachConversationError,
    CoachConversationReplyClaimResult, CoachConversationReplyOperation,
    CoachConversationReplyOperationRepository,
};
use crate::domain::llm::{LlmReplyClaimResult, LlmReplyOperation, LlmReplyOperationStatus};
use crate::domain::workout_summary::{
    BoxFuture as WorkoutSummaryBoxFuture, CoachReplyClaimResult, CoachReplyOperation,
    CoachReplyOperationRepository, WorkoutSummaryError,
};

impl MongoLlmReplyOperationRepository {
    async fn do_find_by_user_message_id(
        &self,
        user_id: &str,
        scope_id: &str,
        user_message_id: &str,
    ) -> Result<Option<LlmReplyOperation>, String> {
        let collection = self.collection.clone();
        let scope_type = self.scope_type;
        let user_id = user_id.to_string();
        let scope_id = scope_id.to_string();
        let user_message_id = user_message_id.to_string();

        let document = collection
            .find_one(doc! {
                "user_id": &user_id,
                "scope_type": scope_type,
                "scope_id": &scope_id,
                "user_message_id": &user_message_id,
            })
            .await
            .map_err(|error| error.to_string())?;

        document.map(map_document_to_operation).transpose()
    }

    async fn do_claim_pending(
        &self,
        operation: LlmReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> Result<LlmReplyClaimResult, String> {
        let document = map_operation_to_document(&operation, self.scope_type);
        let scope_type = self.scope_type;
        let user_id = document.user_id.clone();
        let scope_id = document.scope_id.clone();
        let user_message_id = document.user_message_id.clone();
        let fallback_message_id = operation.reply_message_id.clone();

        mongo_claim_pending(
            &self.collection,
            document,
            operation,
            stale_before_epoch_seconds,
            || doc! { "user_id": &user_id, "scope_type": scope_type, "scope_id": &scope_id, "user_message_id": &user_message_id },
            map_document_to_operation,
            |op, s| {
                matches!(op.status, LlmReplyOperationStatus::Pending) && op.is_stale(s)
                    || matches!(op.status, LlmReplyOperationStatus::Failed)
            },
            |op| i64::from(op.attempt_count),
            |op| op.updated_at_epoch_seconds,
            |op| op.last_attempt_at_epoch_seconds,
            |existing, _pending, now| {
                let fallback = fallback_message_id.ok_or_else(|| {
                    "pending reply operation missing reserved reply message id".to_string()
                })?;
                let reclaimed = existing.reclaim(fallback, now);
                let doc = map_operation_to_document(&reclaimed, scope_type);
                Ok((reclaimed, doc))
            },
        )
        .await
        .map(|outcome| match outcome {
            ClaimOutcome::Claimed(op) => LlmReplyClaimResult::Claimed(op),
            ClaimOutcome::Existing(op) => LlmReplyClaimResult::Existing(op),
        })
    }

    async fn do_upsert(&self, operation: LlmReplyOperation) -> Result<LlmReplyOperation, String> {
        let collection = self.collection.clone();
        let document = map_operation_to_document(&operation, self.scope_type);

        collection
            .replace_one(
                doc! {
                    "user_id": &document.user_id,
                    "scope_type": &document.scope_type,
                    "scope_id": &document.scope_id,
                    "user_message_id": &document.user_message_id,
                },
                &document,
            )
            .upsert(true)
            .await
            .map_err(|error| error.to_string())?;

        Ok(operation)
    }
}

impl CoachReplyOperationRepository for MongoLlmReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        scope_id: &str,
        user_message_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let this = self.clone();
        let user_id = user_id.to_string();
        let scope_id = scope_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            this.do_find_by_user_message_id(&user_id, &scope_id, &user_message_id)
                .await
                .map_err(WorkoutSummaryError::Repository)
        })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let this = self.clone();
        Box::pin(async move {
            this.do_claim_pending(operation, stale_before_epoch_seconds)
                .await
                .map_err(WorkoutSummaryError::Repository)
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> WorkoutSummaryBoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let this = self.clone();
        Box::pin(async move {
            this.do_upsert(operation)
                .await
                .map_err(WorkoutSummaryError::Repository)
        })
    }
}

impl CoachConversationReplyOperationRepository for MongoLlmReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> CoachConversationBoxFuture<
        Result<Option<CoachConversationReplyOperation>, CoachConversationError>,
    > {
        let this = self.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            this.do_find_by_user_message_id(&user_id, &conversation_id, &user_message_id)
                .await
                .map_err(CoachConversationError::Repository)
        })
    }

    fn claim_pending(
        &self,
        operation: CoachConversationReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> CoachConversationBoxFuture<Result<CoachConversationReplyClaimResult, CoachConversationError>>
    {
        let this = self.clone();
        Box::pin(async move {
            this.do_claim_pending(operation, stale_before_epoch_seconds)
                .await
                .map_err(CoachConversationError::Repository)
        })
    }

    fn upsert(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> CoachConversationBoxFuture<Result<CoachConversationReplyOperation, CoachConversationError>>
    {
        let this = self.clone();
        Box::pin(async move {
            this.do_upsert(operation)
                .await
                .map_err(CoachConversationError::Repository)
        })
    }
}
