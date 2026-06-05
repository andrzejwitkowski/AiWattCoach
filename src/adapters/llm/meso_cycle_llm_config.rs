use std::sync::Arc;

use crate::domain::{
    llm::{LlmProvider, LlmProviderConfig},
    meso_cycle::{BoxFuture, MesoCycleError, MesoCycleLlmConfigPort},
    settings::UserSettingsUseCases,
};

#[derive(Clone)]
pub struct MesoCycleLlmConfigProvider {
    settings_service: Arc<dyn UserSettingsUseCases>,
}

impl MesoCycleLlmConfigProvider {
    pub fn new(settings_service: Arc<dyn UserSettingsUseCases>) -> Self {
        Self { settings_service }
    }
}

impl MesoCycleLlmConfigPort for MesoCycleLlmConfigProvider {
    fn get_meso_cycle_config(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<LlmProviderConfig, MesoCycleError>> {
        let settings_service = self.settings_service.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let settings = settings_service
                .get_settings(&user_id)
                .await
                .map_err(|error| MesoCycleError::Repository(error.to_string()))?;

            let provider = settings
                .ai_agents
                .meso_cycle_provider
                .or(settings.ai_agents.selected_provider)
                .ok_or(MesoCycleError::NotConfigured)?;
            let model = settings
                .ai_agents
                .meso_cycle_model
                .or(settings.ai_agents.selected_model)
                .filter(|value| !value.trim().is_empty())
                .ok_or(MesoCycleError::NotConfigured)?;

            let api_key = match provider {
                LlmProvider::OpenAi => settings.ai_agents.openai_api_key,
                LlmProvider::Gemini => settings.ai_agents.gemini_api_key,
                LlmProvider::OpenRouter => settings.ai_agents.openrouter_api_key,
                LlmProvider::DeepSeek => settings.ai_agents.deepseek_api_key,
            }
            .filter(|value| !value.trim().is_empty())
            .ok_or(MesoCycleError::NotConfigured)?;

            Ok(LlmProviderConfig {
                provider,
                model,
                api_key,
            })
        })
    }
}
