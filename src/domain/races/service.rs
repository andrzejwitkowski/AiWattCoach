use crate::domain::{
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository, NoopProviderPollStateRepository,
        ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
    },
    identity::{Clock, IdGenerator},
    intervals::{CreateEvent, DateRange, IntervalsError, IntervalsUseCases, UpdateEvent},
};
use tracing::warn;

use super::{BoxFuture, CreateRace, Race, RaceError, RaceRepository, RaceUseCases, UpdateRace};

#[derive(Clone)]
pub struct RaceService<
    Repository,
    Intervals,
    SyncStates,
    Time,
    Ids,
    PollStates = NoopProviderPollStateRepository,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
    PollStates: ProviderPollStateRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    repository: Repository,
    intervals: Intervals,
    sync_states: SyncStates,
    clock: Time,
    ids: Ids,
    poll_states: PollStates,
    refresh: Refresh,
}

impl<Repository, Intervals, SyncStates, Time, Ids>
    RaceService<Repository, Intervals, SyncStates, Time, Ids>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    pub fn new(
        repository: Repository,
        intervals: Intervals,
        sync_states: SyncStates,
        clock: Time,
        ids: Ids,
    ) -> Self {
        Self {
            repository,
            intervals,
            sync_states,
            clock,
            ids,
            poll_states: NoopProviderPollStateRepository,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Repository, Intervals, SyncStates, Time, Ids, PollStates, Refresh>
    RaceService<Repository, Intervals, SyncStates, Time, Ids, PollStates, Refresh>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
    PollStates: ProviderPollStateRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    pub fn with_provider_poll_states<NewPollStates>(
        self,
        poll_states: NewPollStates,
    ) -> RaceService<Repository, Intervals, SyncStates, Time, Ids, NewPollStates, Refresh>
    where
        NewPollStates: ProviderPollStateRepository + Clone + 'static,
    {
        RaceService {
            repository: self.repository,
            intervals: self.intervals,
            sync_states: self.sync_states,
            clock: self.clock,
            ids: self.ids,
            poll_states,
            refresh: self.refresh,
        }
    }

    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> RaceService<Repository, Intervals, SyncStates, Time, Ids, PollStates, NewRefresh>
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone + 'static,
    {
        RaceService {
            repository: self.repository,
            intervals: self.intervals,
            sync_states: self.sync_states,
            clock: self.clock,
            ids: self.ids,
            poll_states: self.poll_states,
            refresh,
        }
    }

    async fn mark_calendar_poll_due_soon(&self, user_id: &str) {
        let now = self.clock.now_epoch_seconds();
        let existing_state = match self
            .poll_states
            .find_by_provider_and_stream(
                user_id,
                ExternalProvider::Intervals,
                ProviderPollStream::Calendar,
            )
            .await
        {
            Ok(state) => state,
            Err(error) => {
                warn!(%user_id, %error, "race sync succeeded but failed to load provider poll state");
                return;
            }
        };

        let state = existing_state.unwrap_or_else(|| {
            ProviderPollState::new(
                user_id.to_string(),
                ExternalProvider::Intervals,
                ProviderPollStream::Calendar,
                now,
            )
        });

        if let Err(error) = self.poll_states.upsert(state.mark_due_soon(now)).await {
            warn!(%user_id, %error, "race sync succeeded but failed to mark calendar poll due soon");
        }
    }

    async fn refresh_race_date(&self, user_id: &str, date: &str) {
        if let Err(error) = self
            .refresh
            .refresh_range_for_user(user_id, date, date)
            .await
        {
            warn!(%user_id, %date, %error, "race write succeeded but calendar view refresh failed");
        }
    }

    async fn list_races_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Race>, RaceError> {
        self.repository
            .list_by_user_id_and_range(user_id, range)
            .await
    }

    async fn get_race_impl(&self, user_id: &str, race_id: &str) -> Result<Race, RaceError> {
        self.repository
            .find_by_user_id_and_race_id(user_id, race_id)
            .await?
            .ok_or(RaceError::NotFound)
    }

    async fn create_race_impl(
        &self,
        user_id: &str,
        request: CreateRace,
    ) -> Result<Race, RaceError> {
        validate_request(&request.date, &request.name, request.distance_meters)?;

        let now = self.clock.now_epoch_seconds();
        let pending = Race::pending_new(self.ids.new_id("race"), user_id.to_string(), request, now);
        let pending = self.repository.upsert(pending).await?;
        let result = self.sync_pending_race(pending.clone()).await;
        self.refresh_race_date(&pending.user_id, &pending.date)
            .await;
        result
    }

    async fn update_race_impl(
        &self,
        user_id: &str,
        race_id: &str,
        request: UpdateRace,
    ) -> Result<Race, RaceError> {
        validate_request(&request.date, &request.name, request.distance_meters)?;

        let existing = self
            .repository
            .find_by_user_id_and_race_id(user_id, race_id)
            .await?
            .ok_or(RaceError::NotFound)?;
        let pending = self
            .repository
            .upsert(existing.mark_pending_update(request, self.clock.now_epoch_seconds()))
            .await?;
        let result = self.sync_pending_race(pending.clone()).await;
        self.refresh_race_date(&pending.user_id, &pending.date)
            .await;
        result
    }

    async fn delete_race_impl(&self, user_id: &str, race_id: &str) -> Result<(), RaceError> {
        let existing = self
            .repository
            .find_by_user_id_and_race_id(user_id, race_id)
            .await?
            .ok_or(RaceError::NotFound)?;

        let race_ref = race_entity_ref(&existing.race_id);
        let existing_sync_state = self
            .sync_states
            .find_by_provider_and_canonical_entity(user_id, ExternalProvider::Intervals, &race_ref)
            .await
            .map_err(map_sync_repository_error)?;

        if let Some(sync_state) = existing_sync_state.as_ref() {
            if let Some(linked_intervals_event_id) =
                parse_intervals_event_id(sync_state.external_id.as_deref(), &existing.race_id)?
            {
                let pending_sync_state = self
                    .sync_states
                    .upsert(sync_state.clone().mark_pending_delete())
                    .await
                    .map_err(map_sync_repository_error)?;
                let delete_result = self
                    .intervals
                    .delete_event(user_id, linked_intervals_event_id)
                    .await
                    .map_err(map_intervals_error);
                if let Err(error) = delete_result {
                    self.sync_states
                        .upsert(pending_sync_state.mark_failed(error.to_string()))
                        .await
                        .map_err(map_sync_repository_error)?;
                    return Err(error);
                }
            }
        }

        if let Err(error) = self.repository.delete(user_id, race_id).await {
            self.refresh_race_date(&existing.user_id, &existing.date)
                .await;
            return Err(error);
        }

        if existing_sync_state.is_some() {
            if let Err(error) = self
                .sync_states
                .delete_by_provider_and_canonical_entity(
                    user_id,
                    ExternalProvider::Intervals,
                    &race_ref,
                )
                .await
            {
                warn!(
                    user_id = %existing.user_id,
                    race_id = %existing.race_id,
                    %error,
                    "race delete succeeded locally but failed to delete sync state"
                );
            }
        }

        self.refresh_race_date(&existing.user_id, &existing.date)
            .await;

        Ok(())
    }

    async fn sync_pending_race(&self, pending: Race) -> Result<Race, RaceError> {
        let race_ref = race_entity_ref(&pending.race_id);
        let existing_sync_state = self
            .sync_states
            .find_by_provider_and_canonical_entity(
                &pending.user_id,
                ExternalProvider::Intervals,
                &race_ref,
            )
            .await
            .map_err(map_sync_repository_error)?
            .unwrap_or_else(|| {
                ExternalSyncState::new(
                    pending.user_id.clone(),
                    ExternalProvider::Intervals,
                    race_ref.clone(),
                )
            });
        let pending_sync_state = self
            .sync_states
            .upsert(existing_sync_state.mark_pending_push())
            .await
            .map_err(map_sync_repository_error)?;
        let sync_result: Result<i64, RaceError> = async {
            let remote_event = if let Some(event_id) = parse_intervals_event_id(
                pending_sync_state.external_id.as_deref(),
                &pending.race_id,
            )? {
                self.intervals
                    .update_event(
                        &pending.user_id,
                        event_id,
                        UpdateEvent {
                            category: Some(projected_event_category(&pending)),
                            start_date_local: Some(projected_event_start_date_local(&pending.date)),
                            event_type: Some(projected_event_type(&pending).to_string()),
                            name: Some(projected_event_name(&pending)),
                            description: Some(projected_event_description(&pending)),
                            indoor: Some(false),
                            color: None,
                            workout_doc: None,
                            file_upload: None,
                        },
                    )
                    .await
                    .map_err(map_intervals_error)?
            } else if let Some(existing_event) = self
                .find_existing_remote_race_event(&pending.user_id, &pending)
                .await?
            {
                self.sync_states
                    .upsert(
                        pending_sync_state
                            .clone()
                            .mark_remote_created(existing_event.id.to_string()),
                    )
                    .await
                    .map_err(map_sync_repository_error)?;

                self.intervals
                    .update_event(
                        &pending.user_id,
                        existing_event.id,
                        UpdateEvent {
                            category: Some(projected_event_category(&pending)),
                            start_date_local: Some(projected_event_start_date_local(&pending.date)),
                            event_type: Some(projected_event_type(&pending).to_string()),
                            name: Some(projected_event_name(&pending)),
                            description: Some(projected_event_description(&pending)),
                            indoor: Some(false),
                            color: None,
                            workout_doc: None,
                            file_upload: None,
                        },
                    )
                    .await
                    .map_err(map_intervals_error)?
            } else {
                let remote_event = self
                    .intervals
                    .create_event(
                        &pending.user_id,
                        CreateEvent {
                            category: projected_event_category(&pending),
                            start_date_local: projected_event_start_date_local(&pending.date),
                            event_type: Some(projected_event_type(&pending).to_string()),
                            name: Some(projected_event_name(&pending)),
                            description: Some(projected_event_description(&pending)),
                            indoor: false,
                            color: None,
                            workout_doc: None,
                            file_upload: None,
                        },
                    )
                    .await
                    .map_err(map_intervals_error)?;

                self.sync_states
                    .upsert(
                        pending_sync_state
                            .clone()
                            .mark_remote_created(remote_event.id.to_string()),
                    )
                    .await
                    .map_err(map_sync_repository_error)?;

                remote_event
            };

            Ok(remote_event.id)
        }
        .await;

        match sync_result {
            Ok(remote_event_id) => {
                self.sync_states
                    .upsert(pending_sync_state.mark_synced(
                        remote_event_id.to_string(),
                        pending.payload_hash(),
                        self.clock.now_epoch_seconds(),
                    ))
                    .await
                    .map_err(map_sync_repository_error)?;
                self.mark_calendar_poll_due_soon(&pending.user_id).await;
                Ok(pending)
            }
            Err(error) => {
                self.sync_states
                    .upsert(pending_sync_state.mark_failed(error.to_string()))
                    .await
                    .map_err(map_sync_repository_error)?;
                Err(error)
            }
        }
    }

    async fn find_existing_remote_race_event(
        &self,
        user_id: &str,
        race: &Race,
    ) -> Result<Option<crate::domain::intervals::Event>, RaceError> {
        let date_range = DateRange {
            oldest: race.date.clone(),
            newest: race.date.clone(),
        };
        let canonical_race_marker = canonical_race_id_marker(&race.race_id);

        let events = self
            .intervals
            .list_events(user_id, &date_range)
            .await
            .map_err(map_intervals_error)?;

        Ok(events.into_iter().find(|event| {
            event.description.as_deref().is_some_and(|description| {
                description
                    .lines()
                    .any(|line| line == canonical_race_marker)
            })
        }))
    }
}

impl<Repository, Intervals, SyncStates, Time, Ids, PollStates, Refresh> RaceUseCases
    for RaceService<Repository, Intervals, SyncStates, Time, Ids, PollStates, Refresh>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    SyncStates: ExternalSyncStateRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
    PollStates: ProviderPollStateRepository + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    fn list_races(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Race>, RaceError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_races_impl(&user_id, &range).await })
    }

    fn get_race(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<Race, RaceError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move { service.get_race_impl(&user_id, &race_id).await })
    }

    fn create_race(
        &self,
        user_id: &str,
        request: CreateRace,
    ) -> BoxFuture<Result<Race, RaceError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.create_race_impl(&user_id, request).await })
    }

    fn update_race(
        &self,
        user_id: &str,
        race_id: &str,
        request: UpdateRace,
    ) -> BoxFuture<Result<Race, RaceError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move { service.update_race_impl(&user_id, &race_id, request).await })
    }

    fn delete_race(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<(), RaceError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move { service.delete_race_impl(&user_id, &race_id).await })
    }
}

pub(super) fn validate_request(
    date: &str,
    name: &str,
    distance_meters: i32,
) -> Result<(), RaceError> {
    if date.trim().is_empty() {
        return Err(RaceError::Validation("Race date is required".to_string()));
    }
    if !is_valid_date_format(date) {
        return Err(RaceError::Validation(
            "Race date must be in YYYY-MM-DD format".to_string(),
        ));
    }
    if name.trim().is_empty() {
        return Err(RaceError::Validation("Race name is required".to_string()));
    }
    if distance_meters <= 0 {
        return Err(RaceError::Validation(
            "Race distance must be greater than zero".to_string(),
        ));
    }
    if distance_meters > 10_000_000 {
        return Err(RaceError::Validation(
            "Race distance exceeds maximum allowed value".to_string(),
        ));
    }

    Ok(())
}

/// Returns true when `date` is a valid calendar date in YYYY-MM-DD format.
fn is_valid_date_format(date: &str) -> bool {
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        year.parse::<i32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return false;
    }
    let days_in_month = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    d <= days_in_month
}

fn projected_event_start_date_local(date: &str) -> String {
    format!("{date}T00:00:00")
}

fn projected_event_name(race: &Race) -> String {
    format!("Race {}", race.name)
}

fn projected_event_type(race: &Race) -> &'static str {
    // All cycling disciplines currently map to "Ride" in Intervals.icu.
    // When running disciplines are added this match will need extending.
    match race.discipline {
        super::RaceDiscipline::Road => "Ride",
        super::RaceDiscipline::Mtb => "Ride",
        super::RaceDiscipline::Gravel => "Ride",
        super::RaceDiscipline::Cyclocross => "Ride",
        super::RaceDiscipline::Timetrial => "Ride",
    }
}

fn projected_event_category(race: &Race) -> crate::domain::intervals::EventCategory {
    match race.priority {
        super::RacePriority::A => crate::domain::intervals::EventCategory::RaceA,
        super::RacePriority::B => crate::domain::intervals::EventCategory::RaceB,
        super::RacePriority::C => crate::domain::intervals::EventCategory::RaceC,
    }
}

fn projected_event_description(race: &Race) -> String {
    format!(
        "distance_meters={}\ndiscipline={}\npriority={}\n{}",
        race.distance_meters,
        race.discipline.as_str(),
        race.priority.as_str(),
        canonical_race_id_marker(&race.race_id)
    )
}

fn canonical_race_id_marker(race_id: &str) -> String {
    format!("canonical_race_id={race_id}")
}

fn map_intervals_error(error: IntervalsError) -> RaceError {
    match error {
        IntervalsError::NotFound => RaceError::NotFound,
        IntervalsError::Unauthenticated => RaceError::Unauthenticated,
        IntervalsError::CredentialsNotConfigured => {
            RaceError::Validation("Intervals.icu credentials are not configured".to_string())
        }
        IntervalsError::ApiError(message)
        | IntervalsError::ConnectionError(message)
        | IntervalsError::Internal(message) => RaceError::Unavailable(message),
    }
}

fn map_sync_repository_error(error: ExternalSyncRepositoryError) -> RaceError {
    match error {
        ExternalSyncRepositoryError::Storage(message)
        | ExternalSyncRepositoryError::CorruptData(message) => RaceError::Internal(message),
    }
}

fn race_entity_ref(race_id: &str) -> CanonicalEntityRef {
    CanonicalEntityRef::new(CanonicalEntityKind::Race, race_id.to_string())
}

fn parse_intervals_event_id(
    external_id: Option<&str>,
    race_id: &str,
) -> Result<Option<i64>, RaceError> {
    match external_id {
        None => Ok(None),
        Some(value) => value.parse::<i64>().map(Some).map_err(|_| {
            RaceError::Internal(format!(
                "invalid stored intervals event id for race {race_id}: {value}"
            ))
        }),
    }
}
