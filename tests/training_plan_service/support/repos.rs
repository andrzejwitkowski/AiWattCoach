use std::sync::{Arc, Mutex};

use super::{
    push_call, CallLog, TrainingPlanError, TrainingPlanGenerationClaimResult,
    TrainingPlanGenerationOperation, TrainingPlanGenerationOperationRepository,
    TrainingPlanProjectedDay, TrainingPlanProjectionRepository, TrainingPlanSnapshot,
    TrainingPlanSnapshotRepository, WorkflowStatus,
};

#[derive(Clone)]
pub(crate) struct InMemoryTrainingPlanSnapshotRepository {
    pub(crate) snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanSnapshotRepository {
    pub(crate) fn new() -> Self {
        Self {
            snapshots: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn stored_snapshots(&self) -> Vec<TrainingPlanSnapshot> {
        self.snapshots.lock().unwrap().clone()
    }
}

impl TrainingPlanSnapshotRepository for InMemoryTrainingPlanSnapshotRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanSnapshot>, TrainingPlanError>,
    > {
        let snapshot = self
            .snapshots
            .lock()
            .unwrap()
            .iter()
            .find(|snapshot| snapshot.operation_key == operation_key)
            .cloned();
        Box::pin(async move { Ok(snapshot) })
    }
}

#[derive(Clone)]
pub(crate) struct InMemoryTrainingPlanProjectedDayRepository {
    projected_days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
    snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>,
}

impl InMemoryTrainingPlanProjectedDayRepository {
    pub(crate) fn new(snapshots: Arc<Mutex<Vec<TrainingPlanSnapshot>>>) -> Self {
        Self {
            projected_days: Arc::new(Mutex::new(Vec::new())),
            snapshots,
        }
    }

    pub(crate) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.projected_days.lock().unwrap().clone()
    }

    pub(crate) fn store_snapshot_only(&self, snapshot: TrainingPlanSnapshot) {
        self.snapshots.lock().unwrap().push(snapshot);
    }
}

impl TrainingPlanProjectionRepository for InMemoryTrainingPlanProjectedDayRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let user_id = user_id.to_string();
        let snapshots = self.snapshots.clone();
        let days = self
            .projected_days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.user_id == user_id && day.superseded_at_epoch_seconds.is_none())
            .cloned()
            .collect::<Vec<_>>();
        Box::pin(async move {
            let snapshot_start_dates = snapshots
                .lock()
                .unwrap()
                .iter()
                .filter(|snapshot| snapshot.user_id == user_id)
                .map(|snapshot| (snapshot.operation_key.clone(), snapshot.start_date.clone()))
                .collect::<std::collections::HashMap<_, _>>();
            Ok(days
                .into_iter()
                .filter(|day| {
                    snapshot_start_dates
                        .get(&day.operation_key)
                        .is_some_and(|start_date| day.date > *start_date)
                })
                .collect())
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let snapshots = self.snapshots.clone();
        let days = self
            .projected_days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| {
                day.operation_key == operation_key && day.superseded_at_epoch_seconds.is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let snapshot_start_date = snapshots
                .lock()
                .unwrap()
                .iter()
                .find(|snapshot| snapshot.operation_key == operation_key)
                .map(|snapshot| snapshot.start_date.clone());
            let Some(snapshot_start_date) = snapshot_start_date else {
                return Ok(Vec::new());
            };
            Ok(days
                .into_iter()
                .filter(|day| day.date > snapshot_start_date)
                .collect())
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let snapshots = self.snapshots.clone();
        let days = self
            .projected_days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| {
                day.user_id == user_id
                    && day.operation_key == operation_key
                    && day.superseded_at_epoch_seconds.is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let operation_key = operation_key.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let snapshot_start_date = snapshots
                .lock()
                .unwrap()
                .iter()
                .find(|snapshot| {
                    snapshot.user_id == user_id && snapshot.operation_key == operation_key
                })
                .map(|snapshot| snapshot.start_date.clone());
            let Some(snapshot_start_date) = snapshot_start_date else {
                return Ok(Vec::new());
            };
            Ok(days
                .into_iter()
                .filter(|day| day.date > snapshot_start_date)
                .collect())
        })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<(TrainingPlanSnapshot, Vec<TrainingPlanProjectedDay>), TrainingPlanError>,
    > {
        let store = self.projected_days.clone();
        let snapshots = self.snapshots.clone();
        let today = today.to_string();
        Box::pin(async move {
            let mut stored = store.lock().unwrap();

            for day in stored.iter_mut() {
                if day.superseded_at_epoch_seconds.is_some() {
                    continue;
                }
                if day.user_id != snapshot.user_id {
                    continue;
                }
                if day.date < today
                    || day.date < snapshot.start_date
                    || day.date > snapshot.end_date
                {
                    continue;
                }
                day.superseded_at_epoch_seconds = Some(replaced_at_epoch_seconds);
                day.updated_at_epoch_seconds = replaced_at_epoch_seconds;
            }

            for projected_day in &projected_days {
                if let Some(existing) = stored.iter_mut().find(|existing| {
                    existing.user_id == projected_day.user_id
                        && existing.operation_key == projected_day.operation_key
                        && existing.date == projected_day.date
                }) {
                    *existing = projected_day.clone();
                } else {
                    stored.push(projected_day.clone());
                }
            }

            let mut stored_snapshots = snapshots.lock().unwrap();
            if let Some(existing) = stored_snapshots
                .iter_mut()
                .find(|existing| existing.operation_key == snapshot.operation_key)
            {
                *existing = snapshot.clone();
            } else {
                stored_snapshots.push(snapshot.clone());
            }

            Ok((snapshot, projected_days))
        })
    }
}

#[derive(Clone)]
pub(crate) struct InMemoryTrainingPlanOperationRepository {
    operations: Arc<Mutex<Vec<TrainingPlanGenerationOperation>>>,
    call_log: CallLog,
}

impl InMemoryTrainingPlanOperationRepository {
    pub(crate) fn new(call_log: CallLog) -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            call_log,
        }
    }

    pub(crate) fn stored_operation(&self) -> TrainingPlanGenerationOperation {
        self.operations
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("expected stored operation")
    }

    pub(crate) fn with_operation(
        call_log: CallLog,
        operation: TrainingPlanGenerationOperation,
    ) -> Self {
        Self {
            operations: Arc::new(Mutex::new(vec![operation])),
            call_log,
        }
    }
}

#[derive(Clone)]
pub(crate) struct FailingUpsertTrainingPlanOperationRepository {
    operation: Arc<Mutex<Option<TrainingPlanGenerationOperation>>>,
    error_message: String,
}

impl FailingUpsertTrainingPlanOperationRepository {
    pub(crate) fn new(operation: TrainingPlanGenerationOperation, error_message: &str) -> Self {
        Self {
            operation: Arc::new(Mutex::new(Some(operation))),
            error_message: error_message.to_string(),
        }
    }
}

impl TrainingPlanGenerationOperationRepository for InMemoryTrainingPlanOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>,
    > {
        let operation = self
            .operations
            .lock()
            .unwrap()
            .iter()
            .find(|operation| operation.operation_key == operation_key)
            .cloned();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationClaimResult, TrainingPlanError>,
    > {
        push_call(&self.call_log, "operation.claim_pending");
        let mut stored_operations = self.operations.lock().unwrap();
        let existing = stored_operations
            .iter()
            .find(|existing| existing.operation_key == operation.operation_key)
            .cloned();
        let result = match existing {
            None => {
                stored_operations.push(operation.clone());
                TrainingPlanGenerationClaimResult::Claimed(operation)
            }
            Some(existing)
                if existing.status == WorkflowStatus::Failed
                    || (existing.status == WorkflowStatus::Pending
                        && existing.last_attempt_at_epoch_seconds
                            <= stale_before_epoch_seconds) =>
            {
                let reclaimed = existing.reclaim(operation.last_attempt_at_epoch_seconds);
                if let Some(stored) = stored_operations
                    .iter_mut()
                    .find(|stored| stored.operation_key == reclaimed.operation_key)
                {
                    *stored = reclaimed.clone();
                }
                TrainingPlanGenerationClaimResult::Claimed(reclaimed)
            }
            Some(existing) => TrainingPlanGenerationClaimResult::Existing(existing),
        };
        Box::pin(async move { Ok(result) })
    }

    fn upsert(
        &self,
        operation: TrainingPlanGenerationOperation,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationOperation, TrainingPlanError>,
    > {
        push_call(&self.call_log, "operation.upsert");
        let store = self.operations.clone();
        Box::pin(async move {
            let mut operations = store.lock().unwrap();
            if let Some(existing) = operations
                .iter_mut()
                .find(|existing| existing.operation_key == operation.operation_key)
            {
                *existing = operation.clone();
            } else {
                operations.push(operation.clone());
            }
            Ok(operation)
        })
    }
}

impl TrainingPlanGenerationOperationRepository for FailingUpsertTrainingPlanOperationRepository {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<Option<TrainingPlanGenerationOperation>, TrainingPlanError>,
    > {
        let operation = self
            .operation
            .lock()
            .unwrap()
            .clone()
            .filter(|operation| operation.operation_key == operation_key);
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: TrainingPlanGenerationOperation,
        _stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationClaimResult, TrainingPlanError>,
    > {
        Box::pin(async move { Ok(TrainingPlanGenerationClaimResult::Claimed(operation)) })
    }

    fn upsert(
        &self,
        _operation: TrainingPlanGenerationOperation,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<
        Result<TrainingPlanGenerationOperation, TrainingPlanError>,
    > {
        let error_message = self.error_message.clone();
        Box::pin(async move { Err(TrainingPlanError::Repository(error_message)) })
    }
}
