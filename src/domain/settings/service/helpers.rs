use chrono::{DateTime, Utc};

use super::{AiAgentsConfig, IntervalsConfig, SettingsError, UserSettings};
use crate::domain::external_sync::{
    ExternalProvider, ExternalSyncRepositoryError, ProviderPollState, ProviderPollStateRepository,
    ProviderPollStream,
};
use crate::domain::training_load::{FtpHistoryEntry, FtpHistoryRepository, TrainingLoadError};

pub(super) trait FtpHistoryWritePort: Send + Sync {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_load::BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>>;

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> crate::domain::training_load::BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>>;
}

impl<Repository> FtpHistoryWritePort for Repository
where
    Repository: FtpHistoryRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::training_load::BoxFuture<Result<Vec<FtpHistoryEntry>, TrainingLoadError>>
    {
        FtpHistoryRepository::list_by_user_id(self, user_id)
    }

    fn upsert(
        &self,
        entry: FtpHistoryEntry,
    ) -> crate::domain::training_load::BoxFuture<Result<FtpHistoryEntry, TrainingLoadError>> {
        FtpHistoryRepository::upsert(self, entry)
    }
}

pub(super) fn normalize_intervals_config(mut intervals: IntervalsConfig) -> IntervalsConfig {
    intervals.api_key = normalize_optional_non_empty(intervals.api_key);
    intervals.athlete_id = normalize_optional_non_empty(intervals.athlete_id);
    intervals.connected =
        intervals.connected && intervals.api_key.is_some() && intervals.athlete_id.is_some();
    intervals
}

fn normalize_optional_non_empty(value: Option<String>) -> Option<String> {
    let normalized = value?.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) async fn sync_poll_states_after_intervals_update<PollStates>(
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

    let existing = poll_states
        .find_by_provider_and_stream(
            user_id,
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
        )
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
            } else {
                state
            }
        }
        None => ProviderPollState::new(
            user_id.to_string(),
            ExternalProvider::Intervals,
            ProviderPollStream::CompletedWorkouts,
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

    park_existing_intervals_calendar_poll_state(poll_states, user_id)
        .await
        .map_err(map_poll_state_error)?;

    Ok(())
}

async fn park_existing_intervals_calendar_poll_state<PollStates>(
    poll_states: &PollStates,
    user_id: &str,
) -> Result<(), ExternalSyncRepositoryError>
where
    PollStates: ProviderPollStateRepository,
{
    let existing = poll_states
        .find_by_provider_and_stream(
            user_id,
            ExternalProvider::Intervals,
            ProviderPollStream::Calendar,
        )
        .await?;

    if let Some(state) = existing {
        poll_states
            .upsert(ProviderPollState {
                next_due_at_epoch_seconds: i64::MAX,
                cursor: None,
                backoff_until_epoch_seconds: None,
                last_error: None,
                ..state
            })
            .await?;
    }

    Ok(())
}

pub(super) fn map_poll_state_error(error: ExternalSyncRepositoryError) -> SettingsError {
    SettingsError::Repository(error.to_string())
}

fn map_training_load_error(error: TrainingLoadError) -> SettingsError {
    SettingsError::Repository(error.to_string())
}

pub(super) async fn seed_initial_ftp_history_if_needed(
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

pub(super) fn epoch_seconds_to_utc_date(epoch_seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch_seconds, 0)
        .map(|value| value.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| {
            DateTime::<Utc>::UNIX_EPOCH
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        })
}

pub(super) fn should_invalidate_llm_cache(
    previous: &AiAgentsConfig,
    updated: &AiAgentsConfig,
) -> bool {
    previous.selected_provider != updated.selected_provider
        || previous.selected_model != updated.selected_model
        || previous.openai_api_key != updated.openai_api_key
        || previous.gemini_api_key != updated.gemini_api_key
        || previous.openrouter_api_key != updated.openrouter_api_key
        || previous.deepseek_api_key != updated.deepseek_api_key
}
