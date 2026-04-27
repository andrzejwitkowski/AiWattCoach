use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{PlannedWorkoutWahooSyncError, PlannedWorkoutWahooSyncRecord};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedWorkoutWahooSyncRepository: Clone + Send + Sync + 'static {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>;

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>;

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>;

    fn upsert(
        &self,
        record: PlannedWorkoutWahooSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutWahooSyncRecord, PlannedWorkoutWahooSyncError>>;
}

#[derive(Clone, Default)]
pub struct NoopPlannedWorkoutWahooSyncRepository {
    #[cfg(test)]
    stored: Arc<Mutex<Vec<PlannedWorkoutWahooSyncRecord>>>,
}

impl PlannedWorkoutWahooSyncRepository for NoopPlannedWorkoutWahooSyncRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let planned_workout_id = planned_workout_id.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("planned workout wahoo sync repo mutex poisoned")
                    .iter()
                    .find(|record| {
                        record.user_id == user_id && record.planned_workout_id == planned_workout_id
                    })
                    .cloned())
            })
        }

        #[cfg(not(test))]
        {
            let _ = (user_id, planned_workout_id);
            Box::pin(async { Ok(None) })
        }
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("planned workout wahoo sync repo mutex poisoned")
                    .iter()
                    .find(|record| {
                        record.user_id == user_id && record.wahoo_plan_id == Some(wahoo_plan_id)
                    })
                    .cloned())
            })
        }

        #[cfg(not(test))]
        {
            let _ = (user_id, wahoo_plan_id);
            Box::pin(async { Ok(None) })
        }
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutWahooSyncRecord>, PlannedWorkoutWahooSyncError>>
    {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let wahoo_workout_token = wahoo_workout_token.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("planned workout wahoo sync repo mutex poisoned")
                    .iter()
                    .find(|record| {
                        record.user_id == user_id
                            && record.wahoo_workout_token.as_deref()
                                == Some(wahoo_workout_token.as_str())
                    })
                    .cloned())
            })
        }

        #[cfg(not(test))]
        {
            let _ = (user_id, wahoo_workout_token);
            Box::pin(async { Ok(None) })
        }
    }

    fn upsert(
        &self,
        record: PlannedWorkoutWahooSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutWahooSyncRecord, PlannedWorkoutWahooSyncError>> {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            Box::pin(async move {
                let mut stored = stored
                    .lock()
                    .expect("planned workout wahoo sync repo mutex poisoned");
                stored.retain(|existing| {
                    !(existing.user_id == record.user_id
                        && existing.planned_workout_id == record.planned_workout_id)
                });
                stored.push(record.clone());
                Ok(record)
            })
        }

        #[cfg(not(test))]
        {
            Box::pin(async move { Ok(record) })
        }
    }
}
