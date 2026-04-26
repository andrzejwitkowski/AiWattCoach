use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::domain::{
    calendar::{CalendarError, CalendarEvent, PlannedWorkoutSyncRecord, SyncPlannedWorkout},
    intervals::PlannedWorkoutLine,
    planned_workout_tokens::{build_planned_workout_match_token, PlannedWorkoutToken},
    planned_workout_wahoo_syncs::PlannedWorkoutWahooSyncRecord,
    training_plan::TrainingPlanProjectedDay,
    wahoo::{WahooCreatePlan, WahooCreateWorkout, WahooUpdatePlan, WahooUpdateWorkout},
};

use super::{
    errors::{
        map_planned_workout_token_error, map_settings_error, map_training_plan_error,
        map_wahoo_error, map_wahoo_sync_error,
    },
    projected::{
        build_projected_calendar_event, projected_day_payload_hash, projected_workout_id,
        projected_workout_name,
    },
    CalendarService,
};

impl<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
        Settings,
        Tokens,
        Refresh,
        Completed,
    >
    CalendarService<
        Intervals,
        Entries,
        Projections,
        Syncs,
        Time,
        Wahoo,
        WahooSyncs,
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
    Syncs: crate::domain::calendar::PlannedWorkoutSyncRepository + Clone,
    Time: crate::domain::identity::Clock + Clone,
    Wahoo: crate::domain::wahoo::WahooUseCases + Clone,
    WahooSyncs:
        crate::domain::planned_workout_wahoo_syncs::PlannedWorkoutWahooSyncRepository + Clone,
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
        let planned_workout_id = projected_workout_id(&request.operation_key, &request.date);

        if projected_day.rest_day || projected_day.workout.is_none() {
            return Err(CalendarError::Validation(
                "Only planned workout days can be synchronized".to_string(),
            ));
        }
        ensure_sync_window(&self.clock, &request.date)?;

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
        let wahoo_sync_record = self
            .wahoo_syncs
            .find_by_planned_workout_id(user_id, &planned_workout_id)
            .await
            .map_err(map_wahoo_sync_error)?
            .unwrap_or_else(|| {
                PlannedWorkoutWahooSyncRecord::pending(
                    user_id.to_string(),
                    request.operation_key.clone(),
                    request.date.clone(),
                    planned_workout_id.clone(),
                    projected_day.workout_id.clone(),
                    planned_workout_id.clone(),
                    now,
                )
            });

        let pending_record = self
            .syncs
            .upsert(
                sync_record
                    .mark_pending_without_remote_event(projected_day.workout_id.clone(), now),
            )
            .await?;
        let pending_wahoo_record = self
            .wahoo_syncs
            .upsert(wahoo_sync_record.mark_pending(now))
            .await
            .map_err(map_wahoo_sync_error)?;

        let sync_result: Result<
            (
                crate::domain::wahoo::WahooPlan,
                crate::domain::wahoo::WahooWorkout,
                String,
                String,
            ),
            CalendarError,
        > = async {
            let settings = self
                .settings
                .find_by_user_id(user_id)
                .await
                .map_err(map_settings_error)?
                .ok_or_else(|| {
                    CalendarError::Validation(
                        "Set your cycling FTP in Settings before syncing to Wahoo".to_string(),
                    )
                })?;
            let ftp_watts = settings.cycling.ftp_watts.ok_or_else(|| {
                CalendarError::Validation(
                    "Set your cycling FTP in Settings before syncing to Wahoo".to_string(),
                )
            })?;
            let planned_workout_marker = ensure_planned_workout_marker(
                &self.planned_workout_tokens,
                user_id,
                &planned_workout_id,
            )
            .await?;
            let workout_token = pending_wahoo_record
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
                &pending_wahoo_record,
                &planned_workout_id,
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
                            filename: Some(plan_filename(&planned_workout_id)),
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
                            filename: Some(plan_filename(&planned_workout_id)),
                            external_id: planned_workout_id.clone(),
                            provider_updated_at: provider_updated_at.clone(),
                        },
                    )
                    .await
                    .map_err(map_wahoo_error)?,
            };
            let starts = projected_workout_start_at(&request.date);
            let minutes = workout_minutes(&projected_day)?;
            let workout = if let Some(wahoo_workout_id) = pending_wahoo_record.wahoo_workout_id {
                self.wahoo
                    .update_workout(
                        user_id,
                        wahoo_workout_id,
                        WahooUpdateWorkout {
                            name: projected_workout_name(&projected_day),
                            workout_token: Some(workout_token.clone()),
                            workout_type_id: Some(0),
                            starts: Some(starts),
                            minutes: Some(minutes),
                            plan_id: Some(plan.id),
                        },
                    )
                    .await
                    .map_err(map_wahoo_error)?
            } else {
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
            };

            Ok((plan, workout, workout_token, planned_workout_marker))
        }
        .await;

        match sync_result {
            Ok((plan, workout, workout_token, _planned_workout_marker)) => {
                self.wahoo_syncs
                    .upsert(pending_wahoo_record.mark_synced(
                        payload_hash.clone(),
                        plan.id,
                        workout.id,
                        workout_token,
                        self.clock.now_epoch_seconds(),
                    ))
                    .await
                    .map_err(map_wahoo_sync_error)?;
                let synced_record = self
                    .syncs
                    .upsert(pending_record.mark_synced_without_remote_event(
                        projected_day.workout_id.clone(),
                        payload_hash,
                        self.clock.now_epoch_seconds(),
                    ))
                    .await?;
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
                let sync_action = if pending_wahoo_record.wahoo_workout_id.is_some() {
                    "update"
                } else {
                    "create"
                };
                tracing::warn!(
                    user_id,
                    operation_key = %request.operation_key,
                    date = %request.date,
                    sync_action,
                    linked_wahoo_plan_id = pending_wahoo_record.wahoo_plan_id,
                    linked_wahoo_workout_id = pending_wahoo_record.wahoo_workout_id,
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
                let failed_wahoo_record = pending_wahoo_record
                    .mark_failed(error.to_string(), self.clock.now_epoch_seconds());
                if let Err(persist_error) = self.syncs.upsert(failed_record).await {
                    tracing::error!(
                        user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        error = %persist_error,
                        "failed to persist planned workout sync failure state"
                    );
                }
                if let Err(persist_error) = self.wahoo_syncs.upsert(failed_wahoo_record).await {
                    tracing::error!(
                        user_id,
                        operation_key = %request.operation_key,
                        date = %request.date,
                        error = %persist_error,
                        "failed to persist planned workout Wahoo sync failure state"
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
    let requested_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| {
        CalendarError::Validation("planned workout date must be in YYYY-MM-DD format".to_string())
    })?;
    let latest_sync_date = today + ChronoDuration::days(6);
    if requested_date < today || requested_date > latest_sync_date {
        return Err(CalendarError::Validation(
            "Only planned workouts scheduled between today and the next 6 days can sync to Wahoo"
                .to_string(),
        ));
    }
    Ok(())
}

async fn resolve_existing_plan<Wahoo>(
    wahoo: &Wahoo,
    user_id: &str,
    record: &PlannedWorkoutWahooSyncRecord,
    planned_workout_id: &str,
) -> Result<Option<crate::domain::wahoo::WahooPlan>, CalendarError>
where
    Wahoo: crate::domain::wahoo::WahooUseCases,
{
    if let Some(wahoo_plan_id) = record.wahoo_plan_id {
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

fn workout_minutes(projected_day: &TrainingPlanProjectedDay) -> Result<i32, CalendarError> {
    let workout = projected_day.workout.as_ref().ok_or_else(|| {
        CalendarError::Validation("planned workout is missing workout body".to_string())
    })?;
    let total_seconds: i32 = workout
        .lines
        .iter()
        .filter_map(|line| match line {
            PlannedWorkoutLine::Step(step) => Some(step.duration_seconds),
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
