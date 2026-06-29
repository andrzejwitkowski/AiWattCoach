use std::sync::Arc;

use super::resolve_settings_llm_config::resolve_llm_config;
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

            resolve_llm_config(&settings.ai_agents, None, None)
        })
    }
}
