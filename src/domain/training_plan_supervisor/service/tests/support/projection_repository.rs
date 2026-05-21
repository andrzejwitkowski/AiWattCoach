use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::domain::training_plan::{
    TrainingPlanError, TrainingPlanPartialReplacement, TrainingPlanProjectedDay,
    TrainingPlanProjectionRepository, TrainingPlanReplacementResult, TrainingPlanSnapshot,
};
use crate::domain::training_plan_supervisor::TrainingPlanSupervisorStatus;

#[derive(Clone, Default)]
pub(crate) struct RecordingProjectionRepository {
    stored: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
}

impl RecordingProjectionRepository {
    pub(crate) fn seed_day(&self, day: TrainingPlanProjectedDay) {
        self.stored
            .lock()
            .expect("projection repo mutex poisoned")
            .push(day);
    }

    pub(crate) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.stored
            .lock()
            .expect("projection repo mutex poisoned")
            .clone()
    }
}

#[derive(Clone)]
pub(crate) struct FailingOnceProjectionRepository {
    inner: RecordingProjectionRepository,
    should_fail: Arc<Mutex<bool>>,
}

impl FailingOnceProjectionRepository {
    pub(crate) fn new(inner: RecordingProjectionRepository) -> Self {
        Self {
            inner,
            should_fail: Arc::new(Mutex::new(true)),
        }
    }

    pub(crate) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.inner.stored_days()
    }
}

impl TrainingPlanProjectionRepository for RecordingProjectionRepository {
    fn list_active_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "list_active_by_user_id not implemented in test".to_string(),
            ))
        })
    }

    fn find_active_by_operation_key(
        &self,
        _operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "find_active_by_operation_key not implemented in test".to_string(),
            ))
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("projection repo mutex poisoned")
                .iter()
                .filter(|day| {
                    day.user_id == user_id
                        && day.operation_key == operation_key
                        && day.superseded_at_epoch_seconds.is_none()
                })
                .cloned()
                .collect())
        })
    }

    fn replace_window(
        &self,
        _snapshot: TrainingPlanSnapshot,
        _projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<TrainingPlanReplacementResult, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "replace_window not implemented in test".to_string(),
            ))
        })
    }

    fn apply_partial_replacement(
        &self,
        replacement: TrainingPlanPartialReplacement,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let replace_dates = replacement
                .replace_dates
                .into_iter()
                .collect::<HashSet<_>>();
            let mut stored = stored.lock().expect("projection repo mutex poisoned");
            for replacement_day in replacement.projected_days {
                if !replace_dates.contains(&replacement_day.date) {
                    continue;
                }
                stored.retain(|existing| {
                    !(existing.user_id == replacement_day.user_id
                        && existing.operation_key == replacement_day.operation_key
                        && existing.date == replacement_day.date
                        && existing.superseded_at_epoch_seconds.is_none())
                });
                stored.push(replacement_day);
            }
            Ok(())
        })
    }

    fn update_supervisor_status(
        &self,
        user_id: &str,
        operation_key: &str,
        supervisor_status: Option<TrainingPlanSupervisorStatus>,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            for day in stored
                .lock()
                .expect("projection repo mutex poisoned")
                .iter_mut()
            {
                if day.user_id == user_id
                    && day.operation_key == operation_key
                    && day.superseded_at_epoch_seconds.is_none()
                {
                    day.supervisor_status = supervisor_status;
                    day.updated_at_epoch_seconds = updated_at_epoch_seconds;
                }
            }
            Ok(())
        })
    }
}

impl TrainingPlanProjectionRepository for FailingOnceProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner.list_active_by_user_id(user_id)
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner.find_active_by_operation_key(operation_key)
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner
            .find_active_by_user_id_and_operation_key(user_id, operation_key)
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<TrainingPlanReplacementResult, TrainingPlanError>,
    > {
        self.inner
            .replace_window(snapshot, projected_days, today, replaced_at_epoch_seconds)
    }

    fn apply_partial_replacement(
        &self,
        replacement: TrainingPlanPartialReplacement,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        self.inner.apply_partial_replacement(replacement)
    }

    fn update_supervisor_status(
        &self,
        user_id: &str,
        operation_key: &str,
        supervisor_status: Option<TrainingPlanSupervisorStatus>,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let should_fail = self.should_fail.clone();
        let inner = self.inner.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let fail_this_call = {
                let mut should_fail = should_fail.lock().expect("projection repo mutex poisoned");
                if *should_fail {
                    *should_fail = false;
                    true
                } else {
                    false
                }
            };
            if fail_this_call {
                return Err(TrainingPlanError::Repository(
                    "projection update failed once".to_string(),
                ));
            }
            inner
                .update_supervisor_status(
                    &user_id,
                    &operation_key,
                    supervisor_status,
                    updated_at_epoch_seconds,
                )
                .await
        })
    }
}
