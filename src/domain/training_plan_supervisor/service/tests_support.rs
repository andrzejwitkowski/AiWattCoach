use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar_view::{CalendarEntryView, CalendarEntryViewError, CalendarEntryViewRefreshPort},
    external_sync::{
        CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError, ExternalSyncState,
        ExternalSyncStateRepository,
    },
    identity::Clock,
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases, WahooConfig,
    },
    training_plan::{
        TrainingPlanError, TrainingPlanPartialReplacement, TrainingPlanProjectedDay,
        TrainingPlanProjectionRepository, TrainingPlanReplacementResult, TrainingPlanSnapshot,
    },
    training_plan_supervisor::{
        TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorBatchRequest,
        TrainingPlanSupervisorBatchSubmission, TrainingPlanSupervisorOperation,
        TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
        TrainingPlanSupervisorStatus,
    },
};

#[derive(Clone, Copy)]
pub(super) struct FixedClock {
    pub(super) now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemorySupervisorOperationRepository {
    stored: Arc<Mutex<Vec<TrainingPlanSupervisorOperation>>>,
}

impl TrainingPlanSupervisorOperationRepository for InMemorySupervisorOperationRepository {
    fn find_by_worker_operation_key(
        &self,
        worker_operation_key: &str,
    ) -> super::super::BoxFuture<Result<Option<TrainingPlanSupervisorOperation>, TrainingPlanError>>
    {
        let stored = self.stored.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("supervisor operation repo mutex poisoned")
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned())
        })
    }

    fn upsert(
        &self,
        operation: TrainingPlanSupervisorOperation,
    ) -> super::super::BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("supervisor operation repo mutex poisoned");
            stored
                .retain(|existing| existing.worker_operation_key != operation.worker_operation_key);
            stored.push(operation.clone());
            Ok(operation)
        })
    }

    fn complete_review_if_pending(
        &self,
        worker_operation_key: &str,
        review: crate::domain::training_plan_supervisor::TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> super::super::BoxFuture<Result<TrainingPlanSupervisorOperation, TrainingPlanError>> {
        let stored = self.stored.clone();
        let worker_operation_key = worker_operation_key.to_string();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("supervisor operation repo mutex poisoned");
            let existing = stored
                .iter()
                .find(|operation| operation.worker_operation_key == worker_operation_key)
                .cloned()
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "training plan supervisor operation {worker_operation_key} not found"
                    ))
                })?;
            let completed = existing.complete_review(review, now_epoch_seconds)?;
            stored
                .retain(|existing| existing.worker_operation_key != completed.worker_operation_key);
            stored.push(completed.clone());
            Ok(completed)
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingProjectionRepository {
    stored: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordedCalendarRefresh {
    pub(super) user_id: String,
    pub(super) oldest: String,
    pub(super) newest: String,
}

#[derive(Clone, Default)]
pub(super) struct RecordingCalendarRefresh {
    calls: Arc<Mutex<Vec<RecordedCalendarRefresh>>>,
}

impl RecordingCalendarRefresh {
    pub(super) fn calls(&self) -> Vec<RecordedCalendarRefresh> {
        self.calls
            .lock()
            .expect("calendar refresh mutex poisoned")
            .clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CalendarEntryView>, CalendarEntryViewError>,
    > {
        let calls = self.calls.clone();
        let call = RecordedCalendarRefresh {
            user_id: user_id.to_string(),
            oldest: oldest.to_string(),
            newest: newest.to_string(),
        };
        Box::pin(async move {
            calls
                .lock()
                .expect("calendar refresh mutex poisoned")
                .push(call);
            Ok(Vec::new())
        })
    }
}

impl RecordingProjectionRepository {
    pub(super) fn seed_day(&self, day: TrainingPlanProjectedDay) {
        self.stored
            .lock()
            .expect("projection repo mutex poisoned")
            .push(day);
    }

    pub(super) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.stored
            .lock()
            .expect("projection repo mutex poisoned")
            .clone()
    }
}

#[derive(Clone)]
pub(super) struct FailingOnceProjectionRepository {
    inner: RecordingProjectionRepository,
    should_fail: Arc<Mutex<bool>>,
}

impl FailingOnceProjectionRepository {
    pub(super) fn new(inner: RecordingProjectionRepository) -> Self {
        Self {
            inner,
            should_fail: Arc::new(Mutex::new(true)),
        }
    }

    pub(super) fn stored_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.inner.stored_days()
    }
}

impl TrainingPlanProjectionRepository for RecordingProjectionRepository {
    fn list_active_by_user_id(
        &self,
        _user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "list_active_by_user_id not implemented in test".to_string(),
            ))
        })
    }

    fn find_active_by_operation_key(
        &self,
        _operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "find_active_by_operation_key not implemented in test".to_string(),
            ))
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .expect("projection repo mutex poisoned")
                .iter()
                .filter(|day| {
                    day.user_id == user_id
                        && day.operation_key == operation_key
                        && day.superseded_at_epoch_seconds.is_none()
                })
                .cloned()
                .collect())
        })
    }

    fn replace_window(
        &self,
        _snapshot: TrainingPlanSnapshot,
        _projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<TrainingPlanReplacementResult, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "replace_window not implemented in test".to_string(),
            ))
        })
    }

    fn apply_partial_replacement(
        &self,
        replacement: TrainingPlanPartialReplacement,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let replace_dates = replacement
                .replace_dates
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let mut stored = stored.lock().expect("projection repo mutex poisoned");
            for replacement_day in replacement.projected_days {
                if !replace_dates.contains(&replacement_day.date) {
                    continue;
                }
                stored.retain(|existing| {
                    !(existing.user_id == replacement_day.user_id
                        && existing.operation_key == replacement_day.operation_key
                        && existing.date == replacement_day.date
                        && existing.superseded_at_epoch_seconds.is_none())
                });
                stored.push(replacement_day);
            }
            Ok(())
        })
    }

    fn update_supervisor_status(
        &self,
        user_id: &str,
        operation_key: &str,
        supervisor_status: Option<TrainingPlanSupervisorStatus>,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            for day in stored
                .lock()
                .expect("projection repo mutex poisoned")
                .iter_mut()
            {
                if day.user_id == user_id
                    && day.operation_key == operation_key
                    && day.superseded_at_epoch_seconds.is_none()
                {
                    day.supervisor_status = supervisor_status;
                    day.updated_at_epoch_seconds = updated_at_epoch_seconds;
                }
            }
            Ok(())
        })
    }
}

impl TrainingPlanProjectionRepository for FailingOnceProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner.list_active_by_user_id(user_id)
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner.find_active_by_operation_key(operation_key)
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        self.inner
            .find_active_by_user_id_and_operation_key(user_id, operation_key)
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        today: &str,
        replaced_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<TrainingPlanReplacementResult, TrainingPlanError>,
    > {
        self.inner
            .replace_window(snapshot, projected_days, today, replaced_at_epoch_seconds)
    }

    fn apply_partial_replacement(
        &self,
        replacement: TrainingPlanPartialReplacement,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        self.inner.apply_partial_replacement(replacement)
    }

    fn update_supervisor_status(
        &self,
        user_id: &str,
        operation_key: &str,
        supervisor_status: Option<TrainingPlanSupervisorStatus>,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::training_plan::BoxFuture<Result<(), TrainingPlanError>> {
        let should_fail = self.should_fail.clone();
        let inner = self.inner.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let fail_this_call = {
                let mut should_fail = should_fail.lock().expect("projection repo mutex poisoned");
                if *should_fail {
                    *should_fail = false;
                    true
                } else {
                    false
                }
            };
            if fail_this_call {
                return Err(TrainingPlanError::Repository(
                    "projection update failed once".to_string(),
                ));
            }
            inner
                .update_supervisor_status(
                    &user_id,
                    &operation_key,
                    supervisor_status,
                    updated_at_epoch_seconds,
                )
                .await
        })
    }
}

pub(super) fn accepted_review() -> TrainingPlanSupervisorReview {
    TrainingPlanSupervisorReview {
        decision: crate::domain::training_plan_supervisor::TrainingPlanSupervisorDecision::Accept,
        reason: "plan already looks good".to_string(),
        plan: None,
    }
}

#[derive(Clone)]
pub(super) struct StubUserSettingsService {
    settings: Option<UserSettings>,
}

impl StubUserSettingsService {
    pub(super) fn enabled(model: &str) -> Self {
        Self {
            settings: Some(UserSettings {
                user_id: "user-1".to_string(),
                ai_agents: AiAgentsConfig {
                    gemini_api_key: Some("gem-key".to_string()),
                    training_plan_supervisor_enabled: true,
                    training_plan_supervisor_model: Some(model.to_string()),
                    ..AiAgentsConfig::default()
                },
                intervals: IntervalsConfig::default(),
                wahoo: WahooConfig::default(),
                options: AnalysisOptions::default(),
                availability: AvailabilitySettings::default(),
                cycling: CyclingSettings::default(),
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }),
        }
    }

    pub(super) fn disabled() -> Self {
        Self {
            settings: Some(UserSettings {
                user_id: "user-1".to_string(),
                ai_agents: AiAgentsConfig::default(),
                intervals: IntervalsConfig::default(),
                wahoo: WahooConfig::default(),
                options: AnalysisOptions::default(),
                availability: AvailabilitySettings::default(),
                cycling: CyclingSettings::default(),
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            }),
        }
    }

    pub(super) fn no_settings() -> Self {
        Self { settings: None }
    }
}

impl UserSettingsUseCases for StubUserSettingsService {
    fn find_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move { Ok(settings) })
    }

    fn get_settings(
        &self,
        _user_id: &str,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move {
            settings.ok_or_else(|| SettingsError::Repository("missing settings".to_string()))
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async {
            Err(SettingsError::Repository(
                "update_ai_agents not implemented in test".to_string(),
            ))
        })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async {
            Err(SettingsError::Repository(
                "update_intervals not implemented in test".to_string(),
            ))
        })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async {
            Err(SettingsError::Repository(
                "update_options not implemented in test".to_string(),
            ))
        })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async {
            Err(SettingsError::Repository(
                "update_availability not implemented in test".to_string(),
            ))
        })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> crate::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async {
            Err(SettingsError::Repository(
                "update_cycling not implemented in test".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingBatchPort {
    requests: Arc<Mutex<Vec<TrainingPlanSupervisorBatchRequest>>>,
}

impl RecordingBatchPort {
    pub(super) fn requests(&self) -> Vec<TrainingPlanSupervisorBatchRequest> {
        self.requests
            .lock()
            .expect("batch requests mutex poisoned")
            .clone()
    }
}

impl TrainingPlanSupervisorBatchPort for RecordingBatchPort {
    fn submit_review(
        &self,
        _api_key: &str,
        request: TrainingPlanSupervisorBatchRequest,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<TrainingPlanSupervisorBatchSubmission, TrainingPlanError>,
    > {
        let requests = self.requests.clone();
        Box::pin(async move {
            requests
                .lock()
                .expect("batch requests mutex poisoned")
                .push(request);
            Ok(TrainingPlanSupervisorBatchSubmission {
                batch_name: "batches/supervisor-1".to_string(),
            })
        })
    }

    fn download_result(
        &self,
        _api_key: &str,
        _batch_name: &str,
    ) -> crate::domain::training_plan_supervisor::BoxFuture<
        Result<TrainingPlanSupervisorReview, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Unavailable(
                "download_result not implemented in test".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct FixedSyncStateRepository {
    states: Arc<Mutex<Vec<ExternalSyncState>>>,
}

impl FixedSyncStateRepository {
    pub(super) fn seed_state(&self, state: ExternalSyncState) {
        self.states
            .lock()
            .expect("sync state mutex poisoned")
            .push(state);
    }
}

impl ExternalSyncStateRepository for FixedSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        let entity_ids = canonical_entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect::<std::collections::HashSet<_>>();
        Box::pin(async move {
            Ok(states
                .lock()
                .expect("sync state mutex poisoned")
                .iter()
                .filter(|state| {
                    state.user_id == user_id
                        && entity_ids.contains(&state.canonical_entity.entity_id)
                })
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        Box::pin(async { Ok(()) })
    }

    fn find_by_wahoo_plan_id(
        &self,
        _user_id: &str,
        _wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_wahoo_workout_token(
        &self,
        _user_id: &str,
        _wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_by_provider_and_external_id(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }
}
