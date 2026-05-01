use chrono::{TimeZone, Utc};

use crate::domain::{
    calendar_view::{select_visible_planned_workout_candidates, CalendarPlannedWorkoutSource},
    completed_workouts::{CompletedWorkoutError, CompletedWorkoutRepository},
    identity::Clock,
    planned_workouts::PlannedWorkoutError,
    races::{RaceError, RaceRepository},
    special_days::{SpecialDayError, SpecialDayRepository},
};

use super::{
    BoxFuture, CalendarEntryViewError, CalendarEntryViewRefreshPort, CalendarEntryViewRepository,
};

pub trait ManualCalendarRefreshUseCases: Send + Sync {
    fn refresh_calendar_view_for_user(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<ManualCalendarRefreshResult, CalendarEntryViewError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualCalendarRefreshResult {
    pub oldest: String,
    pub newest: String,
    pub rebuilt_entry_count: usize,
}

#[derive(Clone)]
pub struct ManualCalendarRefreshService<
    Views,
    Planned,
    Completed,
    Races,
    SpecialDays,
    Time,
    Refresh,
> where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    views: Views,
    planned_workouts: Planned,
    completed_workouts: Completed,
    races: Races,
    special_days: SpecialDays,
    clock: Time,
    refresh: Refresh,
}

impl<Views, Planned, Completed, Races, SpecialDays, Time, Refresh>
    ManualCalendarRefreshService<Views, Planned, Completed, Races, SpecialDays, Time, Refresh>
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    pub fn new(
        views: Views,
        planned_workouts: Planned,
        completed_workouts: Completed,
        races: Races,
        special_days: SpecialDays,
        clock: Time,
        refresh: Refresh,
    ) -> Self {
        Self {
            views,
            planned_workouts,
            completed_workouts,
            races,
            special_days,
            clock,
            refresh,
        }
    }
}

impl<Views, Planned, Completed, Races, SpecialDays, Time, Refresh> ManualCalendarRefreshUseCases
    for ManualCalendarRefreshService<Views, Planned, Completed, Races, SpecialDays, Time, Refresh>
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    fn refresh_calendar_view_for_user(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<ManualCalendarRefreshResult, CalendarEntryViewError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.refresh_for_user_impl(&user_id).await })
    }
}

impl<Views, Planned, Completed, Races, SpecialDays, Time, Refresh>
    ManualCalendarRefreshService<Views, Planned, Completed, Races, SpecialDays, Time, Refresh>
where
    Views: CalendarEntryViewRepository + Clone,
    Planned: CalendarPlannedWorkoutSource + Clone,
    Completed: CompletedWorkoutRepository + Clone,
    Races: RaceRepository + Clone,
    SpecialDays: SpecialDayRepository + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    async fn refresh_for_user_impl(
        &self,
        user_id: &str,
    ) -> Result<ManualCalendarRefreshResult, CalendarEntryViewError> {
        let today = epoch_seconds_to_date(self.clock.now_epoch_seconds())?;
        let (oldest, newest) = self
            .resolve_refresh_range_for_user(user_id, &today)
            .await?
            .unwrap_or_else(|| (today.clone(), today));
        let rebuilt_entries = self
            .refresh
            .refresh_range_for_user(user_id, &oldest, &newest)
            .await?;

        Ok(ManualCalendarRefreshResult {
            oldest,
            newest,
            rebuilt_entry_count: rebuilt_entries.len(),
        })
    }

    async fn resolve_refresh_range_for_user(
        &self,
        user_id: &str,
        today: &str,
    ) -> Result<Option<(String, String)>, CalendarEntryViewError> {
        let source_dates = self.list_source_dates_for_user(user_id).await?;
        let oldest_view = self.views.find_oldest_date_by_user_id(user_id).await?;
        let newest_view = self.views.find_newest_date_by_user_id(user_id).await?;

        let oldest = source_dates
            .iter()
            .chain(oldest_view.as_ref())
            .min()
            .cloned();
        let newest = source_dates
            .into_iter()
            .chain(newest_view)
            .max()
            .map(|latest| latest.max(today.to_string()));

        Ok(match (oldest, newest) {
            (Some(oldest), Some(newest)) => Some((oldest, newest)),
            _ => None,
        })
    }

    async fn list_source_dates_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<String>, CalendarEntryViewError> {
        let planned_dates = self
            .planned_workouts
            .list_candidates_by_user_id_and_date_range(user_id, "0000-01-01", "9999-12-31")
            .await
            .map_err(map_planned_error)?;
        let planned_dates = select_visible_planned_workout_candidates(planned_dates)
            .into_iter()
            .map(|candidate| candidate.workout.date)
            .collect::<Vec<_>>();

        let completed_dates = self
            .completed_workouts
            .list_by_user_id(user_id)
            .await
            .map_err(map_completed_error)?
            .into_iter()
            .filter_map(|workout| workout.start_date_local.get(..10).map(str::to_string))
            .filter(|date| is_valid_calendar_date(date))
            .collect::<Vec<_>>();

        let race_dates = self
            .races
            .list_by_user_id(user_id)
            .await
            .map_err(map_race_error)?
            .into_iter()
            .map(|race| race.date)
            .collect::<Vec<_>>();

        let special_day_dates = self
            .special_days
            .list_by_user_id(user_id)
            .await
            .map_err(map_special_day_error)?
            .into_iter()
            .map(|day| day.date)
            .collect::<Vec<_>>();

        let mut dates = Vec::new();
        dates.extend(planned_dates);
        dates.extend(completed_dates);
        dates.extend(race_dates);
        dates.extend(special_day_dates);

        Ok(dates)
    }
}

fn epoch_seconds_to_date(now_epoch_seconds: i64) -> Result<String, CalendarEntryViewError> {
    Utc.timestamp_opt(now_epoch_seconds, 0)
        .single()
        .map(|timestamp| timestamp.date_naive().format("%Y-%m-%d").to_string())
        .ok_or_else(|| {
            CalendarEntryViewError::Repository(format!(
                "invalid now_epoch_seconds for calendar refresh: {now_epoch_seconds}"
            ))
        })
}

fn is_valid_calendar_date(value: &str) -> bool {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

fn map_planned_error(error: PlannedWorkoutError) -> CalendarEntryViewError {
    match error {
        PlannedWorkoutError::Repository(message) => CalendarEntryViewError::Repository(message),
    }
}

fn map_completed_error(error: CompletedWorkoutError) -> CalendarEntryViewError {
    match error {
        CompletedWorkoutError::Repository(message) => CalendarEntryViewError::Repository(message),
    }
}

fn map_race_error(error: RaceError) -> CalendarEntryViewError {
    match error {
        RaceError::Validation(message)
        | RaceError::Unavailable(message)
        | RaceError::Internal(message) => CalendarEntryViewError::Repository(message),
        RaceError::Unauthenticated => CalendarEntryViewError::InvariantViolation(
            "manual calendar refresh encountered unauthenticated race lookup".to_string(),
        ),
        RaceError::NotFound => CalendarEntryViewError::InvariantViolation(
            "manual calendar refresh encountered not-found race lookup".to_string(),
        ),
    }
}

fn map_special_day_error(error: SpecialDayError) -> CalendarEntryViewError {
    match error {
        SpecialDayError::Validation(message) | SpecialDayError::Repository(message) => {
            CalendarEntryViewError::Repository(message)
        }
    }
}
