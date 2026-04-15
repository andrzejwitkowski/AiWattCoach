use crate::domain::{
    calendar::{CalendarError, CalendarEvent, PlannedWorkoutSyncRecord, SyncPlannedWorkout},
    external_sync::{ExternalProvider, ProviderPollState, ProviderPollStream},
    intervals::{CreateEvent, DateRange, Event, EventCategory, IntervalsError},
    planned_workout_tokens::{
        build_planned_workout_match_token, extract_planned_workout_marker,
        format_planned_workout_marker, PlannedWorkoutToken,
    },
    training_plan::TrainingPlanProjectedDay,
};

use super::{
    errors::{map_intervals_error, map_planned_workout_token_error, map_training_plan_error},
    projected::{
        build_projected_calendar_event, build_update_event, projected_day_payload_hash,
        projected_event_payload_hash, projected_event_start_date_local, projected_workout_id,
        projected_workout_name, projected_workout_sync_description,
    },
    CalendarService,
};

impl<Intervals, Entries, Projections, Syncs, Time, Tokens, PollStates, Refresh, Completed>
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Tokens,
        PollStates,
        Refresh,
        Completed,
    >
where
    Intervals: crate::domain::intervals::IntervalsUseCases + Clone,
    Entries: crate::domain::calendar_view::CalendarEntryViewRepository + Clone,
    Completed: crate::domain::completed_workouts::CompletedWorkoutRepository + Clone,
    Projections: crate::domain::training_plan::TrainingPlanProjectionRepository + Clone,
    Syncs: crate::domain::calendar::PlannedWorkoutSyncRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Tokens: crate::domain::planned_workout_tokens::PlannedWorkoutTokenRepository + Clone,
    PollStates: crate::domain::external_sync::ProviderPollStateRepository + Clone,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort + Clone,
{
    pub(super) async fn mark_calendar_poll_due_soon(&self, user_id: &str) {
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
                tracing::warn!(%user_id, %error, "planned workout sync succeeded but failed to load provider poll state");
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
            tracing::warn!(%user_id, %error, "planned workout sync succeeded but failed to mark calendar poll due soon");
        }
    }

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
        let planned_workout_id = projected_workout_id(&request.operation_key, &request.date);

        if projected_day.rest_day || projected_day.workout.is_none() {
            return Err(CalendarError::Validation(
                "Only planned workout days can be synchronized".to_string(),
            ));
        }

        let payload_hash = projected_day_payload_hash(&projected_day);
        let now = self.clock.now_epoch_seconds();
        let sync_record = self
            .syncs
            .find_by_user_id_and_projection(user_id, &request.operation_key, &request.date)
            .await?
            .unwrap_or_else(|| {
                PlannedWorkoutSyncRecord::pending(
                    user_id.to_string(),
                    request.operation_key.clone(),
                    request.date.clone(),
                    projected_day.workout_id.clone(),
                    now,
                )
            });

        let pending_record = self
            .syncs
            .upsert(sync_record.mark_pending(projected_day.workout_id.clone(), now))
            .await?;

        let sync_result = async {
            let existing_remote_event = if let Some(intervals_event_id) =
                pending_record.intervals_event_id
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
                let planned_workout_marker = ensure_planned_workout_marker(
                    &self.planned_workout_tokens,
                    user_id,
                    &planned_workout_id,
                    existing_remote_event.description.as_deref(),
                )
                .await?;
                self.intervals
                    .update_event(
                        user_id,
                        existing_remote_event.id,
                        build_update_event(
                            &projected_day,
                            &existing_remote_event,
                            Some(&planned_workout_marker),
                        ),
                    )
                    .await
                    .map_err(map_intervals_error)?
            } else {
                let planned_workout_marker = ensure_planned_workout_marker(
                    &self.planned_workout_tokens,
                    user_id,
                    &planned_workout_id,
                    None,
                )
                .await?;
                self.intervals
                    .create_event(
                        user_id,
                        build_create_event(&projected_day, Some(&planned_workout_marker)),
                    )
                    .await
                    .map_err(map_intervals_error)?
            };

            Ok(remote_event)
        }
        .await;

        match sync_result {
            Ok(remote_event) => {
                let synced_record = self
                    .syncs
                    .upsert(pending_record.mark_synced(
                        remote_event.id,
                        projected_day.workout_id.clone(),
                        payload_hash,
                        self.clock.now_epoch_seconds(),
                    ))
                    .await?;
                self.mark_calendar_poll_due_soon(user_id).await;
                if let Err(error) = self
                    .refresh
                    .refresh_range_for_user(user_id, &request.date, &request.date)
                    .await
                {
                    tracing::warn!(
                        %user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        %error,
                        "planned workout sync succeeded but calendar view refresh failed"
                    );
                }
                Ok(build_projected_calendar_event(
                    projected_day,
                    Some(&synced_record),
                ))
            }
            Err(error) => {
                let sync_action = if pending_record.intervals_event_id.is_some() {
                    "update"
                } else {
                    "create"
                };
                tracing::warn!(
                    user_id,
                    operation_key = %request.operation_key,
                    date = %request.date,
                    sync_action,
                    linked_intervals_event_id = pending_record.intervals_event_id,
                    payload_hash = %payload_hash,
                    workout_name = projected_workout_name(&projected_day).as_deref().unwrap_or_default(),
                    error = %error,
                    "planned workout sync failed"
                );
                let failed_record = pending_record.mark_failed(
                    projected_day.workout_id.clone(),
                    error.to_string(),
                    self.clock.now_epoch_seconds(),
                );
                if let Err(persist_error) = self.syncs.upsert(failed_record).await {
                    tracing::error!(
                        user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        error = %persist_error,
                        "failed to persist planned workout sync failure state"
                    );
                } else if let Err(refresh_error) = self
                    .refresh
                    .refresh_range_for_user(user_id, &request.date, &request.date)
                    .await
                {
                    tracing::warn!(
                        %user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        %refresh_error,
                        "planned workout sync failure state persisted but calendar view refresh failed"
                    );
                }
                Err(error)
            }
        }
    }
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
    let date_range = DateRange {
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
                event
                    .description
                    .as_deref()
                    .or(event.workout_doc.as_deref()),
            ) == payload_hash
    }))
}

fn build_create_event(
    day: &TrainingPlanProjectedDay,
    planned_workout_marker: Option<&str>,
) -> CreateEvent {
    CreateEvent {
        category: EventCategory::Workout,
        start_date_local: projected_event_start_date_local(&day.date),
        event_type: Some("Ride".to_string()),
        name: projected_workout_name(day),
        description: projected_workout_sync_description(day, planned_workout_marker),
        indoor: false,
        color: None,
        workout_doc: None,
        file_upload: None,
    }
}

async fn ensure_planned_workout_marker<Tokens>(
    tokens: &Tokens,
    user_id: &str,
    planned_workout_id: &str,
    existing_description: Option<&str>,
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
        None => match existing_description.and_then(extract_planned_workout_marker) {
            Some(match_token) => {
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
        },
    };

    Ok(format_planned_workout_marker(&match_token))
}
