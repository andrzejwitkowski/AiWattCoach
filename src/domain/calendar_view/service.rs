use crate::domain::{
    completed_workouts::CompletedWorkout, external_sync::ExternalSyncState,
    planned_workouts::PlannedWorkout, races::Race, special_days::SpecialDay,
};

use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, rebuild_calendar_entries, BoxFuture, CalendarEntryView,
    CalendarEntryViewError, CalendarEntryViewRepository,
};

#[derive(Clone)]
pub struct CalendarEntryViewService<Repository>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
{
    repository: Repository,
}

impl<Repository> CalendarEntryViewService<Repository>
where
    Repository: CalendarEntryViewRepository + Clone + 'static,
{
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    pub fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            repository
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await
        })
    }

    pub fn upsert_planned_workout(
        &self,
        workout: &PlannedWorkout,
        sync_state: Option<&ExternalSyncState>,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_planned_workout_entry(workout, sync_state);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_completed_workout(
        &self,
        workout: &CompletedWorkout,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_completed_workout_entry(workout);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_race(
        &self,
        race: &Race,
        sync_state: Option<&ExternalSyncState>,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_race_entry(race, sync_state);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn upsert_special_day(
        &self,
        special_day: &SpecialDay,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let entry = project_special_day_entry(special_day);
        Box::pin(async move { repository.upsert(entry).await })
    }

    pub fn rebuild_for_user(
        &self,
        user_id: &str,
        planned_workouts: &[PlannedWorkout],
        completed_workouts: &[CompletedWorkout],
        races: &[Race],
        special_days: &[SpecialDay],
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let entries =
            rebuild_calendar_entries(planned_workouts, completed_workouts, races, special_days);
        Box::pin(async move { repository.replace_all_for_user(&user_id, entries).await })
    }
}
