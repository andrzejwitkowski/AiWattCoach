use std::sync::Arc;

use aiwattcoach::{
    adapters::intervals_icu::settings_adapter::SettingsIntervalsProvider,
    domain::{
        intervals::{IntervalsError, IntervalsSettingsPort},
        settings::IntervalsConfig,
    },
};

use crate::support::FakeSettingsUseCases;

#[tokio::test]
async fn settings_provider_returns_credentials_from_user_settings() {
    let settings_service = Arc::new(FakeSettingsUseCases::with_intervals(IntervalsConfig {
        api_key: Some("key-123".to_string()),
        athlete_id: Some("athlete-99".to_string()),
        connected: true,
    }));
    let provider = SettingsIntervalsProvider::new(settings_service);

    let credentials = provider.get_credentials("user-1").await.unwrap();

    assert_eq!(credentials.api_key, "key-123");
    assert_eq!(credentials.athlete_id, "athlete-99");
}

#[tokio::test]
async fn settings_provider_rejects_missing_credentials() {
    let settings_service = Arc::new(FakeSettingsUseCases::with_intervals(IntervalsConfig {
        api_key: None,
        athlete_id: Some("athlete-99".to_string()),
        connected: false,
    }));
    let provider = SettingsIntervalsProvider::new(settings_service);

    let result = provider.get_credentials("user-1").await;

    assert_eq!(result, Err(IntervalsError::CredentialsNotConfigured));
}
