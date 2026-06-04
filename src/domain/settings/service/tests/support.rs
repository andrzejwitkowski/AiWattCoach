use super::super::*;
use crate::domain::settings::WahooUserIdBackfillCandidate;
use crate::domain::{
    external_sync::{
        BoxFuture as SyncBoxFuture, ExternalProvider, ExternalSyncRepositoryError,
        ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
    },
    identity::Clock,
    llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
    training_load::{
        BoxFuture as TrainingLoadBoxFuture, FtpHistoryEntry, FtpHistoryRepository,
        TrainingLoadError, TrainingLoadRecomputeUseCases,
    },
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(super) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryUserSettingsRepository {
    settings: Arc<Mutex<Option<UserSettings>>>,
}

impl InMemoryUserSettingsRepository {
    pub(super) fn with_settings(settings: UserSettings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(Some(settings))),
        }
    }
}

impl UserSettingsRepository for InMemoryUserSettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(settings
                .lock()
                .unwrap()
                .clone()
                .filter(|settings| settings.user_id == user_id))
        })
    }

    fn find_by_wahoo_user_id(
        &self,
        wahoo_user_id: i64,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move {
            Ok(settings
                .lock()
                .unwrap()
                .clone()
                .filter(|settings| settings.wahoo.user_id == Some(wahoo_user_id)))
        })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> BoxFuture<Result<Vec<WahooUserIdBackfillCandidate>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move {
            Ok(settings
                .lock()
                .unwrap()
                .clone()
                .filter(|settings| {
                    settings.wahoo.connected
                        && settings.wahoo.user_id.is_none()
                        && settings
                            .wahoo
                            .refresh_token
                            .as_deref()
                            .is_some_and(|value| !value.trim().is_empty())
                })
                .into_iter()
                .map(|settings| WahooUserIdBackfillCandidate {
                    user_id: settings.user_id,
                    wahoo: settings.wahoo,
                })
                .collect())
        })
    }

    fn upsert(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let store = self.settings.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(settings.clone());
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut guard = settings.lock().unwrap();
            let current = guard
                .as_mut()
                .filter(|current| current.user_id == user_id)
                .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
            current.ai_agents = ai_agents;
            current.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut guard = settings.lock().unwrap();
            let current = guard
                .as_mut()
                .filter(|current| current.user_id == user_id)
                .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
            current.intervals = intervals;
            current.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
        _updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        Box::pin(async move { unreachable!("not used in test") })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut guard = settings.lock().unwrap();
            let current = guard
                .as_mut()
                .filter(|current| current.user_id == user_id)
                .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
            current.cycling = cycling;
            current.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut guard = settings.lock().unwrap();
            let current = guard
                .as_mut()
                .filter(|current| current.user_id == user_id)
                .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
            current.availability = availability;
            current.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingCacheRepository {
    deleted_users: Arc<Mutex<Vec<String>>>,
}

impl RecordingCacheRepository {
    pub(super) fn deleted_users(&self) -> Vec<String> {
        self.deleted_users.lock().unwrap().clone()
    }
}

impl LlmContextCacheRepository for RecordingCacheRepository {
    fn find_reusable(
        &self,
        _user_id: &str,
        _provider: &crate::domain::llm::LlmProvider,
        _model: &str,
        _scope_key: &str,
        _context_hash: &str,
        _now_epoch_seconds: i64,
    ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        Box::pin(async move { unreachable!("not used in test") })
    }

    fn upsert(&self, _cache: LlmContextCache) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
        Box::pin(async move { unreachable!("not used in test") })
    }

    fn delete_by_user_id(&self, user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
        let deleted_users = self.deleted_users.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            deleted_users.lock().unwrap().push(user_id);
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingFtpHistoryRepository {
    entries: Arc<Mutex<Vec<FtpHistoryEntry>>>,
}

impl RecordingFtpHistoryRepository {
    pub(super) fn stored(&self) -> Vec<FtpHistoryEntry> {
        let mut entries = self.entries.lock().unwrap().clone();
        entries.sort_by(|left, right| left.effective_from_date.cmp(&right.effective_from_date));
        entries
    }
}

impl FtpHistoryRepository for RecordingFtpHistoryRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> TrainingLoadBoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>> {
        let entries = self.entries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(entries
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn find_effective_for_date(
        &self,
        user_id: &str,
        date: &str,
    ) -> TrainingLoadBoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>> {
        let entries = self.entries.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move {
            Ok(entries
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| entry.user_id == user_id && entry.effective_from_date <= date)
                .cloned()
                .max_by_key(|entry| entry.effective_from_date.clone()))
        })
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> TrainingLoadBoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        let entries = self.entries.clone();
        Box::pin(async move {
            let mut entries = entries.lock().unwrap();
            entries.retain(|existing| {
                !(existing.user_id == entry.user_id
                    && existing.effective_from_date == entry.effective_from_date)
            });
            entries.push(entry.clone());
            Ok(entry)
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct RecordingTrainingLoadRecomputeService {
    calls: Arc<Mutex<Vec<(String, String, i64)>>>,
}

impl RecordingTrainingLoadRecomputeService {
    pub(super) fn calls(&self) -> Vec<(String, String, i64)> {
        self.calls.lock().unwrap().clone()
    }
}

impl TrainingLoadRecomputeUseCases for RecordingTrainingLoadRecomputeService {
    fn recompute_from(
        &self,
        user_id: &str,
        oldest_date: &str,
        now_epoch_seconds: i64,
    ) -> TrainingLoadBoxFuture<Result<(), TrainingLoadError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest_date = oldest_date.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest_date, now_epoch_seconds));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct FailingFtpHistoryRepository;

impl FtpHistoryRepository for FailingFtpHistoryRepository {
    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> TrainingLoadBoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>> {
        Box::pin(async move {
            Err(TrainingLoadError::Repository(
                "ftp history unavailable".to_string(),
            ))
        })
    }

    fn find_effective_for_date(
        &self,
        _user_id: &str,
        _date: &str,
    ) -> TrainingLoadBoxFuture<Result<Option<FtpHistoryEntry>, TrainingLoadError>> {
        Box::pin(async move {
            Err(TrainingLoadError::Repository(
                "ftp history unavailable".to_string(),
            ))
        })
    }

    fn upsert(
        &self,
        _entry: FtpHistoryEntry,
    ) -> TrainingLoadBoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        Box::pin(async move {
            Err(TrainingLoadError::Repository(
                "ftp history unavailable".to_string(),
            ))
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct InMemoryProviderPollStateRepository {
    states: Arc<Mutex<Vec<ProviderPollState>>>,
}

impl InMemoryProviderPollStateRepository {
    pub(super) fn stored(&self) -> Vec<ProviderPollState> {
        self.states.lock().unwrap().clone()
    }
}

impl ProviderPollStateRepository for InMemoryProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> SyncBoxFuture<Result<ProviderPollState, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        Box::pin(async move {
            let mut states = states.lock().unwrap();
            states.retain(|existing| {
                !(existing.user_id == state.user_id
                    && existing.provider == state.provider
                    && existing.stream == state.stream)
            });
            states.push(state.clone());
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> SyncBoxFuture<Result<Vec<ProviderPollState>, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.is_due(now_epoch_seconds))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> SyncBoxFuture<Result<Option<ProviderPollState>, ExternalSyncRepositoryError>> {
        let states = self.states.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id && state.provider == provider && state.stream == stream
                })
                .cloned())
        })
    }
}
