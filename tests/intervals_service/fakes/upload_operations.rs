use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    ActivityUploadOperation, ActivityUploadOperationClaimResult,
    ActivityUploadOperationRepositoryPort, ActivityUploadOperationStatus, IntervalsError,
};

use crate::common::BoxFuture;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UploadOperationRepoCall {
    ClaimPending(String),
    FindByOperationKey(String),
    Upsert(String, ActivityUploadOperationStatus),
}

#[derive(Clone, Default)]
pub(crate) struct FakeActivityUploadOperationRepository {
    stored: Arc<Mutex<HashMap<String, Vec<ActivityUploadOperation>>>>,
    pub(crate) call_log: Arc<Mutex<Vec<UploadOperationRepoCall>>>,
}

impl FakeActivityUploadOperationRepository {
    pub(crate) fn with_existing(user_id: &str, operation: ActivityUploadOperation) -> Self {
        let mut stored = HashMap::new();
        stored.insert(user_id.to_string(), vec![operation]);
        Self {
            stored: Arc::new(Mutex::new(stored)),
            ..Self::default()
        }
    }
}

impl ActivityUploadOperationRepositoryPort for FakeActivityUploadOperationRepository {
    fn claim_pending(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperationClaimResult, IntervalsError>> {
        let store = self.stored.clone();
        let call_log = self.call_log.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            call_log
                .lock()
                .unwrap()
                .push(UploadOperationRepoCall::ClaimPending(
                    operation.operation_key.clone(),
                ));

            let mut store = store.lock().unwrap();
            let operations = store.entry(user_id).or_default();
            if let Some(index) = operations
                .iter()
                .position(|existing| existing.operation_key == operation.operation_key)
            {
                let existing = operations[index].clone();
                if existing.status == ActivityUploadOperationStatus::Failed {
                    operations[index] = operation.clone();
                    return Ok(ActivityUploadOperationClaimResult::Claimed(operation));
                }

                return Ok(ActivityUploadOperationClaimResult::Existing(existing));
            }

            operations.push(operation.clone());
            Ok(ActivityUploadOperationClaimResult::Claimed(operation))
        })
    }

    fn find_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<ActivityUploadOperation>, IntervalsError>> {
        let store = self.stored.clone();
        let call_log = self.call_log.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            call_log
                .lock()
                .unwrap()
                .push(UploadOperationRepoCall::FindByOperationKey(
                    operation_key.clone(),
                ));
            Ok(store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|operation| operation.operation_key == operation_key))
        })
    }

    fn upsert(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperation, IntervalsError>> {
        let store = self.stored.clone();
        let call_log = self.call_log.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            call_log
                .lock()
                .unwrap()
                .push(UploadOperationRepoCall::Upsert(
                    operation.operation_key.clone(),
                    operation.status.clone(),
                ));
            let mut store = store.lock().unwrap();
            let operations = store.entry(user_id).or_default();
            operations.retain(|existing| existing.operation_key != operation.operation_key);
            operations.push(operation.clone());
            Ok(operation)
        })
    }
}
