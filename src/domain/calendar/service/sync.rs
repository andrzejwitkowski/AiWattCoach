use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::domain::{
    calendar::{CalendarError, CalendarEvent, PlannedWorkoutSyncProvider, SyncPlannedWorkout},
    external_sync::{CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState},
    intervals::{CreateEvent, Event, EventCategory, IntervalsError, UpdateEvent},
    planned_workout_tokens::{build_planned_workout_match_token, PlannedWorkoutToken},
    training_plan::TrainingPlanProjectedDay,
    wahoo::{WahooCreatePlan, WahooCreateWorkout, WahooUpdatePlan, WahooUpdateWorkout},
};

use super::{
    errors::{
        map_external_sync_error, map_intervals_error, map_planned_workout_token_error,
        map_settings_error, map_training_plan_error, map_wahoo_error,
    },
    projected::{
        build_projected_calendar_event, comparable_workout_text_for_payload_hash,
        projected_day_payload_hash, projected_event_payload_hash, projected_event_sync_body,
        projected_workout_id, projected_workout_name,
    },
    CalendarService,
};

const MISSING_WAHOO_FTP_MESSAGE: &str = "Set your cycling FTP in Settings before syncing to Wahoo";
const INVALID_PLANNED_WORKOUT_DATE_MESSAGE: &str =
    "planned workout date must be in YYYY-MM-DD format";
const WAHOO_SYNC_WINDOW_MESSAGE: &str =
    "Only planned workouts scheduled between today and the next 6 days can sync to Wahoo";

impl<
        Intervals,
        Entries,
        Projections,
        SyncStates,
        Time,
        Wahoo,
        Settings,
        Tokens,
        Refresh,
        Completed,
    >
    CalendarService<
        Intervals,
        Entries,
        Projections,
        SyncStates,
        Time,
        Wahoo,
        Settings,
        Tokens,
        Refresh,
        Completed,
    >
where
    Intervals: crate::domain::intervals::IntervalsUseCases + Clone,
    Entries: crate::domain::calendar_view::CalendarEntryViewRepository + Clone,
    Completed: crate::domain::completed_workouts::CompletedWorkoutRepository + Clone,
    Projections: crate::domain::training_plan::TrainingPlanProjectionRepository + Clone,
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Wahoo: crate::domain::wahoo::WahooUseCases + Clone,
    Settings: crate::domain::settings::UserSettingsRepository + Clone,
    Tokens: crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository + Clone,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort + Clone,
{
    pub(super) async fn sync_planned_workout_impl(
        &self,
        user_id: &str,
        request: SyncPlannedWorkout,
    ) -> Result<CalendarEvent, CalendarError> {
        let projected_day = self
            .projections
            .find_active_by_user_id_and_operation_key(user_id, &request.operation_key)
            .await
            .map_err(map_training_plan_error)?
            .into_iter()
            .find(|day| day.date == request.date)
            .ok_or(CalendarError::NotFound)?;

        if projected_day.rest_day || projected_day.workout.is_none() {
            return Err(CalendarError::Validation(
                "Only planned workout days can be synchronized".to_string(),
            ));
        }

        let planned_workout_id = projected_workout_id(&request.operation_key, &request.date);
        let canonical_entity = CanonicalEntityRef::new(
            CanonicalEntityKind::PlannedWorkout,
            planned_workout_id.clone(),
        );

        let synced_event = match request.provider {
            PlannedWorkoutSyncProvider::Intervals => {
                self.sync_planned_workout_to_intervals(
                    user_id,
                    &request,
                    projected_day,
                    canonical_entity,
                )
                .await?
            }
            PlannedWorkoutSyncProvider::Wahoo => {
                ensure_sync_window(&self.clock, &request.date)?;
                self.sync_planned_workout_to_wahoo(
                    user_id,
                    &request,
                    projected_day,
                    canonical_entity,
                    &planned_workout_id,
                )
                .await?
            }
        };

        refresh_planned_workout_day(&self.refresh, user_id, &request).await;

        Ok(synced_event)
    }

    async fn sync_planned_workout_to_intervals(
        &self,
        user_id: &str,
        request: &SyncPlannedWorkout,
        projected_day: TrainingPlanProjectedDay,
        canonical_entity: CanonicalEntityRef,
    ) -> Result<CalendarEvent, CalendarError> {
        let payload_hash = projected_day_payload_hash(&projected_day);
        let existing_state = self
            .sync_states
            .find_by_provider_and_canonical_entity(
                user_id,
                ExternalProvider::Intervals,
                &canonical_entity,
            )
            .await
            .map_err(map_external_sync_error)?
            .unwrap_or_else(|| {
                ExternalSyncState::new(
                    user_id.to_string(),
                    ExternalProvider::Intervals,
                    canonical_entity.clone(),
                )
            });

        let pending_state = self
            .sync_states
            .upsert(existing_state.mark_pending_push())
            .await
            .map_err(map_external_sync_error)?;

        let sync_result = async {
            let existing_remote_event = if let Some(intervals_event_id) =
                intervals_event_id(&pending_state)
            {
                match self.intervals.get_event(user_id, intervals_event_id).await {
                    Ok(event) => Some(event),
                    Err(IntervalsError::NotFound) => None,
                    Err(error) => return Err(map_intervals_error(error)),
                }
            } else {
                find_existing_remote_event(&self.intervals, user_id, &projected_day, &payload_hash)
                    .await?
            };

            let remote_event = if let Some(existing_remote_event) = existing_remote_event {
                self.intervals
                    .update_event(
                        user_id,
                        existing_remote_event.id,
                        build_update_event(&projected_day, &existing_remote_event),
                    )
                    .await
                    .map_err(map_intervals_error)?
            } else {
                self.intervals
                    .create_event(user_id, build_create_event(&projected_day))
                    .await
                    .map_err(map_intervals_error)?
            };

            Ok(remote_event)
        }
        .await;

        match sync_result {
            Ok(remote_event) => {
                let synced_state = self
                    .sync_states
                    .upsert(pending_state.mark_synced(
                        remote_event.id.to_string(),
                        payload_hash,
                        self.clock.now_epoch_seconds(),
                    ))
                    .await
                    .map_err(map_external_sync_error)?;
                let all_states = self
                    .planned_workout_sync_states(user_id, &canonical_entity)
                    .await?;
                Ok(build_projected_calendar_event(
                    projected_day,
                    &all_states,
                    Some(synced_state),
                ))
            }
            Err(error) => {
                persist_failed_sync_state(
                    &self.sync_states,
                    pending_state.mark_failed(error.to_string()),
                    user_id,
                    request,
                )
                .await;
                refresh_planned_workout_day(&self.refresh, user_id, request).await;
                Err(error)
            }
        }
    }

    async fn sync_planned_workout_to_wahoo(
        &self,
        user_id: &str,
        request: &SyncPlannedWorkout,
        projected_day: TrainingPlanProjectedDay,
        canonical_entity: CanonicalEntityRef,
        planned_workout_id: &str,
    ) -> Result<CalendarEvent, CalendarError> {
        let payload_hash = projected_day_payload_hash(&projected_day);
        let now = self.clock.now_epoch_seconds();
        let existing_state = self
            .sync_states
            .find_by_provider_and_canonical_entity(
                user_id,
                ExternalProvider::Wahoo,
                &canonical_entity,
            )
            .await
            .map_err(map_external_sync_error)?
            .unwrap_or_else(|| {
                ExternalSyncState::new(
                    user_id.to_string(),
                    ExternalProvider::Wahoo,
                    canonical_entity.clone(),
                )
            });
        let pending_state = self
            .sync_states
            .upsert(existing_state.mark_wahoo_pending(planned_workout_id.to_string()))
            .await
            .map_err(map_external_sync_error)?;

        let sync_result: Result<
            (
                crate::domain::wahoo::WahooPlan,
                crate::domain::wahoo::WahooWorkout,
                String,
            ),
            CalendarError,
        > = async {
            let settings = self
                .settings
                .find_by_user_id(user_id)
                .await
                .map_err(map_settings_error)?
                .ok_or_else(|| CalendarError::Validation(MISSING_WAHOO_FTP_MESSAGE.to_string()))?;
            let ftp_watts = settings
                .cycling
                .ftp_watts
                .ok_or_else(|| CalendarError::Validation(MISSING_WAHOO_FTP_MESSAGE.to_string()))?;
            let planned_workout_marker = ensure_planned_workout_marker(
                &self.planned_workout_tokens,
                user_id,
                planned_workout_id,
            )
            .await?;
            let workout_token = pending_state
                .wahoo_workout_token
                .clone()
                .unwrap_or_else(|| planned_workout_marker.clone());
            let plan_file_json = crate::adapters::wahoo::plan_mapping::build_plan_file_json(
                &projected_day,
                ftp_watts,
            )
            .map_err(CalendarError::Validation)?;
            let plan_file_base64 = BASE64_STANDARD.encode(plan_file_json.as_bytes());
            let provider_updated_at = provider_updated_at(now);
            let plan = match resolve_existing_plan(
                &self.wahoo,
                user_id,
                &pending_state,
                planned_workout_id,
            )
            .await?
            {
                Some(existing_plan) => self
                    .wahoo
                    .update_plan(
                        user_id,
                        existing_plan.id,
                        WahooUpdatePlan {
                            file_base64: plan_file_base64,
                            filename: Some(plan_filename(planned_workout_id)),
                            provider_updated_at: provider_updated_at.clone(),
                        },
                    )
                    .await
                    .map_err(map_wahoo_error)?,
                None => self
                    .wahoo
                    .create_plan(
                        user_id,
                        WahooCreatePlan {
                            file_base64: plan_file_base64,
                            filename: Some(plan_filename(planned_workout_id)),
                            external_id: planned_workout_id.to_string(),
                            provider_updated_at: provider_updated_at.clone(),
                        },
                    )
                    .await
                    .map_err(map_wahoo_error)?,
            };
            let starts = projected_workout_start_at(&request.date);
            let minutes = workout_minutes(&projected_day)?;
            let update_request = WahooUpdateWorkout {
                name: projected_workout_name(&projected_day),
                workout_token: Some(workout_token.clone()),
                workout_type_id: Some(0),
                starts: Some(starts.clone()),
                minutes: Some(minutes),
                plan_id: Some(plan.id),
            };
            let workout = match resolve_existing_workout(
                &self.wahoo,
                user_id,
                &pending_state,
                &workout_token,
            )
            .await?
            {
                Some(existing_workout) => match self
                    .wahoo
                    .update_workout(user_id, existing_workout.id, update_request)
                    .await
                {
                    Ok(workout) => workout,
                    Err(crate::domain::wahoo::WahooError::NotFound) => {
                        let recovered_state = clear_stale_wahoo_workout_id(&pending_state);
                        self.sync_states
                            .upsert(recovered_state)
                            .await
                            .map_err(map_external_sync_error)?;
                        self.wahoo
                            .create_workout(
                                user_id,
                                WahooCreateWorkout {
                                    name: projected_workout_name(&projected_day)
                                        .unwrap_or_else(|| "Planned workout".to_string()),
                                    workout_token: workout_token.clone(),
                                    workout_type_id: 0,
                                    starts,
                                    minutes,
                                    plan_id: Some(plan.id),
                                },
                            )
                            .await
                            .map_err(map_wahoo_error)?
                    }
                    Err(error) => return Err(map_wahoo_error(error)),
                },
                None => {
                    if pending_state.wahoo_workout_id.is_some() {
                        let recovered_state = clear_stale_wahoo_workout_id(&pending_state);
                        self.sync_states
                            .upsert(recovered_state)
                            .await
                            .map_err(map_external_sync_error)?;
                    }
                    self.wahoo
                        .create_workout(
                            user_id,
                            WahooCreateWorkout {
                                name: projected_workout_name(&projected_day)
                                    .unwrap_or_else(|| "Planned workout".to_string()),
                                workout_token: workout_token.clone(),
                                workout_type_id: 0,
                                starts,
                                minutes,
                                plan_id: Some(plan.id),
                            },
                        )
                        .await
                        .map_err(map_wahoo_error)?
                }
            };

            Ok((plan, workout, workout_token))
        }
        .await;

        match sync_result {
            Ok((plan, workout, workout_token)) => {
                let synced_state = self
                    .sync_states
                    .upsert(pending_state.mark_wahoo_synced(
                        payload_hash,
                        self.clock.now_epoch_seconds(),
                        planned_workout_id.to_string(),
                        plan.id,
                        workout.id,
                        workout_token,
                    ))
                    .await
                    .map_err(map_external_sync_error)?;
                let all_states = self
                    .planned_workout_sync_states(user_id, &canonical_entity)
                    .await?;
                Ok(build_projected_calendar_event(
                    projected_day,
                    &all_states,
                    Some(synced_state),
                ))
            }
            Err(error) => {
                persist_failed_sync_state(
                    &self.sync_states,
                    pending_state.mark_failed(error.to_string()),
                    user_id,
                    request,
                )
                .await;
                refresh_planned_workout_day(&self.refresh, user_id, request).await;
                Err(error)
            }
        }
    }

    async fn planned_workout_sync_states(
        &self,
        user_id: &str,
        canonical_entity: &CanonicalEntityRef,
    ) -> Result<Vec<ExternalSyncState>, CalendarError> {
        self.sync_states
            .find_by_canonical_entities(user_id, std::slice::from_ref(canonical_entity))
            .await
            .map_err(map_external_sync_error)
    }
}

async fn persist_failed_sync_state<SyncStates>(
    sync_states: &SyncStates,
    failed_state: ExternalSyncState,
    user_id: &str,
    request: &SyncPlannedWorkout,
) where
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository,
{
    if let Err(error) = sync_states.upsert(failed_state).await {
        tracing::error!(
            %user_id,
            provider = request.provider.as_str(),
            operation_key = %request.operation_key,
            date = %request.date,
            %error,
            "failed to persist planned workout sync failure state"
        );
    }
}

async fn refresh_planned_workout_day<Refresh>(
    refresh: &Refresh,
    user_id: &str,
    request: &SyncPlannedWorkout,
) where
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort,
{
    if let Err(error) = refresh
        .refresh_range_for_user(user_id, &request.date, &request.date)
        .await
    {
        tracing::warn!(
            %user_id,
            provider = request.provider.as_str(),
            operation_key = %request.operation_key,
            date = %request.date,
            %error,
            "planned workout sync state persisted but calendar view refresh failed"
        );
    }
}

async fn ensure_planned_workout_marker<Tokens>(
    tokens: &Tokens,
    user_id: &str,
    planned_workout_id: &str,
) -> Result<String, CalendarError>
where
    Tokens: crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository,
{
    let match_token = match tokens
        .find_by_planned_workout_id(user_id, planned_workout_id)
        .await
        .map_err(map_planned_workout_token_error)?
    {
        Some(token) => token.match_token,
        None => {
            let match_token = build_planned_workout_match_token(planned_workout_id);
            tokens
                .upsert(PlannedWorkoutToken::new(
                    user_id.to_string(),
                    planned_workout_id.to_string(),
                    match_token.clone(),
                ))
                .await
                .map_err(map_planned_workout_token_error)?;
            match_token
        }
    };

    Ok(crate::domain::planned_workout_tokens::format_planned_workout_marker(&match_token))
}

fn ensure_sync_window<Time>(clock: &Time, date: &str) -> Result<(), CalendarError>
where
    Time: crate::domain::identity::Clock,
{
    let today = DateTime::<Utc>::from_timestamp(clock.now_epoch_seconds(), 0)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .date_naive();
    let requested_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| CalendarError::Validation(INVALID_PLANNED_WORKOUT_DATE_MESSAGE.to_string()))?;
    let latest_sync_date = today + ChronoDuration::days(6);
    if requested_date < today || requested_date > latest_sync_date {
        return Err(CalendarError::Validation(
            WAHOO_SYNC_WINDOW_MESSAGE.to_string(),
        ));
    }
    Ok(())
}

async fn find_existing_remote_event<Intervals>(
    intervals: &Intervals,
    user_id: &str,
    projected_day: &TrainingPlanProjectedDay,
    payload_hash: &str,
) -> Result<Option<Event>, CalendarError>
where
    Intervals: crate::domain::intervals::IntervalsUseCases,
{
    let date_range = crate::domain::intervals::DateRange {
        oldest: projected_day.date.clone(),
        newest: projected_day.date.clone(),
    };
    let events = intervals
        .list_events(user_id, &date_range)
        .await
        .map_err(map_intervals_error)?;

    Ok(events.into_iter().find(|event| {
        event.category == EventCategory::Workout
            && event.start_date_local.starts_with(&projected_day.date)
            && projected_event_payload_hash(
                &projected_day.date,
                event.name.as_deref(),
                comparable_workout_text_for_payload_hash(
                    event.name.as_deref(),
                    event.structured_workout_text(),
                )
                .as_deref(),
            ) == payload_hash
    }))
}

fn build_create_event(day: &TrainingPlanProjectedDay) -> CreateEvent {
    CreateEvent {
        category: EventCategory::Workout,
        start_date_local: projected_event_start_date_local(&day.date),
        event_type: Some("Ride".to_string()),
        name: projected_workout_name(day),
        description: projected_event_sync_body(day),
        indoor: false,
        color: None,
        workout_doc: None,
        file_upload: None,
    }
}

fn build_update_event(day: &TrainingPlanProjectedDay, existing_event: &Event) -> UpdateEvent {
    UpdateEvent {
        category: Some(EventCategory::Workout),
        start_date_local: Some(projected_event_start_date_local(&day.date)),
        event_type: existing_event
            .event_type
            .clone()
            .or_else(|| Some("Ride".to_string())),
        name: projected_workout_name(day),
        description: preserve_event_description(
            existing_event.description.as_deref(),
            projected_event_sync_body(day).as_deref(),
        ),
        indoor: Some(existing_event.indoor),
        color: existing_event.color.clone(),
        workout_doc: None,
        file_upload: None,
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

fn intervals_event_id(state: &ExternalSyncState) -> Option<i64> {
    state
        .external_id
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
}

async fn resolve_existing_workout<Wahoo>(
    wahoo: &Wahoo,
    user_id: &str,
    state: &ExternalSyncState,
    workout_token: &str,
) -> Result<Option<crate::domain::wahoo::WahooWorkout>, CalendarError>
where
    Wahoo: crate::domain::wahoo::WahooUseCases,
{
    const WAHOO_WORKOUT_LOOKUP_PAGE_SIZE: usize = 100;

    if let Some(wahoo_workout_id) = state.wahoo_workout_id {
        match wahoo.get_workout(user_id, wahoo_workout_id).await {
            Ok(workout) => return Ok(Some(workout)),
            Err(crate::domain::wahoo::WahooError::NotFound) => {}
            Err(error) => return Err(map_wahoo_error(error)),
        }
    }

    let mut page = 1;
    loop {
        let workouts = wahoo
            .list_workouts(user_id, page, WAHOO_WORKOUT_LOOKUP_PAGE_SIZE)
            .await
            .map_err(map_wahoo_error)?;

        let returned_count = workouts.workouts.len();
        if let Some(workout) = workouts
            .workouts
            .into_iter()
            .find(|workout| workout.workout_token.as_deref() == Some(workout_token))
        {
            return Ok(Some(workout));
        }

        if returned_count < WAHOO_WORKOUT_LOOKUP_PAGE_SIZE {
            return Ok(None);
        }

        page += 1;
    }
}

fn clear_stale_wahoo_workout_id(state: &ExternalSyncState) -> ExternalSyncState {
    let mut recovered = state.clone();
    recovered.external_id = None;
    recovered.wahoo_workout_id = None;
    recovered
}

async fn resolve_existing_plan<Wahoo>(
    wahoo: &Wahoo,
    user_id: &str,
    state: &ExternalSyncState,
    planned_workout_id: &str,
) -> Result<Option<crate::domain::wahoo::WahooPlan>, CalendarError>
where
    Wahoo: crate::domain::wahoo::WahooUseCases,
{
    if let Some(wahoo_plan_id) = state.wahoo_plan_id {
        let existing = wahoo
            .find_plan_by_external_id(user_id, planned_workout_id)
            .await
            .map_err(map_wahoo_error)?;
        if existing
            .as_ref()
            .is_some_and(|plan| plan.id == wahoo_plan_id)
        {
            return Ok(existing);
        }

        return Ok(None);
    }

    wahoo
        .find_plan_by_external_id(user_id, planned_workout_id)
        .await
        .map_err(map_wahoo_error)
}

fn provider_updated_at(now_epoch_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(now_epoch_seconds, 0)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339()
}

fn plan_filename(planned_workout_id: &str) -> String {
    format!("{planned_workout_id}.plan.json")
}

fn projected_workout_start_at(date: &str) -> String {
    format!("{date}T00:00:00.000Z")
}

fn projected_event_start_date_local(date: &str) -> String {
    format!("{date}T00:00:00")
}

fn workout_minutes(projected_day: &TrainingPlanProjectedDay) -> Result<i32, CalendarError> {
    let workout = projected_day.workout.as_ref().ok_or_else(|| {
        CalendarError::Validation("planned workout is missing workout body".to_string())
    })?;
    let total_seconds: i32 = workout
        .lines
        .iter()
        .filter_map(|line| match line {
            crate::domain::intervals::PlannedWorkoutLine::Step(step) => Some(step.duration_seconds),
            _ => None,
        })
        .sum();
    if total_seconds <= 0 {
        return Err(CalendarError::Validation(
            "planned workout has no syncable duration".to_string(),
        ));
    }
    Ok((total_seconds + 59) / 60)
}
