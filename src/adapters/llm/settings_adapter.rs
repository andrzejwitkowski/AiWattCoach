use std::sync::Arc;

use crate::domain::{
    llm::{BoxFuture, LlmError, LlmProviderConfig, UserLlmConfigProvider},
    settings::UserSettingsUseCases,
};

#[derive(Clone)]
pub struct SettingsLlmConfigProvider {
    settings_service: Arc<dyn UserSettingsUseCases>,
}

impl SettingsLlmConfigProvider {
    pub fn new(settings_service: Arc<dyn UserSettingsUseCases>) -> Self {
        Self { settings_service }
    }
}

impl UserLlmConfigProvider for SettingsLlmConfigProvider {
    fn get_config(&self, user_id: &str) -> BoxFuture<Result<LlmProviderConfig, LlmError>> {
        let settings_service = self.settings_service.clone();
        let user_id = user_id.to_string();

        Box::pin(async move {
            let settings = settings_service
                .get_settings(&user_id)
                .await
                .map_err(|error| LlmError::Internal(error.to_string()))?;

            let provider = settings
                .ai_agents
                .selected_provider
                .ok_or(LlmError::ProviderNotConfigured)?;
            let model = settings
                .ai_agents
                .selected_model
                .filter(|value| !value.trim().is_empty())
                .ok_or(LlmError::ModelNotConfigured)?;

            let api_key = match provider {
                crate::domain::llm::LlmProvider::OpenAi => settings.ai_agents.openai_api_key,
                crate::domain::llm::LlmProvider::Gemini => settings.ai_agents.gemini_api_key,
                crate::domain::llm::LlmProvider::OpenRouter => {
                    settings.ai_agents.openrouter_api_key
                }
            }
            .filter(|value| !value.trim().is_empty())
            .ok_or(LlmError::CredentialsNotConfigured)?;

            Ok(LlmProviderConfig {
                provider,
                model,
                api_key,
            })
        })
    }
}
