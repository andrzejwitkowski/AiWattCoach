use crate::domain::{
    identity::{Clock, IdGenerator},
    intervals::{CreateEvent, DateRange, IntervalsError, IntervalsUseCases, UpdateEvent},
};

use super::{BoxFuture, CreateRace, Race, RaceError, RaceRepository, RaceUseCases, UpdateRace};

#[derive(Clone)]
pub struct RaceService<Repository, Intervals, Time, Ids>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    repository: Repository,
    intervals: Intervals,
    clock: Time,
    ids: Ids,
}

impl<Repository, Intervals, Time, Ids> RaceService<Repository, Intervals, Time, Ids>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
{
    pub fn new(repository: Repository, intervals: Intervals, clock: Time, ids: Ids) -> Self {
        Self {
            repository,
            intervals,
            clock,
            ids,
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
        self.sync_pending_race(pending).await
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
        self.sync_pending_race(pending).await
    }

    async fn delete_race_impl(&self, user_id: &str, race_id: &str) -> Result<(), RaceError> {
        let existing = self
            .repository
            .find_by_user_id_and_race_id(user_id, race_id)
            .await?
            .ok_or(RaceError::NotFound)?;

        if let Some(linked_intervals_event_id) = existing.linked_intervals_event_id {
            let pending = existing.mark_pending_delete(self.clock.now_epoch_seconds());
            let pending = self.repository.upsert(pending).await?;
            let delete_result = self
                .intervals
                .delete_event(user_id, linked_intervals_event_id)
                .await
                .map_err(map_intervals_error);
            if let Err(error) = delete_result {
                let failed = pending.mark_failed(error.to_string(), self.clock.now_epoch_seconds());
                if let Err(persist_error) = self.repository.upsert(failed).await {
                    tracing::error!(
                        user_id = %pending.user_id,
                        race_id = %pending.race_id,
                        error = %persist_error,
                        "failed to persist race delete failure state"
                    );
                }
                return Err(error);
            }
        }

        self.repository.delete(user_id, race_id).await
    }

    async fn sync_pending_race(&self, pending: Race) -> Result<Race, RaceError> {
        let sync_result: Result<i64, RaceError> = async {
            let remote_event = if let Some(event_id) = pending.linked_intervals_event_id {
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
            } else {
                self.intervals
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
                    .map_err(map_intervals_error)?
            };

            Ok(remote_event.id)
        }
        .await;

        match sync_result {
            Ok(remote_event_id) => {
                let synced = pending.mark_synced(
                    remote_event_id,
                    pending.payload_hash(),
                    self.clock.now_epoch_seconds(),
                );
                self.repository.upsert(synced).await
            }
            Err(error) => {
                let failed = pending.mark_failed(error.to_string(), self.clock.now_epoch_seconds());
                if let Err(persist_error) = self.repository.upsert(failed).await {
                    tracing::error!(
                        user_id = %pending.user_id,
                        race_id = %pending.race_id,
                        error = %persist_error,
                        "failed to persist race sync failure state"
                    );
                }
                Err(error)
            }
        }
    }
}

impl<Repository, Intervals, Time, Ids> RaceUseCases
    for RaceService<Repository, Intervals, Time, Ids>
where
    Repository: RaceRepository + Clone + 'static,
    Intervals: IntervalsUseCases + Clone + 'static,
    Time: Clock + Clone + 'static,
    Ids: IdGenerator + Clone + 'static,
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
        "distance_meters={}\ndiscipline={}\npriority={}",
        race.distance_meters,
        race.discipline.as_str(),
        race.priority.as_str()
    )
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
