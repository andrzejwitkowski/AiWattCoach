use std::sync::{Arc, Mutex};

use crate::domain::{
    identity::Clock,
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
        SettingsError, UserSettings, UserSettingsUseCases, WahooConfig,
    },
    training_plan::{
        TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
        TrainingPlanReplacementResult, TrainingPlanSnapshot,
    },
    training_plan_supervisor::{
        TrainingPlanSupervisorOperation, TrainingPlanSupervisorOperationRepository,
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
}

#[derive(Clone, Default)]
pub(super) struct RecordingProjectionRepository {
    stored: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
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
        _user_id: &str,
        _operation_key: &str,
    ) -> crate::domain::training_plan::BoxFuture<
        Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>,
    > {
        Box::pin(async {
            Err(TrainingPlanError::Repository(
                "find_active_by_user_id_and_operation_key not implemented in test".to_string(),
            ))
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
