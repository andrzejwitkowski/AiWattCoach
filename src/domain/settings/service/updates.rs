use super::{
    helpers::{
        epoch_seconds_to_utc_date, normalize_intervals_config, seed_initial_ftp_history_if_needed,
        should_invalidate_llm_cache, sync_poll_states_after_intervals_update,
    },
    AiAgentsConfig, AnalysisOptions, AvailabilitySettings, CyclingSettings, IntervalsConfig,
    SettingsError, UserSettings, UserSettingsRepository, UserSettingsService,
};
use crate::domain::external_sync::ProviderPollStateRepository;
use crate::domain::identity::Clock;
use crate::domain::settings::validation;
use crate::domain::training_load::{FtpHistoryEntry, FtpSource};

impl<Repo, Time, PollStates> UserSettingsService<Repo, Time, PollStates>
where
    Repo: UserSettingsRepository,
    Time: Clock,
    PollStates: ProviderPollStateRepository,
{
    pub(super) async fn update_ai_agents_impl(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
    ) -> Result<UserSettings, SettingsError> {
        let previous = self.get_or_create(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        self.repository
            .update_ai_agents(user_id, ai_agents, now)
            .await?;
        let updated = self.load_updated_settings(user_id).await?;

        if should_invalidate_llm_cache(&previous.ai_agents, &updated.ai_agents) {
            self.invalidate_llm_context_cache(user_id).await;
        }

        Ok(updated)
    }

    pub(super) async fn update_intervals_impl(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
    ) -> Result<UserSettings, SettingsError> {
        let previous = self.get_or_create(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        let intervals = normalize_intervals_config(intervals);

        self.repository
            .update_intervals(user_id, intervals.clone(), now)
            .await?;

        if let Err(error) = sync_poll_states_after_intervals_update(
            &self.poll_states,
            user_id,
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

        self.load_updated_settings(user_id).await
    }

    pub(super) async fn update_options_impl(
        &self,
        user_id: &str,
        options: AnalysisOptions,
    ) -> Result<UserSettings, SettingsError> {
        self.get_or_create(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        self.repository
            .update_options(user_id, options, now)
            .await?;
        self.load_updated_settings(user_id).await
    }

    pub(super) async fn update_cycling_impl(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
    ) -> Result<UserSettings, SettingsError> {
        let previous = self.get_or_create(user_id).await?;
        let now = self.clock.now_epoch_seconds();
        let recompute_from_date = epoch_seconds_to_utc_date(previous.created_at_epoch_seconds);
        let ftp_changed = previous.cycling.ftp_watts != cycling.ftp_watts;

        self.repository
            .update_cycling(user_id, cycling.clone(), now)
            .await?;

        let updated = self.load_updated_settings(user_id).await?;

        if ftp_changed {
            self.update_ftp_history(user_id, now, &previous, &updated)
                .await;
            self.recompute_training_load(user_id, &recompute_from_date, now)
                .await;
            self.invalidate_llm_context_cache(user_id).await;
        }

        Ok(updated)
    }

    pub(super) async fn update_availability_impl(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
    ) -> Result<UserSettings, SettingsError> {
        self.get_or_create(user_id).await?;
        let availability = validation::validate_availability(availability)?;
        let now = self.clock.now_epoch_seconds();
        self.repository
            .update_availability(user_id, availability, now)
            .await?;
        self.load_updated_settings(user_id).await
    }

    async fn load_updated_settings(&self, user_id: &str) -> Result<UserSettings, SettingsError> {
        self.repository
            .find_by_user_id(user_id)
            .await?
            .ok_or_else(|| {
                SettingsError::Repository("settings disappeared after update".to_string())
            })
    }

    async fn invalidate_llm_context_cache(&self, user_id: &str) {
        if let Some(repository) = &self.llm_context_cache_repository {
            if let Err(error) = repository.delete_by_user_id(user_id).await {
                tracing::warn!(
                    user_id = %user_id,
                    error = %error,
                    "failed to invalidate llm context cache after settings update"
                );
            }
        }
    }

    async fn update_ftp_history(
        &self,
        user_id: &str,
        now: i64,
        previous: &UserSettings,
        updated: &UserSettings,
    ) {
        let Some(repository) = &self.ftp_history_repository else {
            return;
        };

        if let Err(error) = seed_initial_ftp_history_if_needed(repository.as_ref(), previous).await
        {
            tracing::warn!(
                user_id = %user_id,
                error = %error,
                "cycling settings were saved but initial ftp history seed failed"
            );
        }

        let Some(history_ftp_watts) = updated.cycling.ftp_watts.map(|ftp| ftp as i32) else {
            return;
        };

        if let Err(error) = repository
            .upsert(FtpHistoryEntry {
                user_id: user_id.to_string(),
                effective_from_date: epoch_seconds_to_utc_date(now),
                ftp_watts: history_ftp_watts,
                source: FtpSource::Settings,
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

    async fn recompute_training_load(&self, user_id: &str, recompute_from_date: &str, now: i64) {
        let Some(recompute_service) = &self.training_load_recompute_service else {
            return;
        };

        if let Err(error) = recompute_service
            .recompute_from(user_id, recompute_from_date, now)
            .await
        {
            tracing::warn!(
                user_id = %user_id,
                error = %error,
                "cycling settings were saved but training load recompute failed"
            );
        }
    }
}
