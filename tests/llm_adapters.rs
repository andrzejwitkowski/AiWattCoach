use std::sync::{Arc, Mutex};

use aiwattcoach::{
    adapters::llm::{
        gemini::{cache::context_hash, client::GeminiClient},
        openai::client::OpenAiClient,
        openrouter::client::OpenRouterClient,
    },
    domain::llm::{
        LlmChatMessage, LlmChatPort, LlmChatRequest, LlmMessageRole, LlmProvider, LlmProviderConfig,
    },
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct MockServerState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

#[derive(Clone, Debug)]
struct CapturedRequest {
    path: String,
    authorization: Option<String>,
    body: Value,
}

struct MockServer {
    base_url: String,
    state: MockServerState,
}

impl MockServer {
    async fn start() -> Self {
        let state = MockServerState::default();
        let app = Router::new()
            .route("/v1/chat/completions", post(openai_handler))
            .route("/api/v1/chat/completions", post(openrouter_handler))
            .route("/v1beta/cachedContents", post(gemini_cache_handler))
            .route(
                "/v1beta/models/gemini-2.5-flash:generateContent",
                post(gemini_generate_handler),
            )
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url: format!("http://{address}"),
            state,
        }
    }

    fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

fn sample_request() -> LlmChatRequest {
    LlmChatRequest {
        user_id: "user-1".to_string(),
        system_prompt: "system".to_string(),
        stable_context: "stable".to_string(),
        conversation: vec![LlmChatMessage {
            role: LlmMessageRole::User,
            content: "How did I do?".to_string(),
        }],
        cache_scope_key: Some("scope-1".to_string()),
        cache_key: Some("cache-key-1".to_string()),
        reusable_cache_id: None,
    }
}

#[tokio::test]
async fn openai_client_maps_response_and_cached_tokens() {
    let server = MockServer::start().await;
    let client =
        OpenAiClient::new(reqwest::Client::new()).with_base_url(format!("{}/v1", server.base_url));

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
    let client = GeminiClient::new(reqwest::Client::new())
        .with_base_url(format!("{}/v1beta", server.base_url));

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
            LlmChatRequest {
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
}

#[tokio::test]
async fn openrouter_client_maps_cache_discount_and_write_tokens() {
    let server = MockServer::start().await;
    let client = OpenRouterClient::new(reqwest::Client::new())
        .with_base_url(format!("{}/api/v1", server.base_url));

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
}

async fn openai_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/v1/chat/completions", headers, body);
    Json(json!({
        "id": "openai-req-1",
        "model": "gpt-4o-mini",
        "choices": [{ "message": { "content": "OpenAI says hi" } }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120,
            "prompt_tokens_details": { "cached_tokens": 42 }
        }
    }))
}

async fn openrouter_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/api/v1/chat/completions", headers, body);
    Json(json!({
        "id": "openrouter-req-1",
        "model": "openai/gpt-4o-mini",
        "choices": [{ "message": { "content": "OpenRouter says hi" } }],
        "usage": {
            "prompt_tokens": 120,
            "completion_tokens": 25,
            "total_tokens": 145,
            "cache_discount": "0.0012",
            "prompt_tokens_details": {
              "cached_tokens": 80,
              "cache_write_tokens": 32
            }
        }
    }))
}

async fn gemini_cache_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/v1beta/cachedContents", headers, body);
    Json(json!({
        "name": "cachedContents/cache-1",
        "expireTime": "2030-01-01T00:00:00Z"
    }))
}

async fn gemini_generate_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(
        &state,
        "/v1beta/models/gemini-2.5-flash:generateContent",
        headers,
        body,
    );
    (
        StatusCode::OK,
        Json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "text": "Gemini says hi" }]
                }
            }],
            "usageMetadata": {
                "promptTokenCount": 180,
                "candidatesTokenCount": 18,
                "totalTokenCount": 198,
                "cachedContentTokenCount": 128
            }
        })),
    )
}

fn capture_request(state: &MockServerState, path: &str, headers: HeaderMap, body: Value) {
    state.requests.lock().unwrap().push(CapturedRequest {
        path: path.to_string(),
        authorization: headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        body,
    });
}
