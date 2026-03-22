use std::sync::Arc;

use crate::domain::{
    intervals::{
        BoxFuture, IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
    },
    settings::UserSettingsUseCases,
};

#[derive(Clone)]
pub struct SettingsIntervalsProvider {
    settings_service: Arc<dyn UserSettingsUseCases>,
}

impl SettingsIntervalsProvider {
    pub fn new(settings_service: Arc<dyn UserSettingsUseCases>) -> Self {
        Self { settings_service }
    }
}

impl IntervalsSettingsPort for SettingsIntervalsProvider {
    fn get_credentials(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
        let settings_service = self.settings_service.clone();
        let user_id = user_id.to_string();

        Box::pin(async move {
            let settings = settings_service
                .get_settings(&user_id)
                .await
                .map_err(|_| {
                    IntervalsError::Internal(
                        "Failed to load Intervals.icu credentials".to_string(),
                    )
                })?;

            let api_key = settings
                .intervals
                .api_key
                .filter(|value| !value.trim().is_empty())
                .ok_or(IntervalsError::CredentialsNotConfigured)?;

            let athlete_id = settings
                .intervals
                .athlete_id
                .filter(|value| !value.trim().is_empty())
                .ok_or(IntervalsError::CredentialsNotConfigured)?;

            Ok(IntervalsCredentials { api_key, athlete_id })
        })
    }
}
