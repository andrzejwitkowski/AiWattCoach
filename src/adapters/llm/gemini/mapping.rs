use crate::domain::llm::{
    LlmCacheUsage, LlmChatMessage, LlmChatRequest, LlmChatResponse, LlmError, LlmMessageRole,
    LlmProvider, LlmProviderConfig, LlmTokenUsage,
};

use crate::adapters::llm::context_prelude::non_empty_context_parts;

use super::dto::{
    GeminiContent, GeminiCreateCacheRequest, GeminiGenerateContentRequest,
    GeminiGenerateContentResponse, GeminiTextPart,
};

pub fn map_generate_request(
    request: &LlmChatRequest,
    cached_content: Option<String>,
) -> GeminiGenerateContentRequest {
    let cached_content = request.reusable_cache_id.clone().or(cached_content);
    let context_parts = non_empty_context_parts([
        ("user", request.system_prompt.as_str()),
        ("user", request.stable_context.as_str()),
        ("user", request.volatile_context.as_str()),
    ]);
    let system_instruction = cached_content
        .is_none()
        .then(|| {
            context_parts
                .iter()
                .take(2)
                .map(|(_, content)| *content)
                .collect::<Vec<_>>()
        })
        .filter(|parts| !parts.is_empty())
        .map(|_| GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiTextPart {
                text: format!("{}\n\n{}", request.system_prompt, request.stable_context)
                    .trim()
                    .to_string(),
            }],
        });

    GeminiGenerateContentRequest {
        contents: context_parts
            .iter()
            .skip(2)
            .map(|(_, content)| GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiTextPart {
                    text: (*content).to_string(),
                }],
            })
            .chain(request.conversation.iter().cloned().map(map_message))
            .collect(),
        system_instruction,
        cached_content,
    }
}

pub fn map_create_cache_request(
    config: &LlmProviderConfig,
    request: &LlmChatRequest,
) -> Option<GeminiCreateCacheRequest> {
    let system_prompt = request.system_prompt.trim().to_string();
    let stable_context = request.stable_context.trim().to_string();

    if system_prompt.is_empty() && stable_context.is_empty() {
        return None;
    }

    Some(GeminiCreateCacheRequest {
        model: format!("models/{}", config.model),
        contents: if stable_context.is_empty() {
            Vec::new()
        } else {
            vec![GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiTextPart {
                    text: stable_context,
                }],
            }]
        },
        system_instruction: (!system_prompt.is_empty()).then(|| GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiTextPart {
                text: system_prompt,
            }],
        }),
        ttl: Some("3600s".to_string()),
    })
}

pub fn map_response(
    config: &LlmProviderConfig,
    response: GeminiGenerateContentResponse,
    provider_cache_id: Option<String>,
    cache_expires_at_epoch_seconds: Option<i64>,
) -> Result<LlmChatResponse, LlmError> {
    let message = response
        .candidates
        .unwrap_or_default()
        .into_iter()
        .find_map(|candidate| candidate.content)
        .and_then(|content| {
            content.parts.into_iter().find_map(|part| {
                let text = part.text.trim().to_string();
                (!text.is_empty()).then_some(text)
            })
        })
        .ok_or_else(|| {
            LlmError::InvalidResponse("Gemini returned no message content".to_string())
        })?;

    let usage = response.usage_metadata;
    let cached_read_tokens = usage
        .as_ref()
        .and_then(|metadata| metadata.cached_content_token_count);

    Ok(LlmChatResponse {
        provider: LlmProvider::Gemini,
        model: config.model.clone(),
        message,
        provider_request_id: None,
        usage: LlmTokenUsage {
            input_tokens: usage
                .as_ref()
                .and_then(|metadata| metadata.prompt_token_count),
            output_tokens: usage
                .as_ref()
                .and_then(|metadata| metadata.candidates_token_count),
            total_tokens: usage
                .as_ref()
                .and_then(|metadata| metadata.total_token_count),
        },
        cache: LlmCacheUsage {
            cached_read_tokens,
            cache_write_tokens: None,
            cache_hit: cached_read_tokens.unwrap_or(0) > 0,
            cache_discount: None,
            provider_cache_id,
            provider_cache_key: None,
            cache_expires_at_epoch_seconds,
        },
    })
}

fn map_message(message: LlmChatMessage) -> GeminiContent {
    GeminiContent {
        role: match message.role {
            LlmMessageRole::User => "user".to_string(),
            LlmMessageRole::Assistant => "model".to_string(),
        },
        parts: vec![GeminiTextPart {
            text: message.content,
        }],
    }
}
