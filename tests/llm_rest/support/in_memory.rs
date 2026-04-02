use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
    settings::{
        AiAgentsConfig, AnalysisOptions, BoxFuture as SettingsBoxFuture, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsRepository,
    },
    workout_summary::{
        BoxFuture as WorkoutBoxFuture, CoachReplyClaimResult, CoachReplyOperation,
        CoachReplyOperationRepository, CoachReplyOperationStatus, ConversationMessage,
        WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
    },
};

type SummaryKey = (String, String);
type ReplyOperationKey = (String, String, String);

#[derive(Clone, Default)]
pub(crate) struct InMemoryUserSettingsRepository {
    settings: Arc<Mutex<BTreeMap<String, UserSettings>>>,
}

impl InMemoryUserSettingsRepository {
    pub(crate) fn seed(&self, settings: UserSettings) {
        self.settings
            .lock()
            .unwrap()
            .insert(settings.user_id.clone(), settings);
    }
}

impl UserSettingsRepository for InMemoryUserSettingsRepository {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> SettingsBoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(settings.lock().unwrap().get(&user_id).cloned()) })
    }

    fn upsert(
        &self,
        settings: UserSettings,
    ) -> SettingsBoxFuture<Result<UserSettings, SettingsError>> {
        let store = self.settings.clone();
        Box::pin(async move {
            store
                .lock()
                .unwrap()
                .insert(settings.user_id.clone(), settings.clone());
            Ok(settings)
        })
    }

    fn update_ai_agents(
        &self,
        user_id: &str,
        ai_agents: AiAgentsConfig,
        updated_at_epoch_seconds: i64,
    ) -> SettingsBoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = settings.lock().unwrap();
            let Some(existing) = settings.get_mut(&user_id) else {
                return Err(SettingsError::Repository("settings not found".to_string()));
            };
            existing.ai_agents = ai_agents;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_intervals(
        &self,
        user_id: &str,
        intervals: IntervalsConfig,
        updated_at_epoch_seconds: i64,
    ) -> SettingsBoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = settings.lock().unwrap();
            let Some(existing) = settings.get_mut(&user_id) else {
                return Err(SettingsError::Repository("settings not found".to_string()));
            };
            existing.intervals = intervals;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_options(
        &self,
        user_id: &str,
        options: AnalysisOptions,
        updated_at_epoch_seconds: i64,
    ) -> SettingsBoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = settings.lock().unwrap();
            let Some(existing) = settings.get_mut(&user_id) else {
                return Err(SettingsError::Repository("settings not found".to_string()));
            };
            existing.options = options;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn update_cycling(
        &self,
        user_id: &str,
        cycling: CyclingSettings,
        updated_at_epoch_seconds: i64,
    ) -> SettingsBoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = settings.lock().unwrap();
            let Some(existing) = settings.get_mut(&user_id) else {
                return Err(SettingsError::Repository("settings not found".to_string()));
            };
            existing.cycling = cycling;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryLlmContextCacheRepository {
    caches: Arc<Mutex<Vec<LlmContextCache>>>,
}

impl LlmContextCacheRepository for InMemoryLlmContextCacheRepository {
    fn find_reusable(
        &self,
        user_id: &str,
        provider: &aiwattcoach::domain::llm::LlmProvider,
        model: &str,
        scope_key: &str,
        context_hash: &str,
        now_epoch_seconds: i64,
    ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        let caches = self.caches.clone();
        let user_id = user_id.to_string();
        let provider = provider.clone();
        let model = model.to_string();
        let scope_key = scope_key.to_string();
        let context_hash = context_hash.to_string();
        Box::pin(async move {
            Ok(caches
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|cache| {
                    cache.user_id == user_id
                        && cache.provider == provider
                        && cache.model == model
                        && cache.scope_key == scope_key
                        && cache.context_hash == context_hash
                        && cache
                            .expires_at_epoch_seconds
                            .is_none_or(|expires_at| expires_at > now_epoch_seconds)
                })
                .cloned())
        })
    }

    fn upsert(&self, cache: LlmContextCache) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
        let caches = self.caches.clone();
        Box::pin(async move {
            let mut caches = caches.lock().unwrap();
            if let Some(existing) = caches.iter_mut().find(|existing| {
                existing.user_id == cache.user_id
                    && existing.provider == cache.provider
                    && existing.model == cache.model
                    && existing.scope_key == cache.scope_key
                    && existing.context_hash == cache.context_hash
            }) {
                *existing = cache.clone();
            } else {
                caches.push(cache.clone());
            }
            Ok(cache)
        })
    }

    fn delete_by_user_id(&self, user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
        let caches = self.caches.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            caches
                .lock()
                .unwrap()
                .retain(|cache| cache.user_id != user_id);
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<BTreeMap<SummaryKey, WorkoutSummary>>>,
}

impl InMemoryWorkoutSummaryRepository {
    pub(crate) fn seed(&self, summary: WorkoutSummary) {
        self.summaries.lock().unwrap().insert(
            (summary.user_id.clone(), summary.workout_id.clone()),
            summary,
        );
    }
}

impl WorkoutSummaryRepository for InMemoryWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move { Ok(summaries.lock().unwrap().get(&key).cloned()) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> WorkoutBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter_map(|workout_id| {
                    summaries
                        .lock()
                        .unwrap()
                        .get(&(user_id.clone(), workout_id))
                        .cloned()
                })
                .collect())
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> WorkoutBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let key = (summary.user_id.clone(), summary.workout_id.clone());
            if summaries.contains_key(&key) {
                return Err(WorkoutSummaryError::AlreadyExists);
            }
            summaries.insert(key, summary.clone());
            Ok(summary)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn append_message(
        &self,
        user_id: &str,
        workout_id: &str,
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary
                .messages
                .iter()
                .any(|existing| existing.id == message.id)
            {
                return Ok(());
            }
            summary.messages.push(message);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn set_saved_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: Option<i64>,
        updated_at_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&key) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.saved_at_epoch_seconds = saved_at_epoch_seconds;
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        let message_id = message_id.to_string();
        Box::pin(async move {
            Ok(summaries.lock().unwrap().get(&key).and_then(|summary| {
                summary
                    .messages
                    .iter()
                    .find(|message| message.id == message_id)
                    .cloned()
            }))
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<BTreeMap<ReplyOperationKey, CoachReplyOperation>>>,
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> WorkoutBoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        Box::pin(async move { Ok(operations.lock().unwrap().get(&key).cloned()) })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> WorkoutBoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        Box::pin(async move {
            let key = (
                operation.user_id.clone(),
                operation.workout_id.clone(),
                operation.user_message_id.clone(),
            );
            let mut operations = operations.lock().unwrap();
            if let Some(existing) = operations.get(&key).cloned() {
                let reclaimable = match existing.status {
                    CoachReplyOperationStatus::Pending => {
                        existing.is_stale(stale_before_epoch_seconds)
                    }
                    CoachReplyOperationStatus::Failed => true,
                    CoachReplyOperationStatus::Completed => false,
                };
                if reclaimable {
                    let fallback_coach_message_id =
                        operation.coach_message_id.clone().ok_or_else(|| {
                            WorkoutSummaryError::Repository(
                                "pending coach reply operation missing reserved coach message id"
                                    .to_string(),
                            )
                        })?;
                    let reclaimed = existing.reclaim(
                        fallback_coach_message_id,
                        operation.last_attempt_at_epoch_seconds,
                    );
                    operations.insert(key, reclaimed.clone());
                    return Ok(CoachReplyClaimResult::Claimed(reclaimed));
                }
                return Ok(CoachReplyClaimResult::Existing(existing));
            }

            operations.insert(key, operation.clone());
            Ok(CoachReplyClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> WorkoutBoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        Box::pin(async move {
            operations.lock().unwrap().insert(
                (
                    operation.user_id.clone(),
                    operation.workout_id.clone(),
                    operation.user_message_id.clone(),
                ),
                operation.clone(),
            );
            Ok(operation)
        })
    }
}

pub(crate) fn sample_user_settings() -> UserSettings {
    UserSettings::new_defaults("user-1".to_string(), 1_700_000_000)
}

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

pub(crate) fn ai_config(
    provider: aiwattcoach::domain::llm::LlmProvider,
    model: &str,
    api_key: &str,
) -> AiAgentsConfig {
    let mut config = AiAgentsConfig {
        selected_provider: Some(provider.clone()),
        selected_model: Some(model.to_string()),
        ..AiAgentsConfig::default()
    };
    match provider {
        aiwattcoach::domain::llm::LlmProvider::OpenAi => {
            config.openai_api_key = Some(api_key.to_string())
        }
        aiwattcoach::domain::llm::LlmProvider::Gemini => {
            config.gemini_api_key = Some(api_key.to_string())
        }
        aiwattcoach::domain::llm::LlmProvider::OpenRouter => {
            config.openrouter_api_key = Some(api_key.to_string())
        }
    }
    config
}
