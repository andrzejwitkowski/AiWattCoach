use std::{future::Future, pin::Pin};

use crate::domain::intervals::DateRange;

use super::{CalendarError, CalendarEvent, PlannedWorkoutSyncRecord, SyncPlannedWorkout};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedWorkoutSyncRepository: Send + Sync + 'static {
    fn find_by_user_id_and_projection(
        &self,
        user_id: &str,
        operation_key: &str,
        date: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutSyncRecord>, CalendarError>>;

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedWorkoutSyncRecord>, CalendarError>>;

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutSyncRecord, CalendarError>>;
}

pub trait CalendarUseCases: Send + Sync {
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<CalendarEvent>, CalendarError>>;

    fn sync_planned_workout(
        &self,
        user_id: &str,
        request: SyncPlannedWorkout,
    ) -> BoxFuture<Result<CalendarEvent, CalendarError>>;
}
