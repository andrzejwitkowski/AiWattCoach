use aiwattcoach::{
    adapters::llm::gemini::cache::context_hash,
    domain::llm::{LlmChatPort, LlmProvider, LlmProviderConfig},
};

use crate::support::{
    gemini_client, openai_client, openai_forbidden_client, openrouter_client, sample_request,
    MockServer,
};

#[tokio::test]
async fn openai_client_maps_response_and_cached_tokens() {
    let server = MockServer::start().await;
    let client = openai_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                api_key: "openai-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.message, "OpenAI says hi");
    assert_eq!(response.cache.cached_read_tokens, Some(42));
    assert!(response.cache.cache_hit);

    let requests = server.requests();
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer openai-key")
    );
    assert_eq!(requests[0].body["prompt_cache_key"], "cache-key-1");
}

#[tokio::test]
async fn gemini_client_creates_cache_and_reuses_cached_content() {
    let server = MockServer::start().await;
    let client = gemini_client(&server.base_url);

    let first = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "gemini-2.5-flash".to_string(),
                api_key: "gemini-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(first.message, "Gemini says hi");
    assert_eq!(
        first.cache.provider_cache_id.as_deref(),
        Some("cachedContents/cache-1")
    );
    assert_eq!(first.cache.cached_read_tokens, Some(128));
    assert!(first.cache.cache_expires_at_epoch_seconds.is_some());

    let second = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "gemini-2.5-flash".to_string(),
                api_key: "gemini-key".to_string(),
            },
            aiwattcoach::domain::llm::LlmChatRequest {
                reusable_cache_id: Some("cachedContents/cache-1".to_string()),
                ..sample_request()
            },
        )
        .await
        .unwrap();

    assert_eq!(second.cache.cached_read_tokens, Some(128));

    let requests = server.requests();
    assert_eq!(requests[0].path, "/v1beta/cachedContents");
    assert_eq!(
        requests[0].body["systemInstruction"]["parts"][0]["text"],
        "system"
    );
    assert_eq!(
        requests[0].body["contents"][0]["parts"][0]["text"],
        "stable"
    );
    assert_eq!(
        requests[1].path,
        "/v1beta/models/gemini-2.5-flash:generateContent"
    );
    assert_eq!(
        requests[2].path,
        "/v1beta/models/gemini-2.5-flash:generateContent"
    );
    assert_eq!(requests[1].body["cachedContent"], "cachedContents/cache-1");
    assert_eq!(requests[2].body["cachedContent"], "cachedContents/cache-1");
    assert_eq!(context_hash(&sample_request()).len(), 64);
    assert!(requests[1].body.get("systemInstruction").is_none());
    assert!(requests[2].body.get("systemInstruction").is_none());
}

#[tokio::test]
async fn gemini_client_accepts_google_prefixed_model_name() {
    let server = MockServer::start().await;
    let client = gemini_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "google/gemini-2.5-flash".to_string(),
                api_key: "gemini-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.message, "Gemini says hi");
}

#[tokio::test]
async fn openrouter_client_maps_cache_discount_and_write_tokens() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.message, "OpenRouter says hi");
    assert_eq!(response.cache.cached_read_tokens, Some(80));
    assert_eq!(response.cache.cache_write_tokens, Some(32));
    assert_eq!(response.cache.cache_discount.as_deref(), Some("0.0012"));

    let requests = server.requests();
    assert_eq!(requests[0].authorization.as_deref(), Some("Bearer or-key"));
    assert_eq!(requests[0].referer.as_deref(), Some("http://localhost"));
    assert_eq!(requests[0].title.as_deref(), Some("AiWattCoach"));
}

#[tokio::test]
async fn openrouter_request_caches_stable_prefix_only() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "google/gemini-3-flash-preview".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    let requests = server.requests();
    let messages = requests[0].body["messages"].as_array().unwrap();

    assert!(messages[0]["content"].is_array());
    assert_eq!(messages[0]["content"][0]["type"], "text");
    assert_eq!(messages[0]["content"][0]["text"], "system");
    assert_eq!(
        messages[0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(messages[1]["content"][0]["text"], "stable");
    assert_eq!(
        messages[1]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(messages[2]["content"][0]["text"], "volatile");
    assert_eq!(
        messages[2]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(messages[3]["role"], "user");
    assert_eq!(messages[3]["content"], "How did I do?");
    assert!(messages[3].get("cache_control").is_none());
}

#[tokio::test]
async fn gemini_client_skips_cache_creation_without_durable_cache_keys() {
    let server = MockServer::start().await;
    let client = gemini_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "gemini-2.5-flash".to_string(),
                api_key: "gemini-key".to_string(),
            },
            aiwattcoach::domain::llm::LlmChatRequest {
                cache_scope_key: None,
                cache_key: None,
                ..sample_request()
            },
        )
        .await
        .unwrap();

    assert_eq!(response.message, "Gemini says hi");
    assert_eq!(response.cache.provider_cache_id, None);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/v1beta/models/gemini-2.5-flash:generateContent"
    );
    assert_eq!(
        requests[0].body["systemInstruction"]["parts"][0]["text"],
        "system\n\nstable"
    );
}

#[tokio::test]
async fn context_hash_includes_field_boundaries() {
    let first = aiwattcoach::domain::llm::LlmChatRequest {
        system_prompt: "ab".to_string(),
        stable_context: "c".to_string(),
        ..sample_request()
    };
    let second = aiwattcoach::domain::llm::LlmChatRequest {
        system_prompt: "a".to_string(),
        stable_context: "bc".to_string(),
        ..sample_request()
    };

    assert_ne!(context_hash(&first), context_hash(&second));
}

#[tokio::test]
async fn openrouter_client_does_not_fallback_cache_discount_to_cost() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini-no-discount".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.cache.cache_discount, None);
}

#[tokio::test]
async fn openrouter_client_maps_payment_required_to_provider_rejected() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    let error = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini-no-credits".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::llm::LlmError::ProviderRejected(
            r#"{"error":{"message":"Insufficient credits","code":402}}"#.to_string(),
        )
    );
}

#[tokio::test]
async fn openrouter_client_parses_array_content_parts() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "google/gemini-3-flash-preview".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.message, "OpenRouter says hi from parts");
}

#[tokio::test]
async fn openrouter_client_parses_numeric_usage_fields() {
    let server = MockServer::start().await;
    let client = openrouter_client(&server.base_url);

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "google/gemini-3-flash-preview-numeric-usage".to_string(),
                api_key: "or-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap();

    assert_eq!(response.message, "OK");
    assert_eq!(response.cache.cache_discount.as_deref(), Some("0.000014"));
}

#[tokio::test]
async fn openai_client_maps_forbidden_to_credentials_not_configured() {
    let server = MockServer::start().await;
    let client = openai_forbidden_client(&server.base_url);

    let error = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                api_key: "openai-key".to_string(),
            },
            sample_request(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        aiwattcoach::domain::llm::LlmError::CredentialsNotConfigured
    );
}
