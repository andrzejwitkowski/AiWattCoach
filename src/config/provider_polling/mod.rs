use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use tokio::time::MissedTickBehavior;
use tracing::warn;

use crate::{
    adapters::intervals_icu::import_mapping::{
        map_activity_to_import_command, map_event_to_import_command,
    },
    adapters::wahoo::import_mapping::map_workout_to_import_command,
    domain::{
        calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
        external_sync::{
            CanonicalEntityKind, ExternalImportUseCases, ExternalProvider,
            ExternalSyncRepositoryError, ProviderPollState, ProviderPollStateRepository,
            ProviderPollStream,
        },
        identity::{Clock, IdGenerator},
        intervals::{DateRange, IntervalsApiPort, IntervalsSettingsPort},
        training_load::TrainingLoadRecomputeUseCases,
        wahoo::WahooUseCases,
        wahoo_fit_enrichment::WahooFitEnrichmentQueueUseCases,
    },
    BackgroundTaskHandle,
};

const DEFAULT_SUCCESS_INTERVAL_SECONDS: i64 = 5 * 60;
const DEFAULT_WAHOO_SUCCESS_INTERVAL_SECONDS: i64 = 3 * 60 * 60;
const DEFAULT_FAILURE_BACKOFF_SECONDS: i64 = 5 * 60;
const DEFAULT_CALENDAR_PAST_DAYS: i64 = 30;
const DEFAULT_CALENDAR_FUTURE_DAYS: i64 = 30;
const DEFAULT_COMPLETED_PAST_DAYS: i64 = 365 * 2;
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
    wahoo_service: Option<std::sync::Arc<dyn WahooUseCases>>,
    poll_states: PollStates,
    imports: Imports,
    clock: Time,
    ids: Ids,
    refresh: Refresh,
    training_load_recompute_service: Option<std::sync::Arc<dyn TrainingLoadRecomputeUseCases>>,
    wahoo_fit_enrichment_queue: Option<std::sync::Arc<dyn WahooFitEnrichmentQueueUseCases>>,
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
            wahoo_service: None,
            poll_states,
            imports,
            clock,
            ids,
            refresh: NoopCalendarEntryViewRefresh,
            training_load_recompute_service: None,
            wahoo_fit_enrichment_queue: None,
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
            wahoo_service: self.wahoo_service,
            poll_states: self.poll_states,
            imports: self.imports,
            clock: self.clock,
            ids: self.ids,
            refresh,
            training_load_recompute_service: self.training_load_recompute_service,
            wahoo_fit_enrichment_queue: self.wahoo_fit_enrichment_queue,
            success_interval_seconds: self.success_interval_seconds,
            failure_backoff_seconds: self.failure_backoff_seconds,
            calendar_past_days: self.calendar_past_days,
            calendar_future_days: self.calendar_future_days,
            completed_past_days: self.completed_past_days,
            incremental_lookback_days: self.incremental_lookback_days,
        }
    }

    pub fn with_training_load_recompute_service(
        mut self,
        training_load_recompute_service: std::sync::Arc<dyn TrainingLoadRecomputeUseCases>,
    ) -> Self {
        self.training_load_recompute_service = Some(training_load_recompute_service);
        self
    }

    pub fn with_wahoo_fit_enrichment_queue(
        mut self,
        wahoo_fit_enrichment_queue: std::sync::Arc<dyn WahooFitEnrichmentQueueUseCases>,
    ) -> Self {
        self.wahoo_fit_enrichment_queue = Some(wahoo_fit_enrichment_queue);
        self
    }

    pub fn with_wahoo_service(mut self, wahoo_service: std::sync::Arc<dyn WahooUseCases>) -> Self {
        self.wahoo_service = Some(wahoo_service);
        self
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
                let next_due_at_epoch_seconds = attempted_at_epoch_seconds
                    + self.success_interval_seconds_for_state(&attempted_state);
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
            ExternalProvider::Wahoo => self.poll_wahoo_state(state, now_epoch_seconds).await,
            _ => Err(format!(
                "provider polling is not implemented for {:?}",
                state.provider
            )),
        }
    }

    fn success_interval_seconds_for_state(&self, state: &ProviderPollState) -> i64 {
        match state.provider {
            ExternalProvider::Wahoo => DEFAULT_WAHOO_SUCCESS_INTERVAL_SECONDS,
            _ => self.success_interval_seconds,
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

    async fn poll_wahoo_state(
        &self,
        state: &ProviderPollState,
        now_epoch_seconds: i64,
    ) -> Result<Option<String>, String> {
        if state.stream != ProviderPollStream::CompletedWorkouts {
            return Err(format!(
                "provider polling is not implemented for {:?} {:?}",
                state.provider, state.stream
            ));
        }

        let wahoo_service = self
            .wahoo_service
            .as_ref()
            .ok_or_else(|| "Wahoo service is not configured".to_string())?;
        let initial_watermark =
            wahoo_initial_watermark(now_epoch_seconds, self.completed_past_days);
        let watermark = parse_wahoo_cursor(state.cursor.as_deref())?.or(initial_watermark);
        let mut page = 1usize;
        let per_page = 30usize;
        let mut workouts_to_import = Vec::new();
        let mut newest_seen_cursor = watermark.clone();

        loop {
            let list = wahoo_service
                .list_workouts(&state.user_id, page, per_page)
                .await
                .map_err(|error| error.to_string())?;

            if list.workouts.is_empty() {
                break;
            }

            let list_len = list.workouts.len();
            let mut reached_known_watermark = false;
            for workout in list.workouts {
                let updated_at = workout_sort_key(&workout)?;
                if watermark
                    .as_ref()
                    .is_some_and(|watermark| updated_at <= *watermark)
                {
                    reached_known_watermark = true;
                    break;
                }
                newest_seen_cursor = match newest_seen_cursor {
                    Some(current) => Some(std::cmp::max(current, updated_at.clone())),
                    None => Some(updated_at.clone()),
                };
                if workout.workout_summary.is_some() {
                    workouts_to_import.push(workout);
                }
            }

            if reached_known_watermark || list_len < per_page {
                break;
            }

            page += 1;
        }

        let mut newest_cursor = newest_seen_cursor;
        let mut earliest_imported_date = None::<String>;
        for workout in workouts_to_import.iter().rev() {
            let Some(command) = map_workout_to_import_command(&state.user_id, workout) else {
                continue;
            };

            let import_outcome = self
                .imports
                .import(command)
                .await
                .map_err(|error| error.to_string())?;
            if let Some(date) = workout.starts.get(..10) {
                earliest_imported_date = match earliest_imported_date {
                    Some(current) => Some(std::cmp::min(current, date.to_string())),
                    None => Some(date.to_string()),
                };
            }
            if let Err(error) = self
                .enqueue_wahoo_fit_enrichment_if_needed(state, workout, &import_outcome)
                .await
            {
                self.recompute_partial_wahoo_imports_if_needed(
                    state,
                    earliest_imported_date.as_deref(),
                    now_epoch_seconds,
                )
                .await;
                return Err(error);
            }

            let updated_at = workout_sort_key(workout)?;
            newest_cursor = match newest_cursor {
                Some(current) => Some(std::cmp::max(current, updated_at)),
                None => Some(updated_at),
            };
        }

        if let (Some(service), Some(oldest_date)) = (
            &self.training_load_recompute_service,
            earliest_imported_date.as_deref(),
        ) {
            service
                .recompute_from(&state.user_id, oldest_date, now_epoch_seconds)
                .await
                .map_err(|error| error.to_string())?;
        }

        Ok(newest_cursor)
    }

    async fn recompute_partial_wahoo_imports_if_needed(
        &self,
        state: &ProviderPollState,
        oldest_date: Option<&str>,
        now_epoch_seconds: i64,
    ) {
        if let (Some(service), Some(oldest_date)) =
            (&self.training_load_recompute_service, oldest_date)
        {
            if let Err(recompute_error) = service
                .recompute_from(&state.user_id, oldest_date, now_epoch_seconds)
                .await
            {
                warn!(
                    user_id = %state.user_id,
                    oldest_date,
                    error = %recompute_error,
                    "training load recompute failed after partial Wahoo completed workout import"
                );
            }
        }
    }

    async fn enqueue_wahoo_fit_enrichment_if_needed(
        &self,
        state: &ProviderPollState,
        workout: &crate::domain::wahoo::WahooWorkout,
        import_outcome: &crate::domain::external_sync::ExternalImportOutcome,
    ) -> Result<(), String> {
        let Some(queue) = &self.wahoo_fit_enrichment_queue else {
            return Ok(());
        };
        if import_outcome.canonical_entity.entity_kind != CanonicalEntityKind::CompletedWorkout {
            return Ok(());
        }
        let has_fit_file = workout
            .workout_summary
            .as_ref()
            .and_then(|summary| summary.file.as_ref())
            .is_some_and(|file| !file.url.trim().is_empty());
        if !has_fit_file {
            return Ok(());
        }
        queue
            .enqueue_enrichment(
                &state.user_id,
                &import_outcome.canonical_entity.entity_id,
                workout.id,
            )
            .await
            .map_err(|error| error.to_string())
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
        let mut earliest_imported_date = None::<String>;
        for activity in &activities {
            let import_activity = match self
                .intervals_api
                .get_activity(credentials, &activity.id)
                .await
            {
                Ok(detailed) => detailed,
                Err(crate::domain::intervals::IntervalsError::NotFound) => {
                    warn!(
                        user_id = %state.user_id,
                        activity_id = %activity.id,
                        error_kind = "not_found",
                        "completed workout enrichment not found; importing listed activity without full details"
                    );
                    activity.clone()
                }
                Err(error) => {
                    let error_kind = match &error {
                        crate::domain::intervals::IntervalsError::Unauthenticated => {
                            "unauthenticated"
                        }
                        crate::domain::intervals::IntervalsError::CredentialsNotConfigured => {
                            "credentials_not_configured"
                        }
                        crate::domain::intervals::IntervalsError::ApiError(_) => "api_error",
                        crate::domain::intervals::IntervalsError::ConnectionError(_) => {
                            "connection_error"
                        }
                        crate::domain::intervals::IntervalsError::NotFound => "not_found",
                        crate::domain::intervals::IntervalsError::Internal(_) => "internal",
                    };
                    warn!(
                        user_id = %state.user_id,
                        activity_id = %activity.id,
                        error_kind,
                        error = %error,
                        "completed workout enrichment failed"
                    );
                    if let (Some(service), Some(oldest_date)) = (
                        &self.training_load_recompute_service,
                        earliest_imported_date.as_deref(),
                    ) {
                        if let Err(recompute_error) = service
                            .recompute_from(&state.user_id, oldest_date, now_epoch_seconds)
                            .await
                        {
                            warn!(
                                user_id = %state.user_id,
                                oldest_date,
                                error = %recompute_error,
                                "training load recompute failed after partial completed workout import"
                            );
                        }
                    }
                    return Err(format!(
                        "completed workout enrichment failed for activity {}: {}",
                        activity.id, error
                    ));
                }
            };
            if let Err(error) = self
                .imports
                .import(map_activity_to_import_command(
                    &state.user_id,
                    &import_activity,
                ))
                .await
            {
                if let (Some(service), Some(oldest_date)) = (
                    &self.training_load_recompute_service,
                    earliest_imported_date.as_deref(),
                ) {
                    if let Err(recompute_error) = service
                        .recompute_from(&state.user_id, oldest_date, now_epoch_seconds)
                        .await
                    {
                        warn!(
                            user_id = %state.user_id,
                            oldest_date,
                            error = %recompute_error,
                            "training load recompute failed after partial completed workout import"
                        );
                    }
                }
                return Err(error.to_string());
            }

            if let Some(date) = import_activity.start_date_local.get(..10) {
                earliest_imported_date = match earliest_imported_date {
                    Some(current) => Some(std::cmp::min(current, date.to_string())),
                    None => Some(date.to_string()),
                };
            }
        }
        if let (Some(service), Some(oldest_date)) = (
            &self.training_load_recompute_service,
            earliest_imported_date.as_deref(),
        ) {
            service
                .recompute_from(&state.user_id, oldest_date, now_epoch_seconds)
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
) -> BackgroundTaskHandle
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    PollStates: ProviderPollStateRepository,
    Imports: ExternalImportUseCases,
    Time: Clock,
    Ids: IdGenerator,
    Refresh: CalendarEntryViewRefreshPort,
{
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let join_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(DEFAULT_LOOP_INTERVAL_SECONDS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(error) = service.poll_due_once().await {
                        warn!(%error, "provider polling loop failed to list due streams");
                    }
                }
            }
        }
    });

    BackgroundTaskHandle::new("provider-polling", shutdown_tx, join_handle)
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

fn parse_wahoo_cursor(cursor: Option<&str>) -> Result<Option<String>, String> {
    match cursor {
        None => Ok(None),
        Some(cursor) if cursor.trim().is_empty() => Ok(None),
        Some(cursor) => parse_wahoo_timestamp(cursor)
            .map(Some)
            .map_err(|error| format!("invalid Wahoo poll cursor '{cursor}': {error}")),
    }
}

fn wahoo_initial_watermark(now_epoch_seconds: i64, completed_past_days: i64) -> Option<String> {
    let today = epoch_seconds_to_date(now_epoch_seconds);
    let bootstrap_date = today - ChronoDuration::days(completed_past_days);
    bootstrap_date
        .and_hms_opt(0, 0, 0)
        .map(|datetime| DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).to_rfc3339())
}

fn workout_sort_key(workout: &crate::domain::wahoo::WahooWorkout) -> Result<String, String> {
    let raw = workout
        .workout_summary
        .as_ref()
        .and_then(|summary| summary.updated_at.as_deref())
        .or(workout.updated_at.as_deref())
        .unwrap_or(workout.starts.as_str());
    parse_wahoo_timestamp(raw)
}

fn parse_wahoo_timestamp(value: &str) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc).to_rfc3339())
        .map_err(|error| error.to_string())
}

fn advance_calendar_cursor(
    state: &ProviderPollState,
    events: &[crate::domain::intervals::Event],
    range: &DateRange,
) -> Option<String> {
    if !events.is_empty() {
        return Some(range.newest.clone());
    }

    state.cursor.clone().or_else(|| Some(range.newest.clone()))
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
