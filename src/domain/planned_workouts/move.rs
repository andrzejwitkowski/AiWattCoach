use std::sync::Arc;

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    completed_workouts::CompletedWorkoutRepository,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
    },
    identity::Clock,
    intervals::{DateRange, IntervalsError, IntervalsUseCases},
    planned_completed_links::PlannedCompletedWorkoutLinkRepository,
    planned_workouts::{
        BoxFuture, PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository,
        ProviderSyncFailure,
    },
    races::RaceRepository,
    training_plan::TrainingPlanProjectionRepository,
    wahoo::{WahooError, WahooUseCases},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovePlannedWorkoutCommand {
    pub user_id: String,
    pub planned_workout_id: String,
    pub from_date: String,
    pub to_date: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MovePlannedWorkoutOutcome {
    pub planned_workout: PlannedWorkout,
    pub deleted_providers: Vec<ExternalProvider>,
    pub failed_providers: Vec<ProviderSyncFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovePlannedWorkoutError {
    NotFound,
    Conflict(String),
    Validation(String),
    Repository(String),
    Unavailable(String),
}

impl std::fmt::Display for MovePlannedWorkoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "planned workout not found"),
            Self::Conflict(message)
            | Self::Validation(message)
            | Self::Repository(message)
            | Self::Unavailable(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for MovePlannedWorkoutError {}

pub trait PlannedWorkoutMoveUseCases: Send + Sync {
    fn move_planned_workout(
        &self,
        command: MovePlannedWorkoutCommand,
    ) -> BoxFuture<Result<MovePlannedWorkoutOutcome, MovePlannedWorkoutError>>;
}

#[derive(Clone)]
pub struct PlannedWorkoutMoveService<
    Planned,
    SyncStates,
    Intervals,
    Wahoo,
    Completed,
    Links,
    Refresh,
    Time,
> {
    planned_workouts: Planned,
    sync_states: SyncStates,
    intervals: Intervals,
    wahoo: Wahoo,
    projections: Arc<dyn TrainingPlanProjectionRepository>,
    completed: Completed,
    races: Arc<dyn RaceRepository>,
    links: Links,
    refresh: Refresh,
    clock: Time,
}

impl<Planned, SyncStates, Intervals, Wahoo, Completed, Links, Refresh, Time>
    PlannedWorkoutMoveService<
        Planned,
        SyncStates,
        Intervals,
        Wahoo,
        Completed,
        Links,
        Refresh,
        Time,
    >
where
    Planned: PlannedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
    Intervals: IntervalsUseCases + Clone,
    Wahoo: WahooUseCases + Clone,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    Refresh: CalendarEntryViewRefreshPort,
    Time: Clock,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        planned_workouts: Planned,
        sync_states: SyncStates,
        intervals: Intervals,
        wahoo: Wahoo,
        projections: Arc<dyn TrainingPlanProjectionRepository>,
        completed: Completed,
        races: Arc<dyn RaceRepository>,
        links: Links,
        refresh: Refresh,
        clock: Time,
    ) -> Self {
        Self {
            planned_workouts,
            sync_states,
            intervals,
            wahoo,
            projections,
            completed,
            races,
            links,
            refresh,
            clock,
        }
    }

    pub async fn move_planned_workout(
        &self,
        command: MovePlannedWorkoutCommand,
    ) -> Result<MovePlannedWorkoutOutcome, MovePlannedWorkoutError> {
        validate_move_command(&command)?;

        let existing = self
            .load_existing(
                &command.user_id,
                &command.planned_workout_id,
                &command.from_date,
            )
            .await?;
        if existing.rest_day {
            return Err(MovePlannedWorkoutError::Conflict(
                "cannot move a rest day".to_string(),
            ));
        }

        if self
            .links
            .find_by_planned_workout_id(&command.user_id, &command.planned_workout_id)
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?
            .is_some()
        {
            return Err(MovePlannedWorkoutError::Conflict(
                "cannot move a planned workout linked to a completed workout".to_string(),
            ));
        }

        self.assert_target_allowed(&command).await?;

        let now = self.clock.now_epoch_seconds();

        // Materialize projected-only sources so move owns one imported row.
        self.planned_workouts
            .upsert(existing.clone())
            .await
            .map_err(map_planned_workout_error)?;

        let destination_id = rekey_planned_workout_id(
            &command.planned_workout_id,
            &command.from_date,
            &command.to_date,
        );
        let moved = PlannedWorkout {
            planned_workout_id: destination_id.clone(),
            date: command.to_date.clone(),
            rest_day: false,
            rest_day_reason: None,
            ..existing
        };

        let persisted = self
            .planned_workouts
            .upsert(moved)
            .await
            .map_err(map_planned_workout_error)?;

        if destination_id != command.planned_workout_id {
            self.planned_workouts
                .delete_by_user_id_and_planned_workout_id(
                    &command.user_id,
                    &command.planned_workout_id,
                )
                .await
                .map_err(map_planned_workout_error)?;
        }

        // After local persist: drop source projection + rest/target projections on Y.
        self.projections
            .supersede_active_dates(
                &command.user_id,
                &[command.from_date.clone(), command.to_date.clone()],
                now,
            )
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?;

        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            command.planned_workout_id.clone(),
        );
        let existing_states = self
            .sync_states
            .find_by_canonical_entities(&command.user_id, std::slice::from_ref(&canonical_entity))
            .await
            .map_err(map_sync_state_error)?;

        let mut deleted_providers = Vec::new();
        let mut failed_providers = Vec::new();

        for state in existing_states {
            let provider = state.provider.clone();
            let pending = self
                .sync_states
                .upsert(state.clone().mark_pending_delete())
                .await
                .map_err(map_sync_state_error)?;

            let delete_result = match provider {
                ExternalProvider::Intervals => {
                    self.delete_intervals_remote(&command.user_id, &pending)
                        .await
                }
                ExternalProvider::Wahoo => {
                    self.delete_wahoo_remote(&command.user_id, &pending).await
                }
                ExternalProvider::Strava | ExternalProvider::Other => Ok(()),
            };

            match delete_result {
                Ok(()) => {
                    let _ = self
                        .sync_states
                        .delete_by_provider_and_canonical_entity(
                            &command.user_id,
                            provider.clone(),
                            &canonical_entity,
                        )
                        .await;
                    deleted_providers.push(provider);
                }
                Err(error) => {
                    let error_message = error.to_string();
                    let _ = self
                        .sync_states
                        .upsert(pending.mark_failed(error_message.clone()))
                        .await;
                    failed_providers.push(ProviderSyncFailure {
                        provider,
                        error: error_message,
                    });
                }
            }
        }

        refresh_day(&self.refresh, &command.user_id, &command.from_date).await;
        refresh_day(&self.refresh, &command.user_id, &command.to_date).await;

        Ok(MovePlannedWorkoutOutcome {
            planned_workout: persisted,
            deleted_providers,
            failed_providers,
        })
    }

    async fn load_existing(
        &self,
        user_id: &str,
        planned_workout_id: &str,
        date: &str,
    ) -> Result<PlannedWorkout, MovePlannedWorkoutError> {
        if let Some(workout) = self
            .planned_workouts
            .list_by_user_id_and_date_range(user_id, date, date)
            .await
            .map_err(map_planned_workout_error)?
            .into_iter()
            .find(|workout| workout.planned_workout_id == planned_workout_id)
        {
            return Ok(workout);
        }

        let projected = self
            .projections
            .list_active_by_user_id(user_id)
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?
            .into_iter()
            .find(|day| {
                day.date == date
                    && format!("{}:{}", day.operation_key, day.date) == planned_workout_id
            })
            .ok_or(MovePlannedWorkoutError::NotFound)?;

        map_projected_day_to_planned_workout(projected)
    }

    async fn assert_target_allowed(
        &self,
        command: &MovePlannedWorkoutCommand,
    ) -> Result<(), MovePlannedWorkoutError> {
        let on_target = self
            .planned_workouts
            .list_by_user_id_and_date_range(&command.user_id, &command.to_date, &command.to_date)
            .await
            .map_err(map_planned_workout_error)?;

        if on_target.iter().any(|workout| {
            !workout.rest_day && workout.planned_workout_id != command.planned_workout_id
        }) {
            return Err(MovePlannedWorkoutError::Conflict(
                "target day already has a planned workout".to_string(),
            ));
        }

        let projected_on_target = self
            .projections
            .list_active_by_user_id(&command.user_id)
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?
            .into_iter()
            .filter(|day| day.date == command.to_date)
            .collect::<Vec<_>>();
        if projected_on_target.iter().any(|day| {
            !day.rest_day
                && format!("{}:{}", day.operation_key, day.date) != command.planned_workout_id
        }) {
            return Err(MovePlannedWorkoutError::Conflict(
                "target day already has a planned workout".to_string(),
            ));
        }

        let completed = self
            .completed
            .list_by_user_id_and_date_range(&command.user_id, &command.to_date, &command.to_date)
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?;
        if !completed.is_empty() {
            return Err(MovePlannedWorkoutError::Conflict(
                "target day already has a completed workout".to_string(),
            ));
        }

        let races = self
            .races
            .list_by_user_id_and_range(
                &command.user_id,
                &DateRange {
                    oldest: command.to_date.clone(),
                    newest: command.to_date.clone(),
                },
            )
            .await
            .map_err(|error| MovePlannedWorkoutError::Repository(error.to_string()))?;
        if !races.is_empty() {
            return Err(MovePlannedWorkoutError::Conflict(
                "cannot move onto a race day".to_string(),
            ));
        }

        for workout in on_target.into_iter().filter(|workout| {
            workout.rest_day && workout.planned_workout_id != command.planned_workout_id
        }) {
            self.planned_workouts
                .delete_by_user_id_and_planned_workout_id(
                    &command.user_id,
                    &workout.planned_workout_id,
                )
                .await
                .map_err(map_planned_workout_error)?;
        }

        Ok(())
    }

    async fn delete_intervals_remote(
        &self,
        user_id: &str,
        state: &crate::domain::external_sync::ExternalSyncState,
    ) -> Result<(), MovePlannedWorkoutError> {
        let Some(external_id) = state.external_id.as_deref() else {
            return Ok(());
        };
        let event_id = external_id.parse::<i64>().map_err(|_| {
            MovePlannedWorkoutError::Repository(format!(
                "invalid intervals external id '{external_id}'"
            ))
        })?;
        match self.intervals.delete_event(user_id, event_id).await {
            Ok(()) | Err(IntervalsError::NotFound) => Ok(()),
            Err(error) => Err(map_intervals_error(error)),
        }
    }

    async fn delete_wahoo_remote(
        &self,
        user_id: &str,
        state: &crate::domain::external_sync::ExternalSyncState,
    ) -> Result<(), MovePlannedWorkoutError> {
        if let Some(workout_id) = state.wahoo_workout_id {
            match self.wahoo.delete_workout(user_id, workout_id).await {
                Ok(()) | Err(WahooError::NotFound) => {}
                Err(error) => return Err(map_wahoo_error(error)),
            }
        }
        if let Some(plan_id) = state.wahoo_plan_id {
            match self.wahoo.delete_plan(user_id, plan_id).await {
                Ok(()) | Err(WahooError::NotFound) => {}
                Err(error) => return Err(map_wahoo_error(error)),
            }
        }
        Ok(())
    }
}

impl<Planned, SyncStates, Intervals, Wahoo, Completed, Links, Refresh, Time>
    PlannedWorkoutMoveUseCases
    for PlannedWorkoutMoveService<
        Planned,
        SyncStates,
        Intervals,
        Wahoo,
        Completed,
        Links,
        Refresh,
        Time,
    >
where
    Planned: PlannedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
    Intervals: IntervalsUseCases + Clone + 'static,
    Wahoo: WahooUseCases + Clone + 'static,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    Refresh: CalendarEntryViewRefreshPort,
    Time: Clock,
{
    fn move_planned_workout(
        &self,
        command: MovePlannedWorkoutCommand,
    ) -> BoxFuture<Result<MovePlannedWorkoutOutcome, MovePlannedWorkoutError>> {
        let service = self.clone();
        Box::pin(async move { service.move_planned_workout(command).await })
    }
}

fn validate_move_command(
    command: &MovePlannedWorkoutCommand,
) -> Result<(), MovePlannedWorkoutError> {
    for (label, value) in [
        ("fromDate", command.from_date.as_str()),
        ("toDate", command.to_date.as_str()),
    ] {
        if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
            return Err(MovePlannedWorkoutError::Validation(format!(
                "{label} must be in YYYY-MM-DD format"
            )));
        }
    }
    if command.from_date == command.to_date {
        return Err(MovePlannedWorkoutError::Validation(
            "fromDate and toDate must differ".to_string(),
        ));
    }
    if command.planned_workout_id.trim().is_empty() {
        return Err(MovePlannedWorkoutError::Validation(
            "plannedWorkoutId is required".to_string(),
        ));
    }
    Ok(())
}

fn rekey_planned_workout_id(planned_workout_id: &str, from_date: &str, to_date: &str) -> String {
    // ponytail: keep sync UI parseable (operationKey:date) after move
    match planned_workout_id.rsplit_once(':') {
        Some((operation_key, suffix)) if suffix == from_date => {
            format!("{operation_key}:{to_date}")
        }
        _ => planned_workout_id.to_string(),
    }
}

fn map_projected_day_to_planned_workout(
    day: crate::domain::training_plan::TrainingPlanProjectedDay,
) -> Result<PlannedWorkout, MovePlannedWorkoutError> {
    let planned_workout_id = format!("{}:{}", day.operation_key, day.date);
    if day.rest_day {
        return Ok(PlannedWorkout::new(
            planned_workout_id,
            day.user_id,
            day.date,
            crate::domain::planned_workouts::PlannedWorkoutContent { lines: Vec::new() },
        )
        .as_rest_day(day.rest_day_reason));
    }

    let intervals_workout = day.workout.ok_or_else(|| {
        MovePlannedWorkoutError::Validation(
            "projected day is missing planned workout payload".to_string(),
        )
    })?;
    let content =
        crate::domain::planned_workouts::update::map_intervals_to_canonical_planned_workout_content(
            &intervals_workout,
        );
    let name = intervals_workout.lines.iter().find_map(|line| match line {
        crate::domain::intervals::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
        _ => None,
    });

    Ok(
        PlannedWorkout::new(planned_workout_id, day.user_id, day.date, content)
            .with_event_metadata(name, None, Some("Ride".to_string())),
    )
}

fn map_planned_workout_error(error: PlannedWorkoutError) -> MovePlannedWorkoutError {
    match error {
        PlannedWorkoutError::Repository(message) => MovePlannedWorkoutError::Repository(message),
    }
}

fn map_sync_state_error(
    error: crate::domain::external_sync::ExternalSyncRepositoryError,
) -> MovePlannedWorkoutError {
    match error {
        crate::domain::external_sync::ExternalSyncRepositoryError::Storage(message)
        | crate::domain::external_sync::ExternalSyncRepositoryError::CorruptData(message) => {
            MovePlannedWorkoutError::Repository(message)
        }
    }
}

fn map_intervals_error(error: IntervalsError) -> MovePlannedWorkoutError {
    match error {
        IntervalsError::Unauthenticated | IntervalsError::CredentialsNotConfigured => {
            MovePlannedWorkoutError::Unavailable(error.to_string())
        }
        IntervalsError::NotFound => MovePlannedWorkoutError::NotFound,
        IntervalsError::ApiError(message)
        | IntervalsError::ConnectionError(message)
        | IntervalsError::Internal(message) => MovePlannedWorkoutError::Repository(message),
    }
}

fn map_wahoo_error(error: WahooError) -> MovePlannedWorkoutError {
    match error {
        WahooError::NotConnected | WahooError::Unauthenticated => {
            MovePlannedWorkoutError::Unavailable(error.to_string())
        }
        WahooError::NotFound => MovePlannedWorkoutError::NotFound,
        other => MovePlannedWorkoutError::Repository(other.to_string()),
    }
}

async fn refresh_day<Refresh>(refresh: &Refresh, user_id: &str, date: &str)
where
    Refresh: CalendarEntryViewRefreshPort,
{
    if let Err(error) = refresh.refresh_range_for_user(user_id, date, date).await {
        tracing::warn!(%user_id, %date, %error, "planned workout move succeeded but calendar view refresh failed");
    }
}

#[cfg(test)]
#[path = "move_tests.rs"]
mod tests;
