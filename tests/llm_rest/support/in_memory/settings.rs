use super::*;

use aiwattcoach::domain::settings::{WahooUserIdBackfillCandidate, Weekday};

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

    fn find_by_wahoo_user_id(
        &self,
        wahoo_user_id: i64,
    ) -> SettingsBoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move {
            Ok(settings
                .lock()
                .unwrap()
                .values()
                .find(|settings| settings.wahoo.user_id == Some(wahoo_user_id))
                .cloned())
        })
    }

    fn list_wahoo_user_id_backfill_candidates(
        &self,
    ) -> SettingsBoxFuture<Result<Vec<WahooUserIdBackfillCandidate>, SettingsError>> {
        let settings = self.settings.clone();
        Box::pin(async move {
            Ok(settings
                .lock()
                .unwrap()
                .values()
                .filter(|settings| {
                    settings.wahoo.connected
                        && settings.wahoo.user_id.is_none()
                        && settings
                            .wahoo
                            .refresh_token
                            .as_deref()
                            .is_some_and(|value| !value.trim().is_empty())
                })
                .cloned()
                .map(|settings| WahooUserIdBackfillCandidate {
                    user_id: settings.user_id,
                    wahoo: settings.wahoo,
                })
                .collect())
        })
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

    fn update_availability(
        &self,
        user_id: &str,
        availability: AvailabilitySettings,
        updated_at_epoch_seconds: i64,
    ) -> SettingsBoxFuture<Result<(), SettingsError>> {
        let settings = self.settings.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut settings = settings.lock().unwrap();
            let Some(existing) = settings.get_mut(&user_id) else {
                return Err(SettingsError::Repository("settings not found".to_string()));
            };
            existing.availability = availability;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }
}

pub(crate) fn sample_user_settings() -> UserSettings {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_700_000_000);
    settings.availability = AvailabilitySettings {
        configured: true,
        days: vec![
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: true,
                max_duration_minutes: Some(120),
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    };
    settings
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
        aiwattcoach::domain::llm::LlmProvider::DeepSeek => {
            config.deepseek_api_key = Some(api_key.to_string())
        }
        aiwattcoach::domain::llm::LlmProvider::Zai => {
            config.zai_api_key = Some(api_key.to_string())
        }
        aiwattcoach::domain::llm::LlmProvider::OpenAiCompatible => {
            config.openai_compatible_api_key = Some(api_key.to_string());
            config.openai_compatible_base_url = Some("http://127.0.0.1:8080/v1".to_string());
        }
    }
    config
}
