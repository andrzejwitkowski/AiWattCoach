use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use tokio::time::MissedTickBehavior;
use tracing::warn;

use crate::{
    adapters::intervals_icu::import_mapping::{
        map_activity_to_import_command, map_event_to_import_command,
    },
    domain::{
        calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
        external_sync::{
            ExternalImportUseCases, ExternalProvider, ExternalSyncRepositoryError,
            ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
        },
        identity::{Clock, IdGenerator},
        intervals::{DateRange, IntervalsApiPort, IntervalsSettingsPort},
    },
};

const DEFAULT_SUCCESS_INTERVAL_SECONDS: i64 = 5 * 60;
const DEFAULT_FAILURE_BACKOFF_SECONDS: i64 = 5 * 60;
const DEFAULT_CALENDAR_PAST_DAYS: i64 = 30;
const DEFAULT_CALENDAR_FUTURE_DAYS: i64 = 30;
const DEFAULT_COMPLETED_PAST_DAYS: i64 = 30;
const DEFAULT_INCREMENTAL_LOOKBACK_DAYS: i64 = 2;
const DEFAULT_LOOP_INTERVAL_SECONDS: u64 = 60;

#[derive(Clone)]
pub struct ProviderPollingService<
    Api,
    Settings,
    PollStates,
    Imports,
    Time,
    Ids,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    PollStates: ProviderPollStateRepository,
    Imports: ExternalImportUseCases,
    Time: Clock,
    Ids: IdGenerator,
    Refresh: CalendarEntryViewRefreshPort,
{
    intervals_api: Api,
    intervals_settings: Settings,
    poll_states: PollStates,
    imports: Imports,
    clock: Time,
    ids: Ids,
    refresh: Refresh,
    success_interval_seconds: i64,
    failure_backoff_seconds: i64,
    calendar_past_days: i64,
    calendar_future_days: i64,
    completed_past_days: i64,
    incremental_lookback_days: i64,
}

impl<Api, Settings, PollStates, Imports, Time, Ids>
    ProviderPollingService<
        Api,
        Settings,
        PollStates,
        Imports,
        Time,
        Ids,
        NoopCalendarEntryViewRefresh,
    >
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    PollStates: ProviderPollStateRepository,
    Imports: ExternalImportUseCases,
    Time: Clock,
    Ids: IdGenerator,
{
    pub fn new(
        intervals_api: Api,
        intervals_settings: Settings,
        poll_states: PollStates,
        imports: Imports,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            intervals_api,
            intervals_settings,
            poll_states,
            imports,
            clock,
            ids,
            refresh: NoopCalendarEntryViewRefresh,
            success_interval_seconds: DEFAULT_SUCCESS_INTERVAL_SECONDS,
            failure_backoff_seconds: DEFAULT_FAILURE_BACKOFF_SECONDS,
            calendar_past_days: DEFAULT_CALENDAR_PAST_DAYS,
            calendar_future_days: DEFAULT_CALENDAR_FUTURE_DAYS,
            completed_past_days: DEFAULT_COMPLETED_PAST_DAYS,
            incremental_lookback_days: DEFAULT_INCREMENTAL_LOOKBACK_DAYS,
        }
    }
}

impl<Api, Settings, PollStates, Imports, Time, Ids, Refresh>
    ProviderPollingService<Api, Settings, PollStates, Imports, Time, Ids, Refresh>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    PollStates: ProviderPollStateRepository,
    Imports: ExternalImportUseCases,
    Time: Clock,
    Ids: IdGenerator,
    Refresh: CalendarEntryViewRefreshPort,
{
    pub fn with_calendar_view_refresh<NextRefresh>(
        self,
        refresh: NextRefresh,
    ) -> ProviderPollingService<Api, Settings, PollStates, Imports, Time, Ids, NextRefresh>
    where
        NextRefresh: CalendarEntryViewRefreshPort,
    {
        ProviderPollingService {
            intervals_api: self.intervals_api,
            intervals_settings: self.intervals_settings,
            poll_states: self.poll_states,
            imports: self.imports,
            clock: self.clock,
            ids: self.ids,
            refresh,
            success_interval_seconds: self.success_interval_seconds,
            failure_backoff_seconds: self.failure_backoff_seconds,
            calendar_past_days: self.calendar_past_days,
            calendar_future_days: self.calendar_future_days,
            completed_past_days: self.completed_past_days,
            incremental_lookback_days: self.incremental_lookback_days,
        }
    }

    #[cfg(test)]
    fn with_timing(mut self, success_interval_seconds: i64, failure_backoff_seconds: i64) -> Self {
        self.success_interval_seconds = success_interval_seconds;
        self.failure_backoff_seconds = failure_backoff_seconds;
        self
    }

    #[cfg(test)]
    fn with_windows(
        mut self,
        calendar_past_days: i64,
        calendar_future_days: i64,
        completed_past_days: i64,
    ) -> Self {
        self.calendar_past_days = calendar_past_days;
        self.calendar_future_days = calendar_future_days;
        self.completed_past_days = completed_past_days;
        self
    }

    #[cfg(test)]
    fn with_incremental_lookback(mut self, incremental_lookback_days: i64) -> Self {
        self.incremental_lookback_days = incremental_lookback_days;
        self
    }

    pub async fn poll_due_once(&self) -> Result<usize, ExternalSyncRepositoryError> {
        let now_epoch_seconds = self.clock.now_epoch_seconds();
        let due_states = self.poll_states.list_due(now_epoch_seconds).await?;

        for state in &due_states {
            self.process_due_state(state.clone()).await;
        }

        Ok(due_states.len())
    }

    async fn process_due_state(&self, state: ProviderPollState) {
        let attempted_at_epoch_seconds = self.clock.now_epoch_seconds();
        let attempted_state = state.clone().mark_attempted(attempted_at_epoch_seconds);
        let attempted_state = match self.poll_states.upsert(attempted_state).await {
            Ok(state) => state,
            Err(error) => {
                warn!(
                    user_id = %state.user_id,
                    provider = ?state.provider,
                    stream = ?state.stream,
                    error = %error,
                    "failed to persist provider poll attempt"
                );
                return;
            }
        };

        match self
            .poll_state(&attempted_state, attempted_at_epoch_seconds)
            .await
        {
            Ok(cursor) => {
                let next_due_at_epoch_seconds =
                    attempted_at_epoch_seconds + self.success_interval_seconds;
                if let Err(error) = self
                    .poll_states
                    .upsert(attempted_state.mark_succeeded(
                        cursor,
                        attempted_at_epoch_seconds,
                        next_due_at_epoch_seconds,
                    ))
                    .await
                {
                    warn!(
                        user_id = %state.user_id,
                        provider = ?state.provider,
                        stream = ?state.stream,
                        error = %error,
                        "failed to persist provider poll success"
                    );
                }
            }
            Err(error_message) => {
                let backoff_until_epoch_seconds =
                    attempted_at_epoch_seconds + self.failure_backoff_seconds;
                if let Err(error) = self
                    .poll_states
                    .upsert(attempted_state.mark_failed(
                        error_message,
                        attempted_at_epoch_seconds,
                        backoff_until_epoch_seconds,
                        Some(backoff_until_epoch_seconds),
                    ))
                    .await
                {
                    warn!(
                        user_id = %state.user_id,
                        provider = ?state.provider,
                        stream = ?state.stream,
                        error = %error,
                        "failed to persist provider poll failure"
                    );
                }
            }
        }
    }

    async fn poll_state(
        &self,
        state: &ProviderPollState,
        now_epoch_seconds: i64,
    ) -> Result<Option<String>, String> {
        match state.provider {
            ExternalProvider::Intervals => {
                self.poll_intervals_state(state, now_epoch_seconds).await
            }
            _ => Err(format!(
                "provider polling is not implemented for {:?}",
                state.provider
            )),
        }
    }

    async fn poll_intervals_state(
        &self,
        state: &ProviderPollState,
        now_epoch_seconds: i64,
    ) -> Result<Option<String>, String> {
        let credentials = self
            .intervals_settings
            .get_credentials(&state.user_id)
            .await
            .map_err(|error| error.to_string())?;

        match state.stream {
            ProviderPollStream::Calendar => {
                self.poll_intervals_calendar_stream(state, &credentials, now_epoch_seconds)
                    .await
            }
            ProviderPollStream::CompletedWorkouts => {
                self.poll_intervals_completed_workouts_stream(
                    state,
                    &credentials,
                    now_epoch_seconds,
                )
                .await
            }
        }
    }

    async fn poll_intervals_calendar_stream(
        &self,
        state: &ProviderPollState,
        credentials: &crate::domain::intervals::IntervalsCredentials,
        now_epoch_seconds: i64,
    ) -> Result<Option<String>, String> {
        let range = self.calendar_poll_range(state, now_epoch_seconds)?;
        let events = self
            .intervals_api
            .list_events(credentials, &range)
            .await
            .map_err(|error| error.to_string())?;
        for event in &events {
            match map_event_to_import_command(&state.user_id, event, &self.ids) {
                Ok(Some(command)) => {
                    self.imports
                        .import(command)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Ok(None) => {}
                Err(error) => warn!(
                    user_id = %state.user_id,
                    event_id = event.id,
                    error = %error,
                    "skipping intervals event that could not be normalized for import"
                ),
            }
        }
        let cursor = advance_calendar_cursor(state, &events, &range);
        self.refresh_full_range_on_initial_sync(state, &range)
            .await?;
        Ok(cursor)
    }

    async fn poll_intervals_completed_workouts_stream(
        &self,
        state: &ProviderPollState,
        credentials: &crate::domain::intervals::IntervalsCredentials,
        now_epoch_seconds: i64,
    ) -> Result<Option<String>, String> {
        let range = self.completed_workout_poll_range(state, now_epoch_seconds)?;
        let activities = self
            .intervals_api
            .list_activities(credentials, &range)
            .await
            .map_err(|error| error.to_string())?;
        for activity in &activities {
            self.imports
                .import(map_activity_to_import_command(&state.user_id, activity))
                .await
                .map_err(|error| error.to_string())?;
        }
        let cursor = advance_completed_workout_cursor(state, &activities, &range);
        self.refresh_full_range_on_initial_sync(state, &range)
            .await?;
        Ok(cursor)
    }

    async fn refresh_full_range_on_initial_sync(
        &self,
        state: &ProviderPollState,
        range: &DateRange,
    ) -> Result<(), String> {
        if state.cursor.is_none() {
            self.refresh
                .refresh_range_for_user(&state.user_id, &range.oldest, &range.newest)
                .await
                .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    fn calendar_poll_range(
        &self,
        state: &ProviderPollState,
        now_epoch_seconds: i64,
    ) -> Result<DateRange, String> {
        let today = epoch_seconds_to_date(now_epoch_seconds);
        if state.cursor.is_none() {
            return Ok(DateRange {
                oldest: format_date(today - ChronoDuration::days(self.calendar_past_days)),
                newest: format_date(today + ChronoDuration::days(self.calendar_future_days)),
            });
        }

        let cursor = parse_date_cursor(state.cursor.as_deref())?;
        Ok(DateRange {
            oldest: format_date(cursor - ChronoDuration::days(self.incremental_lookback_days)),
            newest: format_date(today + ChronoDuration::days(self.calendar_future_days)),
        })
    }

    fn completed_workout_poll_range(
        &self,
        state: &ProviderPollState,
        now_epoch_seconds: i64,
    ) -> Result<DateRange, String> {
        let today = epoch_seconds_to_date(now_epoch_seconds);
        if state.cursor.is_none() {
            return Ok(DateRange {
                oldest: format_date(today - ChronoDuration::days(self.completed_past_days)),
                newest: format_date(today),
            });
        }

        let cursor = parse_date_cursor(state.cursor.as_deref())?;
        Ok(DateRange {
            oldest: format_date(cursor - ChronoDuration::days(self.incremental_lookback_days)),
            newest: format_date(today),
        })
    }
}

pub fn spawn_provider_polling_loop<Api, Settings, PollStates, Imports, Time, Ids, Refresh>(
    service: ProviderPollingService<Api, Settings, PollStates, Imports, Time, Ids, Refresh>,
) where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    PollStates: ProviderPollStateRepository,
    Imports: ExternalImportUseCases,
    Time: Clock,
    Ids: IdGenerator,
    Refresh: CalendarEntryViewRefreshPort,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(DEFAULT_LOOP_INTERVAL_SECONDS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            if let Err(error) = service.poll_due_once().await {
                warn!(%error, "provider polling loop failed to list due streams");
            }
        }
    });
}

fn epoch_seconds_to_date(epoch_seconds: i64) -> NaiveDate {
    DateTime::<Utc>::from_timestamp(epoch_seconds, 0)
        .map(|value| value.date_naive())
        .unwrap_or_else(|| DateTime::<Utc>::UNIX_EPOCH.date_naive())
}

fn format_date(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn parse_date_cursor(cursor: Option<&str>) -> Result<NaiveDate, String> {
    let cursor = cursor.ok_or_else(|| "missing poll cursor".to_string())?;
    NaiveDate::parse_from_str(cursor, "%Y-%m-%d")
        .map_err(|error| format!("invalid poll cursor '{cursor}': {error}"))
}

fn advance_calendar_cursor(
    state: &ProviderPollState,
    events: &[crate::domain::intervals::Event],
    range: &DateRange,
) -> Option<String> {
    if events.is_empty() {
        return state.cursor.clone().or_else(|| Some(range.newest.clone()));
    }

    Some(range.newest.clone())
}

fn advance_completed_workout_cursor(
    state: &ProviderPollState,
    activities: &[crate::domain::intervals::Activity],
    range: &DateRange,
) -> Option<String> {
    let newest_seen = activities
        .iter()
        .filter_map(|activity| activity.start_date_local.get(..10).map(ToString::to_string))
        .max();
    newest_seen
        .or_else(|| state.cursor.clone())
        .or_else(|| Some(range.newest.clone()))
}

#[cfg(test)]
mod tests;
