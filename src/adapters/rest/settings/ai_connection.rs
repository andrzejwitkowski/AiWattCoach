use crate::domain::{
    llm::{LlmChatRequest, LlmProvider, LlmProviderConfig},
    settings::UserSettings,
};

use super::dto::{
    test_ai_agents_connection_response, TestAiAgentsConnectionResponse, UpdateAiAgentsRequest,
};

pub(super) struct MergedAiConnectionConfig {
    pub(super) config: LlmProviderConfig,
    pub(super) used_saved_api_key: bool,
    pub(super) used_saved_provider: bool,
    pub(super) used_saved_model: bool,
}

pub(super) fn merge_ai_connection_config(
    body: UpdateAiAgentsRequest,
    current: &UserSettings,
) -> Result<MergedAiConnectionConfig, TestAiAgentsConnectionResponse> {
    let transient_provider = match body.selected_provider {
        Some(value) => Some(parse_provider(&value).ok_or_else(|| {
            test_ai_agents_connection_response(
                false,
                "selectedProvider must be one of: openai, gemini, openrouter",
                false,
                false,
                false,
            )
        })?),
        None => None,
    };
    let transient_model = normalize_optional_input(body.selected_model);
    let transient_openai_api_key = normalize_optional_input(body.openai_api_key);
    let transient_gemini_api_key = normalize_optional_input(body.gemini_api_key);
    let transient_openrouter_api_key = normalize_optional_input(body.openrouter_api_key);

    let provider = transient_provider
        .clone()
        .or(current.ai_agents.selected_provider.clone());
    let model = transient_model
        .clone()
        .or(current.ai_agents.selected_model.clone());

    let provider = match provider {
        Some(provider) => provider,
        None => {
            return Err(test_ai_agents_connection_response(
                false,
                "Provider, model, and matching API key are required.",
                false,
                false,
                false,
            ))
        }
    };

    let api_key = match provider {
        LlmProvider::OpenAi => transient_openai_api_key
            .clone()
            .or(current.ai_agents.openai_api_key.clone()),
        LlmProvider::Gemini => transient_gemini_api_key
            .clone()
            .or(current.ai_agents.gemini_api_key.clone()),
        LlmProvider::OpenRouter => transient_openrouter_api_key
            .clone()
            .or(current.ai_agents.openrouter_api_key.clone()),
    };

    let Some(model) = model.filter(|value| !value.trim().is_empty()) else {
        return Err(test_ai_agents_connection_response(
            false,
            "Provider, model, and matching API key are required.",
            false,
            transient_provider.is_none() && current.ai_agents.selected_provider.is_some(),
            false,
        ));
    };

    let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) else {
        return Err(test_ai_agents_connection_response(
            false,
            "Provider, model, and matching API key are required.",
            current_api_key_is_saved(provider.clone(), current)
                && selected_key_was_not_provided(
                    &provider,
                    transient_openai_api_key.is_none(),
                    transient_gemini_api_key.is_none(),
                    transient_openrouter_api_key.is_none(),
                ),
            transient_provider.is_none() && current.ai_agents.selected_provider.is_some(),
            transient_model.is_none() && current.ai_agents.selected_model.is_some(),
        ));
    };

    Ok(MergedAiConnectionConfig {
        config: LlmProviderConfig {
            provider: provider.clone(),
            model,
            api_key,
        },
        used_saved_api_key: current_api_key_is_saved(provider.clone(), current)
            && selected_key_was_not_provided(
                &provider,
                transient_openai_api_key.is_none(),
                transient_gemini_api_key.is_none(),
                transient_openrouter_api_key.is_none(),
            ),
        used_saved_provider: transient_provider.is_none()
            && current.ai_agents.selected_provider.is_some(),
        used_saved_model: transient_model.is_none() && current.ai_agents.selected_model.is_some(),
    })
}

pub(super) fn build_test_request(user_id: &str) -> LlmChatRequest {
    LlmChatRequest {
        user_id: user_id.to_string(),
        system_prompt: "You are a connection test assistant. Reply with OK only.".to_string(),
        stable_context: "llm connection test".to_string(),
        conversation: vec![crate::domain::llm::LlmChatMessage {
            role: crate::domain::llm::LlmMessageRole::User,
            content: "Reply with OK only.".to_string(),
        }],
        cache_scope_key: None,
        cache_key: None,
        reusable_cache_id: None,
    }
}

pub(super) fn map_ai_connection_error_to_response(
    error: crate::domain::llm::LlmError,
    used_saved_api_key: bool,
    used_saved_provider: bool,
    used_saved_model: bool,
) -> (
    axum::http::StatusCode,
    axum::Json<TestAiAgentsConnectionResponse>,
) {
    use axum::http::StatusCode;
    let (status, message) = match error {
        crate::domain::llm::LlmError::CredentialsNotConfigured
        | crate::domain::llm::LlmError::ProviderNotConfigured
        | crate::domain::llm::LlmError::ModelNotConfigured
        | crate::domain::llm::LlmError::UnsupportedProvider(_)
        | crate::domain::llm::LlmError::ProviderRejected(_) => {
            (StatusCode::BAD_REQUEST, error.to_string())
        }
        crate::domain::llm::LlmError::RateLimited(_)
        | crate::domain::llm::LlmError::Transport(_)
        | crate::domain::llm::LlmError::InvalidResponse(_)
        | crate::domain::llm::LlmError::Internal(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, error.to_string())
        }
    };

    (
        status,
        axum::Json(test_ai_agents_connection_response(
            false,
            &message,
            used_saved_api_key,
            used_saved_provider,
            used_saved_model,
        )),
    )
}

fn normalize_optional_input(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn parse_provider(value: &str) -> Option<LlmProvider> {
    LlmProvider::parse(value)
}

fn current_api_key_is_saved(provider: LlmProvider, current: &UserSettings) -> bool {
    match provider {
        LlmProvider::OpenAi => current.ai_agents.openai_api_key.is_some(),
        LlmProvider::Gemini => current.ai_agents.gemini_api_key.is_some(),
        LlmProvider::OpenRouter => current.ai_agents.openrouter_api_key.is_some(),
    }
}

fn selected_key_was_not_provided(
    provider: &LlmProvider,
    openai_missing: bool,
    gemini_missing: bool,
    openrouter_missing: bool,
) -> bool {
    match provider {
        LlmProvider::OpenAi => openai_missing,
        LlmProvider::Gemini => gemini_missing,
        LlmProvider::OpenRouter => openrouter_missing,
    }
}
