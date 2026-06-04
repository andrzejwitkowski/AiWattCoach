use std::sync::Arc;

use super::super::*;
use super::support::{InMemoryUserSettingsRepository, RecordingCacheRepository, TestClock};

#[tokio::test]
async fn update_ai_agents_invalidates_llm_cache_when_provider_config_changes() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::OpenAi);
    settings.ai_agents.selected_model = Some("gpt-4o-mini".to_string());
    settings.ai_agents.openai_api_key = Some("sk-old".to_string());

    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let cache_repository = Arc::new(RecordingCacheRepository::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_llm_context_cache_repository(cache_repository.clone());

    let updated = service
        .update_ai_agents(
            "user-1",
            AiAgentsConfig {
                selected_provider: Some(crate::domain::llm::LlmProvider::OpenRouter),
                selected_model: Some("openai/gpt-4o-mini".to_string()),
                openai_api_key: Some("sk-old".to_string()),
                openrouter_api_key: Some("or-new".to_string()),
                ..AiAgentsConfig::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(
        updated.ai_agents.selected_model.as_deref(),
        Some("openai/gpt-4o-mini")
    );
    assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
}

#[tokio::test]
async fn update_ai_agents_skips_llm_cache_invalidation_when_provider_config_is_unchanged() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::Gemini);
    settings.ai_agents.selected_model = Some("gemini-2.5-flash".to_string());
    settings.ai_agents.gemini_api_key = Some("gem-key".to_string());

    let repository = InMemoryUserSettingsRepository::with_settings(settings.clone());
    let cache_repository = Arc::new(RecordingCacheRepository::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_llm_context_cache_repository(cache_repository.clone());

    service
        .update_ai_agents("user-1", settings.ai_agents)
        .await
        .unwrap();

    assert!(cache_repository.deleted_users().is_empty());
}

#[tokio::test]
async fn update_ai_agents_invalidates_llm_cache_when_deepseek_api_key_changes() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    settings.ai_agents.selected_provider = Some(crate::domain::llm::LlmProvider::DeepSeek);
    settings.ai_agents.selected_model = Some("deepseek-v4-flash".to_string());
    settings.ai_agents.deepseek_api_key = Some("sk-old".to_string());

    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let cache_repository = Arc::new(RecordingCacheRepository::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_llm_context_cache_repository(cache_repository.clone());

    service
        .update_ai_agents(
            "user-1",
            AiAgentsConfig {
                selected_provider: Some(crate::domain::llm::LlmProvider::DeepSeek),
                selected_model: Some("deepseek-v4-flash".to_string()),
                deepseek_api_key: Some("sk-new".to_string()),
                ..AiAgentsConfig::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
}
