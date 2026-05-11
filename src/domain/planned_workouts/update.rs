use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use sha2::Digest;

use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository,
    },
    identity::Clock,
    intervals::{
        parse_planned_workout, Event, EventCategory, IntervalsError, IntervalsUseCases, UpdateEvent,
    },
    planned_workout_tokens::PlannedWorkoutTokenRepository,
    settings::UserSettingsRepository,
    wahoo::{WahooError, WahooUpdatePlan, WahooUpdateWorkout, WahooUseCases},
};

use super::{PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository};

const MISSING_WAHOO_FTP_MESSAGE: &str = "Set your cycling FTP in Settings before syncing to Wahoo";

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
}

#[derive(Clone, Debug, PartialEq)]
struct SyncablePlannedWorkout {
    planned_workout_id: String,
    date: String,
    rest_day: bool,
    rest_day_reason: Option<String>,
    name: Option<String>,
    workout: crate::domain::intervals::PlannedWorkout,
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
            workout: map_intervals_to_canonical_planned_workout_content(&parsed),
            ..existing
        };
        let syncable = map_planned_workout_to_syncable(&updated_workout)?;
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            updated_workout.planned_workout_id.clone(),
        );
        let payload_hash = syncable.payload_hash();

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

        for state in existing_states {
            let provider = state.provider.clone();
            let modified_state = self
                .sync_states
                .upsert(state.mark_modified(payload_hash.clone()))
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
                    self.sync_states
                        .upsert(modified_state.mark_failed(error.to_string()))
                        .await
                        .map_err(map_sync_state_error)?;
                }
            }
        }

        refresh_planned_workout_day(&self.refresh, &command.user_id, &command.date).await;

        Ok(UpdatePlannedWorkoutOutcome {
            planned_workout: persisted,
            synced_providers,
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
        // TODO(arch): domain→adapter violation – build_plan_file_json lives in
        // crate::adapters::wahoo::plan_mapping. cargo_pup only checks `use` declarations,
        // not fully-qualified path expressions, so verify:arch does not catch this.
        // Fix in a follow-up PR by introducing a domain-level port or moving the pure
        // mapping function to the domain layer.
        let plan_file_json = crate::adapters::wahoo::plan_mapping::build_plan_file_json(
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

fn map_intervals_to_canonical_planned_workout_content(
    workout: &crate::domain::intervals::PlannedWorkout,
) -> super::PlannedWorkoutContent {
    super::PlannedWorkoutContent {
        lines: workout
            .lines
            .iter()
            .cloned()
            .map(|line| match line {
                crate::domain::intervals::PlannedWorkoutLine::BlankLine => {
                    super::PlannedWorkoutLine::BlankLine
                }
                crate::domain::intervals::PlannedWorkoutLine::Text(text) => {
                    super::PlannedWorkoutLine::Text(super::PlannedWorkoutText { text: text.text })
                }
                crate::domain::intervals::PlannedWorkoutLine::Repeat(repeat) => {
                    super::PlannedWorkoutLine::Repeat(super::PlannedWorkoutRepeat {
                        title: repeat.title,
                        count: repeat.count,
                    })
                }
                crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
                    super::PlannedWorkoutLine::Step(super::PlannedWorkoutStep {
                        duration_seconds: step.duration_seconds,
                        kind: match step.kind {
                            crate::domain::intervals::PlannedWorkoutStepKind::Steady => {
                                super::PlannedWorkoutStepKind::Steady
                            }
                            crate::domain::intervals::PlannedWorkoutStepKind::Ramp => {
                                super::PlannedWorkoutStepKind::Ramp
                            }
                        },
                        target: match step.target {
                            crate::domain::intervals::PlannedWorkoutTarget::PercentFtp {
                                min,
                                max,
                            } => super::PlannedWorkoutTarget::PercentFtp { min, max },
                            crate::domain::intervals::PlannedWorkoutTarget::WattsRange {
                                min,
                                max,
                            } => super::PlannedWorkoutTarget::WattsRange { min, max },
                        },
                    })
                }
            })
            .collect(),
    }
}

fn map_planned_workout_to_syncable(
    workout: &PlannedWorkout,
) -> Result<SyncablePlannedWorkout, UpdatePlannedWorkoutError> {
    let serialized = crate::domain::planned_workouts::serialize_canonical_planned_workout(workout);
    let parsed = parse_planned_workout(&serialized).map_err(|error| {
        UpdatePlannedWorkoutError::Validation(format!("invalid planned workout: {error}"))
    })?;
    let name = workout
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            parsed.lines.iter().find_map(|line| match line {
                crate::domain::intervals::PlannedWorkoutLine::Text(text) => Some(text.text.clone()),
                _ => None,
            })
        });
    Ok(SyncablePlannedWorkout {
        planned_workout_id: workout.planned_workout_id.clone(),
        date: workout.date.clone(),
        rest_day: workout.rest_day,
        rest_day_reason: workout.rest_day_reason.clone(),
        name,
        workout: parsed,
    })
}

impl SyncablePlannedWorkout {
    fn payload_hash(&self) -> String {
        let workout_text = if self.rest_day {
            None
        } else {
            Some(crate::domain::intervals::serialize_planned_workout_for_intervals(&self.workout))
        };
        let digest = sha2::Sha256::digest(format!(
            "{}\n{}\n{}",
            self.date,
            self.name.as_deref().unwrap_or_default(),
            workout_text.as_deref().unwrap_or_default(),
        ));
        format!("{digest:x}")
    }

    fn build_intervals_update(&self, existing_event: &Event) -> UpdateEvent {
        UpdateEvent {
            category: Some(EventCategory::Workout),
            start_date_local: Some(format!("{}T00:00:00", self.date)),
            event_type: existing_event
                .event_type
                .clone()
                .or_else(|| Some("Ride".to_string())),
            name: self.name.clone(),
            description: preserve_event_description(
                existing_event.description.as_deref(),
                self.sync_body().as_deref(),
            ),
            indoor: Some(existing_event.indoor),
            color: existing_event.color.clone(),
            workout_doc: None,
            file_upload: None,
        }
    }

    fn sync_body(&self) -> Option<String> {
        if self.rest_day {
            return None;
        }
        let workout_text =
            crate::domain::intervals::serialize_planned_workout_for_intervals(&self.workout);
        comparable_workout_text_for_payload_hash(self.name.as_deref(), Some(workout_text.as_str()))
    }

    fn minutes(&self) -> Result<i32, UpdatePlannedWorkoutError> {
        let total_seconds: i32 = self
            .workout
            .lines
            .iter()
            .filter_map(|line| match line {
                crate::domain::intervals::PlannedWorkoutLine::Step(step) => {
                    Some(step.duration_seconds)
                }
                _ => None,
            })
            .sum();
        if total_seconds <= 0 {
            return Err(UpdatePlannedWorkoutError::Validation(
                "planned workout has no syncable duration".to_string(),
            ));
        }
        Ok((total_seconds + 59) / 60)
    }

    fn to_projected_day(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> crate::domain::training_plan::TrainingPlanProjectedDay {
        crate::domain::training_plan::TrainingPlanProjectedDay {
            user_id: user_id.to_string(),
            workout_id: self.planned_workout_id.clone(),
            operation_key: self
                .planned_workout_id
                .split(':')
                .next()
                .unwrap_or(&self.planned_workout_id)
                .to_string(),
            date: self.date.clone(),
            rest_day: self.rest_day,
            rest_day_reason: self.rest_day_reason.clone(),
            workout: Some(self.workout.clone()),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
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

fn preserve_event_description(existing: Option<&str>, projected: Option<&str>) -> Option<String> {
    match (
        existing.map(str::trim).filter(|value| !value.is_empty()),
        projected,
    ) {
        (None, None) => None,
        (Some(existing), None) => Some(existing.to_string()),
        (None, Some(projected)) => Some(projected.to_string()),
        (Some(existing), Some(projected)) if existing.contains(projected) => {
            Some(existing.to_string())
        }
        (Some(existing), Some(projected)) => Some(format!("{existing}\n\n{projected}")),
    }
}

fn comparable_workout_text_for_payload_hash(
    name: Option<&str>,
    workout_text: Option<&str>,
) -> Option<String> {
    let workout_text = workout_text
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let Some(name) = name.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(workout_text.to_string());
    };

    let mut lines = workout_text.lines();
    let Some(first_line) = lines.next() else {
        return Some(workout_text.to_string());
    };
    if first_line.trim() != name {
        return Some(workout_text.to_string());
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        Some(name.to_string())
    } else {
        Some(body)
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
