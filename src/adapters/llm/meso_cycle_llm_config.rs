use std::sync::Arc;

use super::resolve_settings_llm_config::resolve_llm_config;
use crate::domain::{
    llm::{LlmError, LlmProviderConfig},
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

            resolve_llm_config(
                &settings.ai_agents,
                settings.ai_agents.meso_cycle_provider.clone(),
                settings.ai_agents.meso_cycle_model.clone(),
            )
            .map_err(map_llm_error)
        })
    }
}

fn map_llm_error(error: LlmError) -> MesoCycleError {
    match error {
        LlmError::ProviderNotConfigured | LlmError::ModelNotConfigured => {
            MesoCycleError::NotConfigured
        }
        LlmError::CredentialsNotConfigured => MesoCycleError::NotConfigured,
        other => MesoCycleError::Repository(other.to_string()),
    }
}
