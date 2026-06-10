use std::{future::Future, pin::Pin};

use crate::domain::intervals::DateRange;

use super::{CreatePlannedRestDay, PlannedRestDay, PlannedRestDayError, UpdatePlannedRestDay};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedRestDayRepository: Send + Sync + 'static {
    fn list_intersecting_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>>;

    fn find_by_id(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<Option<PlannedRestDay>, PlannedRestDayError>>;

    fn upsert(
        &self,
        entry: PlannedRestDay,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<(), PlannedRestDayError>>;
}

pub trait PlannedRestDayUseCases: Send + Sync {
    fn list(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>>;

    fn get(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;

    fn create(
        &self,
        user_id: &str,
        request: CreatePlannedRestDay,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;

    fn update(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
        request: UpdatePlannedRestDay,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>>;

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<(), PlannedRestDayError>>;
}
