use std::sync::Arc;

use super::resolve_settings_llm_config::resolve_llm_config;
use crate::domain::{
    settings::{AiAgentsConfig, UserSettingsUseCases},
    training_plan::{
        BoxFuture as TrainingPlanBoxFuture, TrainingPlanError, WorkoutPlanningLlmConfigPort,
    },
    workout_summary::{
        BoxFuture as WorkoutSummaryBoxFuture, WorkoutChatLlmConfigPort, WorkoutSummaryError,
    },
};

#[derive(Clone)]
pub struct WorkoutLlmConfigProvider {
    settings_service: Arc<dyn UserSettingsUseCases>,
}

impl WorkoutLlmConfigProvider {
    pub fn new(settings_service: Arc<dyn UserSettingsUseCases>) -> Self {
        Self { settings_service }
    }
}

impl WorkoutChatLlmConfigPort for WorkoutLlmConfigProvider {
    fn get_workout_chat_config(
        &self,
        user_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<crate::domain::llm::LlmProviderConfig, WorkoutSummaryError>>
    {
        let settings_service = self.settings_service.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let ai_agents = load_ai_agents(&settings_service, &user_id)
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            resolve_llm_config(
                &ai_agents,
                ai_agents.workout_chat_provider.clone(),
                ai_agents.workout_chat_model.clone(),
            )
            .map_err(WorkoutSummaryError::Llm)
        })
    }
}

impl WorkoutPlanningLlmConfigPort for WorkoutLlmConfigProvider {
    fn get_workout_planning_config(
        &self,
        user_id: &str,
    ) -> TrainingPlanBoxFuture<Result<crate::domain::llm::LlmProviderConfig, TrainingPlanError>>
    {
        let settings_service = self.settings_service.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let ai_agents = load_ai_agents(&settings_service, &user_id)
                .await
                .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
            resolve_llm_config(
                &ai_agents,
                ai_agents.workout_planning_provider.clone(),
                ai_agents.workout_planning_model.clone(),
            )
            .map_err(|error| TrainingPlanError::Unavailable(error.to_string()))
        })
    }
}

async fn load_ai_agents(
    settings_service: &Arc<dyn UserSettingsUseCases>,
    user_id: &str,
) -> Result<AiAgentsConfig, crate::domain::settings::SettingsError> {
    settings_service
        .get_settings(user_id)
        .await
        .map(|settings| settings.ai_agents)
}
