mod syncable;

#[cfg(test)]
use syncable::preserve_event_description;
use syncable::{
    map_intervals_to_canonical_planned_workout_content, map_planned_workout_to_syncable,
    planned_workout_name, SyncablePlannedWorkout,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository,
    },
    identity::Clock,
    intervals::{parse_planned_workout, IntervalsError, IntervalsUseCases},
    planned_workout_tokens::PlannedWorkoutTokenRepository,
    settings::UserSettingsRepository,
    wahoo::{
        WahooError, WahooUpdatePlan, WahooUpdateWorkout, WahooUseCases, MISSING_WAHOO_FTP_MESSAGE,
    },
};

use super::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePlannedWorkoutError {
    NotFound,
    Validation(String),
    Repository(String),
    Unavailable(String),
}

impl std::fmt::Display for UpdatePlannedWorkoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "planned workout not found"),
            Self::Validation(message) | Self::Repository(message) | Self::Unavailable(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl std::error::Error for UpdatePlannedWorkoutError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePlannedWorkoutCommand {
    pub user_id: String,
    pub planned_workout_id: String,
    pub date: String,
    pub workout_doc: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdatePlannedWorkoutOutcome {
    pub planned_workout: PlannedWorkout,
    pub synced_providers: Vec<ExternalProvider>,
    pub failed_providers: Vec<ProviderSyncFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderSyncFailure {
    pub provider: ExternalProvider,
    pub error: String,
}

#[derive(Clone)]
pub struct PlannedWorkoutUpdateService<
    Planned,
    SyncStates,
    Intervals,
    Wahoo,
    Settings,
    Tokens,
    Refresh,
    Time,
> where
    Planned: PlannedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
    Intervals: IntervalsUseCases + Clone,
    Wahoo: WahooUseCases + Clone,
    Settings: UserSettingsRepository,
    Tokens: PlannedWorkoutTokenRepository,
    Refresh: CalendarEntryViewRefreshPort,
    Time: Clock,
{
    planned_workouts: Planned,
    sync_states: SyncStates,
    intervals: Intervals,
    wahoo: Wahoo,
    settings: Settings,
    planned_workout_tokens: Tokens,
    refresh: Refresh,
    clock: Time,
}

impl<Planned, SyncStates, Intervals, Wahoo, Settings, Tokens, Refresh, Time>
    PlannedWorkoutUpdateService<
        Planned,
        SyncStates,
        Intervals,
        Wahoo,
        Settings,
        Tokens,
        Refresh,
        Time,
    >
where
    Planned: PlannedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
    Intervals: IntervalsUseCases + Clone,
    Wahoo: WahooUseCases + Clone,
    Settings: UserSettingsRepository,
    Tokens: PlannedWorkoutTokenRepository,
    Refresh: CalendarEntryViewRefreshPort,
    Time: Clock,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        planned_workouts: Planned,
        sync_states: SyncStates,
        intervals: Intervals,
        wahoo: Wahoo,
        settings: Settings,
        planned_workout_tokens: Tokens,
        refresh: Refresh,
        clock: Time,
    ) -> Self {
        Self {
            planned_workouts,
            sync_states,
            intervals,
            wahoo,
            settings,
            planned_workout_tokens,
            refresh,
            clock,
        }
    }

    pub async fn update_planned_workout(
        &self,
        command: UpdatePlannedWorkoutCommand,
    ) -> Result<UpdatePlannedWorkoutOutcome, UpdatePlannedWorkoutError> {
        validate_update_command(&command)?;

        let existing = self
            .load_existing_workout(&command.user_id, &command.planned_workout_id, &command.date)
            .await?;
        let parsed = parse_planned_workout(command.workout_doc.trim()).map_err(|error| {
            UpdatePlannedWorkoutError::Validation(format!("invalid workoutDoc: {error}"))
        })?;
        let updated_workout = PlannedWorkout {
            rest_day: false,
            rest_day_reason: None,
            workout: map_intervals_to_canonical_planned_workout_content(&parsed),
            name: planned_workout_name(&parsed),
            ..existing
        };
        let syncable = map_planned_workout_to_syncable(&updated_workout)?;
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            updated_workout.planned_workout_id.clone(),
        );

        let persisted = self
            .planned_workouts
            .upsert(updated_workout)
            .await
            .map_err(map_planned_workout_error)?;

        let existing_states = self
            .sync_states
            .find_by_canonical_entities(&command.user_id, std::slice::from_ref(&canonical_entity))
            .await
            .map_err(map_sync_state_error)?;

        let mut synced_providers = Vec::new();
        let mut failed_providers = Vec::new();

        for state in existing_states {
            let provider = state.provider.clone();
            let modified_state = self
                .sync_states
                .upsert(state.mark_modified())
                .await
                .map_err(map_sync_state_error)?;

            let sync_result = match provider {
                ExternalProvider::Intervals => {
                    self.sync_to_intervals(&command.user_id, &syncable, modified_state.clone())
                        .await
                }
                ExternalProvider::Wahoo => {
                    self.sync_to_wahoo(&command.user_id, &syncable, modified_state.clone())
                        .await
                }
                ExternalProvider::Strava | ExternalProvider::Other => Ok(None),
            };

            match sync_result {
                Ok(Some(synced_state)) => {
                    self.sync_states
                        .upsert(synced_state)
                        .await
                        .map_err(map_sync_state_error)?;
                    synced_providers.push(provider);
                }
                Ok(None) => {}
                Err(error) => {
                    let error_message = error.to_string();
                    self.sync_states
                        .upsert(modified_state.mark_failed(error_message.clone()))
                        .await
                        .map_err(map_sync_state_error)?;
                    failed_providers.push(ProviderSyncFailure {
                        provider,
                        error: error_message,
                    });
                }
            }
        }

        refresh_planned_workout_day(&self.refresh, &command.user_id, &command.date).await;

        Ok(UpdatePlannedWorkoutOutcome {
            planned_workout: persisted,
            synced_providers,
            failed_providers,
        })
    }

    async fn load_existing_workout(
        &self,
        user_id: &str,
        planned_workout_id: &str,
        date: &str,
    ) -> Result<PlannedWorkout, UpdatePlannedWorkoutError> {
        let workouts = self
            .planned_workouts
            .list_by_user_id_and_date_range(user_id, date, date)
            .await
            .map_err(map_planned_workout_error)?;

        workouts
            .into_iter()
            .find(|workout| workout.planned_workout_id == planned_workout_id)
            .ok_or(UpdatePlannedWorkoutError::NotFound)
    }

    async fn sync_to_intervals(
        &self,
        user_id: &str,
        planned_workout: &SyncablePlannedWorkout,
        state: ExternalSyncState,
    ) -> Result<Option<ExternalSyncState>, UpdatePlannedWorkoutError> {
        let Some(external_id) = state.external_id.as_deref() else {
            return Ok(None);
        };
        let event_id = external_id.parse::<i64>().map_err(|_| {
            UpdatePlannedWorkoutError::Repository(format!(
                "invalid intervals external id for planned workout {}",
                planned_workout.planned_workout_id
            ))
        })?;
        let existing_event = self
            .intervals
            .get_event(user_id, event_id)
            .await
            .map_err(map_intervals_error)?;
        let updated = self
            .intervals
            .update_event(
                user_id,
                event_id,
                planned_workout.build_intervals_update(&existing_event),
            )
            .await
            .map_err(map_intervals_error)?;

        Ok(Some(state.mark_synced(
            updated.id.to_string(),
            planned_workout.payload_hash(),
            self.clock.now_epoch_seconds(),
        )))
    }

    async fn sync_to_wahoo(
        &self,
        user_id: &str,
        planned_workout: &SyncablePlannedWorkout,
        state: ExternalSyncState,
    ) -> Result<Option<ExternalSyncState>, UpdatePlannedWorkoutError> {
        let Some(wahoo_plan_id) = state.wahoo_plan_id else {
            return Ok(None);
        };
        let Some(wahoo_workout_id) = state.wahoo_workout_id else {
            return Ok(None);
        };
        let Some(wahoo_plan_external_id) = state.wahoo_plan_external_id.clone() else {
            return Ok(None);
        };

        let settings = self
            .settings
            .find_by_user_id(user_id)
            .await
            .map_err(map_settings_error)?
            .ok_or_else(|| {
                UpdatePlannedWorkoutError::Validation(MISSING_WAHOO_FTP_MESSAGE.to_string())
            })?;
        let ftp_watts = settings.cycling.ftp_watts.ok_or_else(|| {
            UpdatePlannedWorkoutError::Validation(MISSING_WAHOO_FTP_MESSAGE.to_string())
        })?;
        let workout_token = match state.wahoo_workout_token.clone() {
            Some(token) => token,
            None => {
                ensure_planned_workout_marker(
                    &self.planned_workout_tokens,
                    user_id,
                    &planned_workout.planned_workout_id,
                )
                .await?
            }
        };
        let provider_updated_at = provider_updated_at(self.clock.now_epoch_seconds());
        let plan_file_json = crate::domain::wahoo::build_plan_file_json(
            &planned_workout.to_projected_day(user_id, self.clock.now_epoch_seconds()),
            ftp_watts,
        )
        .map_err(UpdatePlannedWorkoutError::Validation)?;
        let plan_file_base64 = BASE64_STANDARD.encode(plan_file_json.as_bytes());
        let plan = self
            .wahoo
            .update_plan(
                user_id,
                wahoo_plan_id,
                WahooUpdatePlan {
                    file_base64: plan_file_base64,
                    filename: Some(plan_filename(&planned_workout.planned_workout_id)),
                    provider_updated_at: provider_updated_at.clone(),
                },
            )
            .await
            .map_err(map_wahoo_error)?;
        let workout = self
            .wahoo
            .update_workout(
                user_id,
                wahoo_workout_id,
                WahooUpdateWorkout {
                    name: planned_workout.name.clone(),
                    workout_token: Some(workout_token.clone()),
                    // Planned-workout syncs intentionally force the Wahoo workout type to
                    // Biking (0) on both create and update paths. Manual type changes in
                    // Wahoo are overwritten on the next sync.
                    workout_type_id: Some(0),
                    starts: Some(projected_workout_start_at(&planned_workout.date)),
                    minutes: Some(planned_workout.minutes()?),
                    plan_id: Some(plan.id),
                },
            )
            .await
            .map_err(map_wahoo_error)?;

        Ok(Some(state.mark_wahoo_synced(
            planned_workout.payload_hash(),
            self.clock.now_epoch_seconds(),
            wahoo_plan_external_id,
            plan.id,
            workout.id,
            workout_token,
        )))
    }
}

fn validate_update_command(
    command: &UpdatePlannedWorkoutCommand,
) -> Result<(), UpdatePlannedWorkoutError> {
    if chrono::NaiveDate::parse_from_str(&command.date, "%Y-%m-%d").is_err() {
        return Err(UpdatePlannedWorkoutError::Validation(
            "planned workout date must be in YYYY-MM-DD format".to_string(),
        ));
    }
    if command.planned_workout_id.trim().is_empty() {
        return Err(UpdatePlannedWorkoutError::Validation(
            "plannedWorkoutId is required".to_string(),
        ));
    }
    if command.workout_doc.trim().is_empty() {
        return Err(UpdatePlannedWorkoutError::Validation(
            "workoutDoc must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn map_planned_workout_error(error: PlannedWorkoutError) -> UpdatePlannedWorkoutError {
    match error {
        PlannedWorkoutError::Repository(message) => UpdatePlannedWorkoutError::Repository(message),
    }
}

fn map_sync_state_error(
    error: crate::domain::external_sync::ExternalSyncRepositoryError,
) -> UpdatePlannedWorkoutError {
    match error {
        crate::domain::external_sync::ExternalSyncRepositoryError::Storage(message)
        | crate::domain::external_sync::ExternalSyncRepositoryError::CorruptData(message) => {
            UpdatePlannedWorkoutError::Repository(message)
        }
    }
}

fn map_settings_error(error: crate::domain::settings::SettingsError) -> UpdatePlannedWorkoutError {
    UpdatePlannedWorkoutError::Repository(error.to_string())
}

fn map_intervals_error(error: IntervalsError) -> UpdatePlannedWorkoutError {
    match error {
        IntervalsError::CredentialsNotConfigured => UpdatePlannedWorkoutError::Validation(
            "Intervals.icu credentials are not configured".to_string(),
        ),
        IntervalsError::Unauthenticated => UpdatePlannedWorkoutError::Unavailable(
            "Intervals.icu authentication failed".to_string(),
        ),
        IntervalsError::NotFound => UpdatePlannedWorkoutError::Unavailable(
            "Intervals.icu event no longer exists".to_string(),
        ),
        IntervalsError::ApiError(message)
        | IntervalsError::ConnectionError(message)
        | IntervalsError::Internal(message) => UpdatePlannedWorkoutError::Unavailable(message),
    }
}

fn map_wahoo_error(error: WahooError) -> UpdatePlannedWorkoutError {
    match error {
        WahooError::Unauthenticated => {
            UpdatePlannedWorkoutError::Unavailable("Wahoo authentication is required".to_string())
        }
        WahooError::InvalidConnectState | WahooError::NotConnected => {
            UpdatePlannedWorkoutError::Validation("Wahoo account is not connected".to_string())
        }
        WahooError::NotFound => UpdatePlannedWorkoutError::Unavailable(
            "Wahoo planned workout no longer exists".to_string(),
        ),
        WahooError::Repository(message) | WahooError::External(message) => {
            UpdatePlannedWorkoutError::Unavailable(message)
        }
    }
}

async fn ensure_planned_workout_marker<Tokens>(
    tokens: &Tokens,
    user_id: &str,
    planned_workout_id: &str,
) -> Result<String, UpdatePlannedWorkoutError>
where
    Tokens: PlannedWorkoutTokenRepository,
{
    let match_token = match tokens
        .find_by_planned_workout_id(user_id, planned_workout_id)
        .await
        .map_err(|error| UpdatePlannedWorkoutError::Repository(error.to_string()))?
    {
        Some(token) => token.match_token,
        None => {
            let match_token =
                crate::domain::planned_workout_tokens::build_planned_workout_match_token(
                    planned_workout_id,
                );
            tokens
                .upsert(
                    crate::domain::planned_workout_tokens::PlannedWorkoutToken::new(
                        user_id.to_string(),
                        planned_workout_id.to_string(),
                        match_token.clone(),
                    ),
                )
                .await
                .map_err(|error| UpdatePlannedWorkoutError::Repository(error.to_string()))?;
            match_token
        }
    };

    Ok(crate::domain::planned_workout_tokens::format_planned_workout_marker(&match_token))
}

async fn refresh_planned_workout_day<Refresh>(refresh: &Refresh, user_id: &str, date: &str)
where
    Refresh: CalendarEntryViewRefreshPort,
{
    if let Err(error) = refresh.refresh_range_for_user(user_id, date, date).await {
        tracing::warn!(%user_id, %date, %error, "planned workout update succeeded but calendar view refresh failed");
    }
}

fn projected_workout_start_at(date: &str) -> String {
    format!("{date}T00:00:00.000Z")
}

fn provider_updated_at(now_epoch_seconds: i64) -> String {
    chrono::DateTime::from_timestamp(now_epoch_seconds, 0)
        .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH)
        .to_rfc3339()
}

fn plan_filename(planned_workout_id: &str) -> String {
    format!("{planned_workout_id}.plan.json")
}

#[cfg(test)]
mod tests;
