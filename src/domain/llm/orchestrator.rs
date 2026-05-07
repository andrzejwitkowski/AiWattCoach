use super::{BoxFuture, LlmReplyClaimResult, LlmReplyOperation, LlmReplyOperationStatus};

pub(crate) enum ResolvedLlmReplyOperation<Reply, Error> {
    Continue(Box<LlmReplyOperation>),
    Reply(Reply),
    Error(Error),
}

pub(crate) trait LlmReplyResolutionWorkflow {
    type Reply;
    type Error;

    fn stale_before_epoch_seconds(&self) -> i64;

    fn claim_pending(
        &self,
        operation: LlmReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<LlmReplyClaimResult, Self::Error>>;

    fn recover_pending_operation(
        &self,
        operation: &LlmReplyOperation,
    ) -> BoxFuture<Result<Option<Self::Reply>, Self::Error>>;

    fn get_completed_reply(
        &self,
        operation: LlmReplyOperation,
    ) -> BoxFuture<Result<Self::Reply, Self::Error>>;

    fn map_existing_llm_failure(&self, operation: LlmReplyOperation) -> Self::Error;

    fn reply_already_pending_error(&self) -> Self::Error;
}

pub(crate) async fn resolve_llm_reply_operation<W>(
    workflow: &W,
    pending_operation: LlmReplyOperation,
) -> Result<ResolvedLlmReplyOperation<W::Reply, W::Error>, W::Error>
where
    W: LlmReplyResolutionWorkflow,
{
    match workflow
        .claim_pending(pending_operation, workflow.stale_before_epoch_seconds())
        .await?
    {
        LlmReplyClaimResult::Claimed(operation) => {
            if let Some(reply) = workflow.recover_pending_operation(&operation).await? {
                return Ok(ResolvedLlmReplyOperation::Reply(reply));
            }

            Ok(ResolvedLlmReplyOperation::Continue(Box::new(operation)))
        }
        LlmReplyClaimResult::Existing(existing) => match existing.status {
            LlmReplyOperationStatus::Completed => Ok(ResolvedLlmReplyOperation::Reply(
                workflow.get_completed_reply(existing).await?,
            )),
            LlmReplyOperationStatus::Failed => Ok(ResolvedLlmReplyOperation::Error(
                workflow.map_existing_llm_failure(existing),
            )),
            LlmReplyOperationStatus::Pending => {
                if let Some(reply) = workflow.recover_pending_operation(&existing).await? {
                    return Ok(ResolvedLlmReplyOperation::Reply(reply));
                }

                Ok(ResolvedLlmReplyOperation::Error(
                    workflow.reply_already_pending_error(),
                ))
            }
        },
    }
}
