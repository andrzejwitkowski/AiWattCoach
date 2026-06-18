use chrono::NaiveDate;

use crate::domain::{
    identity::{Clock, IdGenerator},
    intervals::DateRange,
};

use super::{
    parse_date, validate_past_date_changes_allowed, validate_write_range_ends_on_or_after,
    BoxFuture, CreatePlannedRestDay, PlannedRestDay, PlannedRestDayError, PlannedRestDayRepository,
    PlannedRestDayUseCases, UpdatePlannedRestDay,
};

#[derive(Clone)]
pub struct PlannedRestDayService<Repository, Time, Ids>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    repository: Repository,
    clock: Time,
    ids: Ids,
}

impl<Repository, Time, Ids> PlannedRestDayService<Repository, Time, Ids>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    pub fn new(repository: Repository, clock: Time, ids: Ids) -> Self {
        Self {
            repository,
            clock,
            ids,
        }
    }

    fn today(&self) -> NaiveDate {
        chrono::DateTime::from_timestamp(self.clock.now_epoch_seconds(), 0)
            .map(|time| time.date_naive())
            .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH.date_naive())
    }

    async fn list_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<PlannedRestDay>, PlannedRestDayError> {
        validate_query_range(range)?;
        self.repository
            .list_intersecting_range(user_id, range)
            .await
    }

    async fn get_impl(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> Result<PlannedRestDay, PlannedRestDayError> {
        self.repository
            .find_by_id(user_id, planned_rest_day_id)
            .await?
            .ok_or(PlannedRestDayError::NotFound)
    }

    async fn create_impl(
        &self,
        user_id: &str,
        request: CreatePlannedRestDay,
    ) -> Result<PlannedRestDay, PlannedRestDayError> {
        validate_write_range_ends_on_or_after(self.today(), &request.end_date)?;

        let now = self.clock.now_epoch_seconds();
        let pending =
            PlannedRestDay::pending_new(self.ids.new_id("prd"), user_id.to_string(), request, now)?;
        self.repository.upsert(pending).await
    }

    async fn update_impl(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
        request: UpdatePlannedRestDay,
    ) -> Result<PlannedRestDay, PlannedRestDayError> {
        let today = self.today();
        let existing = self
            .repository
            .find_by_id(user_id, planned_rest_day_id)
            .await?
            .ok_or(PlannedRestDayError::NotFound)?;

        validate_past_date_changes_allowed(&existing, &request, today)?;

        let existing_end = parse_date(&existing.end_date)?;
        if existing_end >= today {
            validate_write_range_ends_on_or_after(today, &request.end_date)?;
        }

        let updated = existing.mark_updated(request, self.clock.now_epoch_seconds())?;
        self.repository.upsert(updated).await
    }

    async fn delete_impl(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> Result<(), PlannedRestDayError> {
        let existing = self
            .repository
            .find_by_id(user_id, planned_rest_day_id)
            .await?
            .ok_or(PlannedRestDayError::NotFound)?;

        self.repository
            .delete(&existing.user_id, &existing.planned_rest_day_id)
            .await
    }
}

impl<Repository, Time, Ids> PlannedRestDayUseCases for PlannedRestDayService<Repository, Time, Ids>
where
    Repository: PlannedRestDayRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    fn list(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<PlannedRestDay>, PlannedRestDayError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_impl(&user_id, &range).await })
    }

    fn get(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move { service.get_impl(&user_id, &planned_rest_day_id).await })
    }

    fn create(
        &self,
        user_id: &str,
        request: CreatePlannedRestDay,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.create_impl(&user_id, request).await })
    }

    fn update(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
        request: UpdatePlannedRestDay,
    ) -> BoxFuture<Result<PlannedRestDay, PlannedRestDayError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move {
            service
                .update_impl(&user_id, &planned_rest_day_id, request)
                .await
        })
    }

    fn delete(
        &self,
        user_id: &str,
        planned_rest_day_id: &str,
    ) -> BoxFuture<Result<(), PlannedRestDayError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let planned_rest_day_id = planned_rest_day_id.to_string();
        Box::pin(async move { service.delete_impl(&user_id, &planned_rest_day_id).await })
    }
}

fn validate_query_range(range: &DateRange) -> Result<(), PlannedRestDayError> {
    parse_date(&range.oldest)?;
    parse_date(&range.newest)?;

    if range.oldest > range.newest {
        return Err(PlannedRestDayError::Validation(
            "oldest must be on or before newest".to_string(),
        ));
    }

    Ok(())
}
