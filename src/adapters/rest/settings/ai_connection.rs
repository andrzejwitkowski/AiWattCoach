use crate::domain::{
    llm::{LlmChatRequest, LlmProvider, LlmProviderConfig},
    settings::UserSettings,
};

use super::dto::{
    test_ai_agents_connection_response, TestAiAgentsConnectionResponse, UpdateAiAgentsRequest,
};
use super::input::{
    apply_field_update, normalize_string_input, parse_provider_input, used_saved_value, FieldUpdate,
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
    let transient_provider = parse_provider_input(body.selected_provider, || {
        test_ai_agents_connection_response(
            false,
            "selectedProvider must be one of: openai, gemini, openrouter",
            false,
            false,
            false,
        )
    })?;
    let transient_model = normalize_string_input(body.selected_model);
    let transient_openai_api_key = normalize_string_input(body.openai_api_key);
    let transient_gemini_api_key = normalize_string_input(body.gemini_api_key);
    let transient_openrouter_api_key = normalize_string_input(body.openrouter_api_key);

    let provider = apply_field_update(
        transient_provider.clone(),
        current.ai_agents.selected_provider.clone(),
    );
    let model = apply_field_update(
        transient_model.clone(),
        current.ai_agents.selected_model.clone(),
    );

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
        LlmProvider::OpenAi => apply_field_update(
            transient_openai_api_key.clone(),
            current.ai_agents.openai_api_key.clone(),
        ),
        LlmProvider::Gemini => apply_field_update(
            transient_gemini_api_key.clone(),
            current.ai_agents.gemini_api_key.clone(),
        ),
        LlmProvider::OpenRouter => apply_field_update(
            transient_openrouter_api_key.clone(),
            current.ai_agents.openrouter_api_key.clone(),
        ),
    };

    let Some(model) = model else {
        return Err(test_ai_agents_connection_response(
            false,
            "Provider, model, and matching API key are required.",
            false,
            used_saved_value(&transient_provider, &current.ai_agents.selected_provider),
            false,
        ));
    };

    let Some(api_key) = api_key else {
        return Err(test_ai_agents_connection_response(
            false,
            "Provider, model, and matching API key are required.",
            current_api_key_is_saved(provider.clone(), current)
                && selected_key_was_not_provided(
                    &provider,
                    matches!(&transient_openai_api_key, FieldUpdate::Missing),
                    matches!(&transient_gemini_api_key, FieldUpdate::Missing),
                    matches!(&transient_openrouter_api_key, FieldUpdate::Missing),
                ),
            used_saved_value(&transient_provider, &current.ai_agents.selected_provider),
            used_saved_value(&transient_model, &current.ai_agents.selected_model),
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
                matches!(&transient_openai_api_key, FieldUpdate::Missing),
                matches!(&transient_gemini_api_key, FieldUpdate::Missing),
                matches!(&transient_openrouter_api_key, FieldUpdate::Missing),
            ),
        used_saved_provider: used_saved_value(
            &transient_provider,
            &current.ai_agents.selected_provider,
        ),
        used_saved_model: used_saved_value(&transient_model, &current.ai_agents.selected_model),
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
