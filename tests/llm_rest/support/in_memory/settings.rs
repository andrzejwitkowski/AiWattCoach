use super::*;

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

pub(crate) fn sample_user_settings() -> UserSettings {
    UserSettings::new_defaults("user-1".to_string(), 1_700_000_000)
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
