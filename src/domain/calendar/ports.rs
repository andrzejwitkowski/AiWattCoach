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

#[derive(Clone, Default)]
pub struct NoopPlannedWorkoutSyncRepository;

impl PlannedWorkoutSyncRepository for NoopPlannedWorkoutSyncRepository {
    fn find_by_user_id_and_projection(
        &self,
        _user_id: &str,
        _operation_key: &str,
        _date: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutSyncRecord>, CalendarError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id_and_range(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedWorkoutSyncRecord>, CalendarError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> BoxFuture<Result<PlannedWorkoutSyncRecord, CalendarError>> {
        Box::pin(async move { Ok(record) })
    }
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

pub trait HiddenCalendarEventSource: Send + Sync + 'static {
    fn list_hidden_intervals_event_ids(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<i64>, CalendarError>>;
}
