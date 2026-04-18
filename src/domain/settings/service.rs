use super::{
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsRepository,
};
use chrono::{DateTime, Utc};

use crate::domain::llm::LlmContextCacheRepository;
use crate::domain::settings::{ports::BoxFuture, validation};
use crate::domain::{
    external_sync::{
        ExternalProvider, NoopProviderPollStateRepository, ProviderPollState,
        ProviderPollStateRepository, ProviderPollStream,
    },
    identity::Clock,
    training_load::{FtpHistoryEntry, FtpHistoryRepository, TrainingLoadRecomputeUseCases},
};
use std::sync::Arc;

trait FtpHistoryWritePort: Send + Sync {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_load::BoxFuture<
        Result<Vec<FtpHistoryEntry>, crate::domain::training_load::TrainingLoadError>,
    >;

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> crate::domain::training_load::BoxFuture<
        Result<FtpHistoryEntry, crate::domain::training_load::TrainingLoadError>,
    >;
}

impl<Repository> FtpHistoryWritePort for Repository
where
    Repository: FtpHistoryRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_load::BoxFuture<
        Result<Vec<FtpHistoryEntry>, crate::domain::training_load::TrainingLoadError>,
    > {
        FtpHistoryRepository::list_by_user_id(self, user_id)
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> crate::domain::training_load::BoxFuture<
        Result<FtpHistoryEntry, crate::domain::training_load::TrainingLoadError>,
    > {
        FtpHistoryRepository::upsert(self, entry)
    }
}

pub trait UserSettingsUseCases: Send + Sync {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>>;
    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>>;
}

#[derive(Clone)]
pub struct UserSettingsService<Repo, Time, PollStates = NoopProviderPollStateRepository>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    repository: Repo,
    clock: Time,
    poll_states: PollStates,
    llm_context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    ftp_history_repository: Option<Arc<dyn FtpHistoryWritePort>>,
    training_load_recompute_service: Option<Arc<dyn TrainingLoadRecomputeUseCases>>,
}

impl<Repo, Time> UserSettingsService<Repo, Time>
where
    Repo: UserSettingsRepository,
    Time: Clock,
{
    pub fn new(repository: Repo, clock: Time) -> Self {
        Self {
            repository,
            clock,
            poll_states: NoopProviderPollStateRepository,
            llm_context_cache_repository: None,
            ftp_history_repository: None,
            training_load_recompute_service: None,
        }
    }
}

impl<Repo, Time, PollStates> UserSettingsService<Repo, Time, PollStates>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    pub fn with_provider_poll_states<NextPollStates>(
        self,
        poll_states: NextPollStates,
    ) -> UserSettingsService<Repo, Time, NextPollStates>
    where
        NextPollStates: ProviderPollStateRepository,
    {
        UserSettingsService {
            repository: self.repository,
            clock: self.clock,
            poll_states,
            llm_context_cache_repository: self.llm_context_cache_repository,
            ftp_history_repository: self.ftp_history_repository,
            training_load_recompute_service: self.training_load_recompute_service,
        }
    }

    pub fn with_llm_context_cache_repository(
        mut self,
        llm_context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.llm_context_cache_repository = Some(llm_context_cache_repository);
        self
    }

    pub fn with_ftp_history_repository(
        mut self,
        ftp_history_repository: impl FtpHistoryRepository,
    ) -> Self {
        self.ftp_history_repository = Some(Arc::new(ftp_history_repository));
        self
    }

    pub fn with_training_load_recompute_service(
        mut self,
        training_load_recompute_service: Arc<dyn TrainingLoadRecomputeUseCases>,
    ) -> Self {
        self.training_load_recompute_service = Some(training_load_recompute_service);
        self
    }

    async fn get_or_create(&self, user_id: &str) -> Result<UserSettings, SettingsError> {
        if let Some(settings) = self.repository.find_by_user_id(user_id).await? {
            return Ok(settings);
        }
        let now = self.clock.now_epoch_seconds();
        let defaults = UserSettings::new_defaults(user_id.to_string(), now);
        self.repository.upsert(defaults).await
    }
}

impl<Repo, Time, PollStates> UserSettingsUseCases for UserSettingsService<Repo, Time, PollStates>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    fn find_settings(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { repository.find_by_user_id(&user_id).await })
    }

    fn get_settings(&self, user_id: &str) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_or_create(&user_id).await })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let previous = service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_ai_agents(&user_id, ai_agents, now)
                .await?;
            let updated = service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })?;

            if should_invalidate_llm_cache(&previous.ai_agents, &updated.ai_agents) {
                if let Some(repository) = &service.llm_context_cache_repository {
                    if let Err(error) = repository.delete_by_user_id(&user_id).await {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "failed to invalidate llm context cache after settings update"
                        );
                    }
                }
            }

            Ok(updated)
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let previous = service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            let intervals = normalize_intervals_config(intervals);
            service
                .repository
                .update_intervals(&user_id, intervals.clone(), now)
                .await?;
            if let Err(error) = sync_poll_states_after_intervals_update(
                &service.poll_states,
                &user_id,
                &previous.intervals,
                &intervals,
                now,
            )
            .await
            {
                tracing::warn!(
                    user_id = %user_id,
                    error = %error,
                    "interval settings were saved but provider poll state sync failed"
                );
            }
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_options(&user_id, options, now)
                .await?;
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let previous = service.get_or_create(&user_id).await?;
            let now = service.clock.now_epoch_seconds();
            let recompute_from_date = epoch_seconds_to_utc_date(previous.created_at_epoch_seconds);
            let ftp_changed = previous.cycling.ftp_watts != cycling.ftp_watts;
            service
                .repository
                .update_cycling(&user_id, cycling.clone(), now)
                .await?;
            let updated = service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })?;

            if ftp_changed {
                if let Some(repository) = &service.ftp_history_repository {
                    if let Err(error) =
                        seed_initial_ftp_history_if_needed(repository.as_ref(), &previous).await
                    {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "cycling settings were saved but initial ftp history seed failed"
                        );
                    }

                    let history_ftp_watts =
                        updated.cycling.ftp_watts.map(|ftp| ftp as i32).unwrap_or(0);
                    if let Err(error) = repository
                        .upsert(FtpHistoryEntry {
                            user_id: user_id.clone(),
                            effective_from_date: epoch_seconds_to_utc_date(now),
                            ftp_watts: history_ftp_watts,
                            source: crate::domain::training_load::FtpSource::Settings,
                            created_at_epoch_seconds: now,
                            updated_at_epoch_seconds: now,
                        })
                        .await
                    {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "cycling settings were saved but ftp history update failed"
                        );
                    }
                }

                if let Some(recompute_service) = &service.training_load_recompute_service {
                    if let Err(error) = recompute_service
                        .recompute_from(&user_id, &recompute_from_date, now)
                        .await
                    {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "cycling settings were saved but training load recompute failed"
                        );
                    }
                }

                if let Some(repository) = &service.llm_context_cache_repository {
                    if let Err(error) = repository.delete_by_user_id(&user_id).await {
                        tracing::warn!(
                            user_id = %user_id,
                            error = %error,
                            "failed to invalidate llm context cache after settings update"
                        );
                    }
                }
            }

            Ok(updated)
        })
    }

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> BoxFuture<Result<UserSettings, SettingsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            service.get_or_create(&user_id).await?;
            let availability = validation::validate_availability(availability)?;
            let now = service.clock.now_epoch_seconds();
            service
                .repository
                .update_availability(&user_id, availability, now)
                .await?;
            service
                .repository
                .find_by_user_id(&user_id)
                .await?
                .ok_or_else(|| {
                    SettingsError::Repository("settings disappeared after update".to_string())
                })
        })
    }
}

fn normalize_intervals_config(mut intervals: IntervalsConfig) -> IntervalsConfig {
    intervals.api_key = normalize_optional_non_empty(intervals.api_key);
    intervals.athlete_id = normalize_optional_non_empty(intervals.athlete_id);
    intervals.connected =
        intervals.connected && intervals.api_key.is_some() && intervals.athlete_id.is_some();
    intervals
}

async fn sync_poll_states_after_intervals_update<PollStates>(
    poll_states: &PollStates,
    user_id: &str,
    previous: &IntervalsConfig,
    intervals: &IntervalsConfig,
    now_epoch_seconds: i64,
) -> Result<(), SettingsError>
where
    PollStates: ProviderPollStateRepository,
{
    let credentials_changed = previous.api_key != intervals.api_key
        || previous.athlete_id != intervals.athlete_id
        || previous.connected != intervals.connected;

    for stream in [
        ProviderPollStream::Calendar,
        ProviderPollStream::CompletedWorkouts,
    ] {
        let existing = poll_states
            .find_by_provider_and_stream(user_id, ExternalProvider::Intervals, stream.clone())
            .await
            .map_err(map_poll_state_error)?;

        let state = match existing {
            Some(state) => {
                if !intervals.connected {
                    ProviderPollState {
                        next_due_at_epoch_seconds: i64::MAX,
                        cursor: None,
                        backoff_until_epoch_seconds: None,
                        last_error: None,
                        ..state
                    }
                } else if credentials_changed {
                    ProviderPollState {
                        next_due_at_epoch_seconds: now_epoch_seconds,
                        cursor: None,
                        backoff_until_epoch_seconds: None,
                        last_error: None,
                        last_attempted_at_epoch_seconds: None,
                        last_successful_at_epoch_seconds: None,
                        ..state
                    }
                } else if state.next_due_at_epoch_seconds <= now_epoch_seconds
                    && state.backoff_until_epoch_seconds.is_none()
                {
                    state
                } else {
                    ProviderPollState { ..state }
                }
            }
            None => ProviderPollState::new(
                user_id.to_string(),
                ExternalProvider::Intervals,
                stream,
                if intervals.connected {
                    now_epoch_seconds
                } else {
                    i64::MAX
                },
            ),
        };

        poll_states
            .upsert(state)
            .await
            .map_err(map_poll_state_error)?;
    }

    Ok(())
}

fn normalize_optional_non_empty(value: Option<String>) -> Option<String> {
    let normalized = value?.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn map_poll_state_error(
    error: crate::domain::external_sync::ExternalSyncRepositoryError,
) -> SettingsError {
    SettingsError::Repository(error.to_string())
}

fn map_training_load_error(
    error: crate::domain::training_load::TrainingLoadError,
) -> SettingsError {
    SettingsError::Repository(error.to_string())
}

async fn seed_initial_ftp_history_if_needed(
    repository: &dyn FtpHistoryWritePort,
    settings: &UserSettings,
) -> Result<(), SettingsError> {
    let Some(initial_ftp) = settings.cycling.ftp_watts else {
        return Ok(());
    };
    let existing = repository
        .list_by_user_id(&settings.user_id)
        .await
        .map_err(map_training_load_error)?;
    if !existing.is_empty() {
        return Ok(());
    }

    repository
        .upsert(FtpHistoryEntry {
            user_id: settings.user_id.clone(),
            effective_from_date: epoch_seconds_to_utc_date(settings.created_at_epoch_seconds),
            ftp_watts: initial_ftp as i32,
            source: crate::domain::training_load::FtpSource::Settings,
            created_at_epoch_seconds: settings.created_at_epoch_seconds,
            updated_at_epoch_seconds: settings.created_at_epoch_seconds,
        })
        .await
        .map_err(map_training_load_error)?;

    Ok(())
}

fn epoch_seconds_to_utc_date(epoch_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch_seconds, 0)
        .map(|value| value.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| {
            DateTime::<Utc>::UNIX_EPOCH
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        })
}

fn should_invalidate_llm_cache(previous: &AiAgentsConfig, updated: &AiAgentsConfig) -> bool {
    previous.selected_provider != updated.selected_provider
        || previous.selected_model != updated.selected_model
        || previous.openai_api_key != updated.openai_api_key
        || previous.gemini_api_key != updated.gemini_api_key
        || previous.openrouter_api_key != updated.openrouter_api_key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        external_sync::{
            BoxFuture as SyncBoxFuture, ExternalProvider, ExternalSyncRepositoryError,
            ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
        },
        identity::Clock,
        llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
        training_load::{
            BoxFuture as TrainingLoadBoxFuture, FtpHistoryEntry, FtpHistoryRepository, FtpSource,
            TrainingLoadError, TrainingLoadRecomputeUseCases,
        },
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestClock;

    impl Clock for TestClock {
        fn now_epoch_seconds(&self) -> i64 {
            1_700_000_000
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryUserSettingsRepository {
        settings: Arc<Mutex<Option<UserSettings>>>,
    }

    impl InMemoryUserSettingsRepository {
        fn with_settings(settings: UserSettings) -> Self {
            Self {
                settings: Arc::new(Mutex::new(Some(settings))),
            }
        }
    }

    impl UserSettingsRepository for InMemoryUserSettingsRepository {
        fn find_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move { Ok(settings.lock().unwrap().clone()) })
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
            _user_id: &str,
            ai_agents: AiAgentsConfig,
            updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                let mut guard = settings.lock().unwrap();
                let current = guard
                    .as_mut()
                    .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
                current.ai_agents = ai_agents;
                current.updated_at_epoch_seconds = updated_at_epoch_seconds;
                Ok(())
            })
        }

        fn update_intervals(
            &self,
            _user_id: &str,
            intervals: IntervalsConfig,
            updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                let mut guard = settings.lock().unwrap();
                let current = guard
                    .as_mut()
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
            _user_id: &str,
            cycling: CyclingSettings,
            updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                let mut guard = settings.lock().unwrap();
                let current = guard
                    .as_mut()
                    .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
                current.cycling = cycling;
                current.updated_at_epoch_seconds = updated_at_epoch_seconds;
                Ok(())
            })
        }

        fn update_availability(
            &self,
            _user_id: &str,
            availability: AvailabilitySettings,
            updated_at_epoch_seconds: i64,
        ) -> BoxFuture<Result<(), SettingsError>> {
            let settings = self.settings.clone();
            Box::pin(async move {
                let mut guard = settings.lock().unwrap();
                let current = guard
                    .as_mut()
                    .ok_or_else(|| SettingsError::Repository("settings not found".to_string()))?;
                current.availability = availability;
                current.updated_at_epoch_seconds = updated_at_epoch_seconds;
                Ok(())
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingCacheRepository {
        deleted_users: Arc<Mutex<Vec<String>>>,
    }

    #[derive(Clone, Default)]
    struct RecordingFtpHistoryRepository {
        entries: Arc<Mutex<Vec<FtpHistoryEntry>>>,
    }

    #[derive(Clone, Default)]
    struct RecordingTrainingLoadRecomputeService {
        calls: Arc<Mutex<Vec<(String, String, i64)>>>,
    }

    #[derive(Clone, Default)]
    struct FailingFtpHistoryRepository;

    #[derive(Clone, Default)]
    struct InMemoryProviderPollStateRepository {
        states: Arc<Mutex<Vec<ProviderPollState>>>,
    }

    impl InMemoryProviderPollStateRepository {
        fn stored(&self) -> Vec<ProviderPollState> {
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
                        state.user_id == user_id
                            && state.provider == provider
                            && state.stream == stream
                    })
                    .cloned())
            })
        }
    }

    impl RecordingCacheRepository {
        fn deleted_users(&self) -> Vec<String> {
            self.deleted_users.lock().unwrap().clone()
        }
    }

    impl RecordingFtpHistoryRepository {
        fn stored(&self) -> Vec<FtpHistoryEntry> {
            let mut entries = self.entries.lock().unwrap().clone();
            entries.sort_by(|left, right| left.effective_from_date.cmp(&right.effective_from_date));
            entries
        }
    }

    impl RecordingTrainingLoadRecomputeService {
        fn calls(&self) -> Vec<(String, String, i64)> {
            self.calls.lock().unwrap().clone()
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

        fn upsert(
            &self,
            _cache: LlmContextCache,
        ) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
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

    #[tokio::test]
    async fn find_settings_does_not_create_defaults_when_missing() {
        let repository = InMemoryUserSettingsRepository::default();
        let service = UserSettingsService::new(repository, TestClock);

        let found = service.find_settings("user-1").await.unwrap();

        assert!(found.is_none());
    }

    #[tokio::test]
    async fn update_ai_agents_invalidates_llm_cache_when_provider_config_changes() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::OpenAi);
        settings.ai_agents.selected_model = Some("gpt-4o-mini".to_string());
        settings.ai_agents.openai_api_key = Some("sk-old".to_string());

        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let cache_repository = Arc::new(RecordingCacheRepository::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_llm_context_cache_repository(cache_repository.clone());

        let updated = service
            .update_ai_agents(
                "user-1",
                AiAgentsConfig {
                    selected_provider: Some(crate::domain::llm::LlmProvider::OpenRouter),
                    selected_model: Some("openai/gpt-4o-mini".to_string()),
                    openai_api_key: Some("sk-old".to_string()),
                    openrouter_api_key: Some("or-new".to_string()),
                    ..AiAgentsConfig::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(
            updated.ai_agents.selected_model.as_deref(),
            Some("openai/gpt-4o-mini")
        );
        assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
    }

    #[tokio::test]
    async fn update_ai_agents_skips_llm_cache_invalidation_when_provider_config_is_unchanged() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::Gemini);
        settings.ai_agents.selected_model = Some("gemini-2.5-flash".to_string());
        settings.ai_agents.gemini_api_key = Some("gem-key".to_string());

        let repository = InMemoryUserSettingsRepository::with_settings(settings.clone());
        let cache_repository = Arc::new(RecordingCacheRepository::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_llm_context_cache_repository(cache_repository.clone());

        service
            .update_ai_agents("user-1", settings.ai_agents)
            .await
            .unwrap();

        assert!(cache_repository.deleted_users().is_empty());
    }

    #[tokio::test]
    async fn update_cycling_seeds_initial_ftp_history_and_recomputes_from_settings_created_date() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
        settings.cycling.ftp_watts = Some(280);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let cache_repository = Arc::new(RecordingCacheRepository::default());
        let ftp_history_repository = RecordingFtpHistoryRepository::default();
        let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_llm_context_cache_repository(cache_repository.clone())
            .with_ftp_history_repository(ftp_history_repository.clone())
            .with_training_load_recompute_service(recompute_service.clone());

        service
            .update_cycling(
                "user-1",
                CyclingSettings {
                    ftp_watts: Some(290),
                    ..CyclingSettings::default()
                },
            )
            .await
            .unwrap();

        let history = ftp_history_repository.stored();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].effective_from_date, "2023-11-07");
        assert_eq!(history[0].ftp_watts, 280);
        assert_eq!(history[1].effective_from_date, "2023-11-14");
        assert_eq!(history[1].ftp_watts, 290);
        assert_eq!(history[1].source, FtpSource::Settings);
        assert_eq!(
            recompute_service.calls(),
            vec![(
                "user-1".to_string(),
                "2023-11-07".to_string(),
                1_700_000_000
            )]
        );
        assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
    }

    #[tokio::test]
    async fn update_cycling_skips_ftp_history_when_ftp_is_unchanged() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
        settings.cycling.ftp_watts = Some(280);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let ftp_history_repository = RecordingFtpHistoryRepository::default();
        FtpHistoryRepository::upsert(
            &ftp_history_repository,
            FtpHistoryEntry {
                user_id: "user-1".to_string(),
                effective_from_date: "2023-11-07".to_string(),
                ftp_watts: 280,
                source: FtpSource::Settings,
                created_at_epoch_seconds: 1_699_315_200,
                updated_at_epoch_seconds: 1_699_315_200,
            },
        )
        .await
        .unwrap();
        let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_ftp_history_repository(ftp_history_repository.clone())
            .with_training_load_recompute_service(recompute_service.clone());

        service
            .update_cycling(
                "user-1",
                CyclingSettings {
                    ftp_watts: Some(280),
                    ..CyclingSettings::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(ftp_history_repository.stored().len(), 1);
        assert!(recompute_service.calls().is_empty());
    }

    #[tokio::test]
    async fn update_cycling_keeps_saved_settings_when_ftp_history_write_fails() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
        settings.cycling.ftp_watts = Some(280);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let service = UserSettingsService::new(repository.clone(), TestClock)
            .with_ftp_history_repository(FailingFtpHistoryRepository);

        let updated = service
            .update_cycling(
                "user-1",
                CyclingSettings {
                    ftp_watts: Some(290),
                    ..CyclingSettings::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.cycling.ftp_watts, Some(290));
        assert_eq!(
            repository
                .find_by_user_id("user-1")
                .await
                .unwrap()
                .and_then(|settings| settings.cycling.ftp_watts),
            Some(290)
        );
    }

    #[tokio::test]
    async fn update_cycling_clears_effective_ftp_history_and_recomputes() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
        settings.cycling.ftp_watts = Some(280);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let ftp_history_repository = RecordingFtpHistoryRepository::default();
        FtpHistoryRepository::upsert(
            &ftp_history_repository,
            FtpHistoryEntry {
                user_id: "user-1".to_string(),
                effective_from_date: "2023-11-07".to_string(),
                ftp_watts: 280,
                source: FtpSource::Settings,
                created_at_epoch_seconds: 1_699_315_200,
                updated_at_epoch_seconds: 1_699_315_200,
            },
        )
        .await
        .unwrap();
        let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
        let service = UserSettingsService::new(repository, TestClock)
            .with_ftp_history_repository(ftp_history_repository.clone())
            .with_training_load_recompute_service(recompute_service.clone());

        let updated = service
            .update_cycling(
                "user-1",
                CyclingSettings {
                    ftp_watts: None,
                    ..CyclingSettings::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.cycling.ftp_watts, None);
        let history = ftp_history_repository.stored();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].ftp_watts, 280);
        assert_eq!(history[1].effective_from_date, "2023-11-14");
        assert_eq!(history[1].ftp_watts, 0);
        assert_eq!(
            recompute_service.calls(),
            vec![(
                "user-1".to_string(),
                "2023-11-07".to_string(),
                1_700_000_000,
            )]
        );
    }

    #[tokio::test]
    async fn update_availability_normalizes_inconsistent_configured_flag() {
        let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let service = UserSettingsService::new(repository, TestClock);

        let updated = service
            .update_availability(
                "user-1",
                AvailabilitySettings {
                    configured: true,
                    days: super::super::model::default_availability_days(),
                },
            )
            .await
            .unwrap();

        assert!(!updated.availability.configured);
        assert!(!updated.availability.is_configured());
    }

    #[tokio::test]
    async fn update_intervals_preserves_requested_connection_state_and_seeds_due_poll_states() {
        let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let poll_states = InMemoryProviderPollStateRepository::default();
        let service = UserSettingsService::new(repository, TestClock)
            .with_provider_poll_states(poll_states.clone());

        let updated = service
            .update_intervals(
                "user-1",
                IntervalsConfig {
                    api_key: Some("api-key".to_string()),
                    athlete_id: Some("athlete-1".to_string()),
                    connected: false,
                },
            )
            .await
            .unwrap();

        assert!(!updated.intervals.connected);
        assert_eq!(updated.intervals.api_key.as_deref(), Some("api-key"));
        assert_eq!(updated.intervals.athlete_id.as_deref(), Some("athlete-1"));

        let stored = poll_states.stored();
        assert_eq!(stored.len(), 2);
        assert!(stored
            .iter()
            .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
    }

    #[tokio::test]
    async fn update_intervals_trims_credentials_and_keeps_empty_values_disconnected() {
        let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let poll_states = InMemoryProviderPollStateRepository::default();
        let service = UserSettingsService::new(repository, TestClock)
            .with_provider_poll_states(poll_states.clone());

        let updated = service
            .update_intervals(
                "user-1",
                IntervalsConfig {
                    api_key: Some("  ".to_string()),
                    athlete_id: Some(" athlete-1 ".to_string()),
                    connected: true,
                },
            )
            .await
            .unwrap();

        assert!(!updated.intervals.connected);
        assert_eq!(updated.intervals.api_key, None);
        assert_eq!(updated.intervals.athlete_id.as_deref(), Some("athlete-1"));
        assert!(poll_states
            .stored()
            .iter()
            .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
    }

    #[tokio::test]
    async fn update_intervals_disconnect_disables_existing_poll_states() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.intervals = IntervalsConfig {
            api_key: Some("old-key".to_string()),
            athlete_id: Some("old-athlete".to_string()),
            connected: true,
        };
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let poll_states = InMemoryProviderPollStateRepository::default();
        poll_states
            .upsert(ProviderPollState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Intervals,
                stream: ProviderPollStream::Calendar,
                cursor: Some("2026-05-01".to_string()),
                next_due_at_epoch_seconds: 1_700_000_000,
                last_attempted_at_epoch_seconds: Some(1_699_999_000),
                last_successful_at_epoch_seconds: Some(1_699_999_100),
                last_error: Some("bad auth".to_string()),
                backoff_until_epoch_seconds: Some(1_700_000_300),
            })
            .await
            .unwrap();
        let service = UserSettingsService::new(repository, TestClock)
            .with_provider_poll_states(poll_states.clone());

        let updated = service
            .update_intervals(
                "user-1",
                IntervalsConfig {
                    api_key: None,
                    athlete_id: None,
                    connected: false,
                },
            )
            .await
            .unwrap();

        assert!(!updated.intervals.connected);
        let stored = poll_states.stored();
        assert_eq!(stored.len(), 2);
        assert!(stored
            .iter()
            .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
        assert!(stored.iter().all(|state| state.cursor.is_none()));
        assert!(stored
            .iter()
            .all(|state| state.backoff_until_epoch_seconds.is_none()));
        assert!(stored.iter().all(|state| state.last_error.is_none()));
    }

    #[tokio::test]
    async fn update_intervals_credential_change_resets_cursor_for_fresh_backfill() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.intervals = IntervalsConfig {
            api_key: Some("old-key".to_string()),
            athlete_id: Some("old-athlete".to_string()),
            connected: true,
        };
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let poll_states = InMemoryProviderPollStateRepository::default();
        poll_states
            .upsert(ProviderPollState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Intervals,
                stream: ProviderPollStream::Calendar,
                cursor: Some("2099-01-01".to_string()),
                next_due_at_epoch_seconds: 1_700_099_999,
                last_attempted_at_epoch_seconds: Some(1_699_999_000),
                last_successful_at_epoch_seconds: Some(1_699_999_100),
                last_error: Some("stale".to_string()),
                backoff_until_epoch_seconds: Some(1_700_000_300),
            })
            .await
            .unwrap();
        let service = UserSettingsService::new(repository, TestClock)
            .with_provider_poll_states(poll_states.clone());

        service
            .update_intervals(
                "user-1",
                IntervalsConfig {
                    api_key: Some("new-key".to_string()),
                    athlete_id: Some("new-athlete".to_string()),
                    connected: false,
                },
            )
            .await
            .unwrap();

        let stored = poll_states.stored();
        assert!(stored
            .iter()
            .all(|state| state.next_due_at_epoch_seconds == i64::MAX));
        assert!(stored.iter().all(|state| state.cursor.is_none()));
        assert!(stored
            .iter()
            .all(|state| state.backoff_until_epoch_seconds.is_none()));
        assert!(stored.iter().all(|state| state.last_error.is_none()));
    }

    #[tokio::test]
    async fn update_intervals_without_credential_change_keeps_future_poll_schedule() {
        let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
        settings.intervals = IntervalsConfig {
            api_key: Some("same-key".to_string()),
            athlete_id: Some("same-athlete".to_string()),
            connected: true,
        };
        let repository = InMemoryUserSettingsRepository::with_settings(settings);
        let poll_states = InMemoryProviderPollStateRepository::default();
        poll_states
            .upsert(ProviderPollState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Intervals,
                stream: ProviderPollStream::Calendar,
                cursor: Some("2026-05-01".to_string()),
                next_due_at_epoch_seconds: 1_700_099_999,
                last_attempted_at_epoch_seconds: Some(1_699_999_000),
                last_successful_at_epoch_seconds: Some(1_699_999_100),
                last_error: Some("transient".to_string()),
                backoff_until_epoch_seconds: Some(1_700_100_100),
            })
            .await
            .unwrap();
        let service = UserSettingsService::new(repository, TestClock)
            .with_provider_poll_states(poll_states.clone());

        service
            .update_intervals(
                "user-1",
                IntervalsConfig {
                    api_key: Some("same-key".to_string()),
                    athlete_id: Some("same-athlete".to_string()),
                    connected: true,
                },
            )
            .await
            .unwrap();

        let stored = poll_states.stored();
        assert!(stored.iter().any(|state| {
            state.stream == ProviderPollStream::Calendar
                && state.next_due_at_epoch_seconds == 1_700_099_999
                && state.backoff_until_epoch_seconds == Some(1_700_100_100)
                && state.last_error.as_deref() == Some("transient")
        }));
        assert!(stored.iter().any(|state| {
            state.stream == ProviderPollStream::CompletedWorkouts
                && state.next_due_at_epoch_seconds == 1_700_000_000
                && state.backoff_until_epoch_seconds.is_none()
                && state.last_error.is_none()
        }));
    }
}
