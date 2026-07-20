use crate::domain::{
    llm::{normalize_openai_compatible_base_url, LlmError, LlmProvider, LlmProviderConfig},
    settings::AiAgentsConfig,
};

pub fn resolve_llm_config(
    ai_agents: &AiAgentsConfig,
    override_provider: Option<LlmProvider>,
    override_model: Option<String>,
) -> Result<LlmProviderConfig, LlmError> {
    let provider = override_provider
        .or(ai_agents.selected_provider.clone())
        .ok_or(LlmError::ProviderNotConfigured)?;
    let model = override_model
        .or_else(|| ai_agents.selected_model.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or(LlmError::ModelNotConfigured)?;

    let api_key = match provider {
        LlmProvider::OpenAi => ai_agents.openai_api_key.clone(),
        LlmProvider::Gemini => ai_agents.gemini_api_key.clone(),
        LlmProvider::OpenRouter => ai_agents.openrouter_api_key.clone(),
        LlmProvider::DeepSeek => ai_agents.deepseek_api_key.clone(),
        LlmProvider::Zai => ai_agents.zai_api_key.clone(),
        LlmProvider::OpenAiCompatible => ai_agents.openai_compatible_api_key.clone(),
    }
    .filter(|value| !value.trim().is_empty())
    .ok_or(LlmError::CredentialsNotConfigured)?;

    let base_url = match provider {
        LlmProvider::OpenAiCompatible => {
            let raw = ai_agents
                .openai_compatible_base_url
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    LlmError::ProviderRejected(
                        "OpenAI Compatible base URL is not configured".to_string(),
                    )
                })?;
            Some(normalize_openai_compatible_base_url(raw).map_err(LlmError::ProviderRejected)?)
        }
        _ => None,
    };

    Ok(LlmProviderConfig {
        provider,
        model,
        api_key,
        base_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::llm::LlmProvider;

    fn sample_ai_agents() -> AiAgentsConfig {
        AiAgentsConfig {
            openai_api_key: Some("sk-openai".to_string()),
            gemini_api_key: Some("sk-gemini".to_string()),
            openrouter_api_key: Some("sk-openrouter".to_string()),
            deepseek_api_key: Some("sk-deepseek".to_string()),
            selected_provider: Some(LlmProvider::OpenAi),
            selected_model: Some("gpt-4o-mini".to_string()),
            ..AiAgentsConfig::default()
        }
    }

    #[test]
    fn resolve_llm_config_uses_active_provider_when_override_empty() {
        let config = resolve_llm_config(&sample_ai_agents(), None, None).unwrap();

        assert_eq!(config.provider, LlmProvider::OpenAi);
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.api_key, "sk-openai");
        assert_eq!(config.base_url, None);
    }

    #[test]
    fn resolve_llm_config_uses_override_provider_and_model() {
        let config = resolve_llm_config(
            &sample_ai_agents(),
            Some(LlmProvider::DeepSeek),
            Some("deepseek-v4-pro".to_string()),
        )
        .unwrap();

        assert_eq!(config.provider, LlmProvider::DeepSeek);
        assert_eq!(config.model, "deepseek-v4-pro");
        assert_eq!(config.api_key, "sk-deepseek");
    }

    #[test]
    fn resolve_llm_config_fails_when_override_provider_has_no_credentials() {
        let ai_agents = AiAgentsConfig {
            deepseek_api_key: None,
            ..sample_ai_agents()
        };

        let error = resolve_llm_config(
            &ai_agents,
            Some(LlmProvider::DeepSeek),
            Some("deepseek-v4-pro".to_string()),
        )
        .unwrap_err();

        assert!(matches!(error, LlmError::CredentialsNotConfigured));
    }

    #[test]
    fn resolve_llm_config_openai_compatible_requires_base_url_and_key() {
        let ai_agents = AiAgentsConfig {
            openai_compatible_api_key: Some("sk-local".to_string()),
            openai_compatible_base_url: Some("http://127.0.0.1:11434/v1/".to_string()),
            selected_provider: Some(LlmProvider::OpenAiCompatible),
            selected_model: Some("llama3.2".to_string()),
            ..AiAgentsConfig::default()
        };

        let config = resolve_llm_config(&ai_agents, None, None).unwrap();
        assert_eq!(config.provider, LlmProvider::OpenAiCompatible);
        assert_eq!(config.model, "llama3.2");
        assert_eq!(config.api_key, "sk-local");
        assert_eq!(
            config.base_url.as_deref(),
            Some("http://127.0.0.1:11434/v1")
        );
    }

    #[test]
    fn resolve_llm_config_openai_compatible_fails_without_base_url() {
        let ai_agents = AiAgentsConfig {
            openai_compatible_api_key: Some("sk-local".to_string()),
            openai_compatible_base_url: None,
            selected_provider: Some(LlmProvider::OpenAiCompatible),
            selected_model: Some("llama3.2".to_string()),
            ..AiAgentsConfig::default()
        };

        let error = resolve_llm_config(&ai_agents, None, None).unwrap_err();
        assert!(matches!(error, LlmError::ProviderRejected(_)));
    }
}
