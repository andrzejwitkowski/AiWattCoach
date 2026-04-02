use std::sync::{Arc, Mutex};

use aiwattcoach::{
    adapters::llm::{
        gemini::{cache::context_hash, client::GeminiClient},
        openai::client::OpenAiClient,
        openrouter::client::OpenRouterClient,
        workout_summary_coach::LlmWorkoutCoach,
    },
    domain::llm::{
        BoxFuture as LlmBoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmContextCache, LlmContextCacheRepository, LlmError, LlmMessageRole, LlmProvider,
        LlmProviderConfig, LlmTokenUsage, UserLlmConfigProvider,
    },
    domain::{
        identity::Clock,
        workout_summary::{WorkoutCoach, WorkoutSummary},
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

#[derive(Clone, Default)]
struct CapturingChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
}

impl CapturingChatPort {
    fn requests(&self) -> Vec<LlmChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl LlmChatPort for CapturingChatPort {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> LlmBoxFuture<Result<LlmChatResponse, LlmError>> {
        self.requests.lock().unwrap().push(request);
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::Gemini,
                model: "gemini-3.1-pro".to_string(),
                message: "Gemini coach reply".to_string(),
                provider_request_id: Some("req-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: Default::default(),
            })
        })
    }
}

#[derive(Clone)]
struct FixedGeminiConfigProvider;

impl UserLlmConfigProvider for FixedGeminiConfigProvider {
    fn get_config(&self, _user_id: &str) -> LlmBoxFuture<Result<LlmProviderConfig, LlmError>> {
        Box::pin(async {
            Ok(LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "gemini-3.1-pro".to_string(),
                api_key: "gemini-key".to_string(),
            })
        })
    }
}

#[derive(Clone)]
struct FailingReusableCacheRepository;

impl LlmContextCacheRepository for FailingReusableCacheRepository {
    fn find_reusable(
        &self,
        _user_id: &str,
        _provider: &LlmProvider,
        _model: &str,
        _scope_key: &str,
        _context_hash: &str,
        _now_epoch_seconds: i64,
    ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        Box::pin(async {
            Err(LlmError::Internal(
                "cache lookup should not fail the coach reply".to_string(),
            ))
        })
    }

    fn upsert(&self, cache: LlmContextCache) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
        Box::pin(async move { Ok(cache) })
    }

    fn delete_by_user_id(&self, _user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

impl MockServer {
    async fn start() -> Self {
        let state = MockServerState::default();
        let app = Router::new()
            .route("/v1/chat/completions", post(openai_handler))
            .route(
                "/v1-forbidden/chat/completions",
                post(openai_forbidden_handler),
            )
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

fn sample_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
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

#[tokio::test]
async fn gemini_client_skips_cache_creation_without_durable_cache_keys() {
    let server = MockServer::start().await;
    let client = GeminiClient::new(reqwest::Client::new())
        .with_base_url(format!("{}/v1beta", server.base_url));

    let response = client
        .chat(
            LlmProviderConfig {
                provider: LlmProvider::Gemini,
                model: "gemini-2.5-flash".to_string(),
                api_key: "gemini-key".to_string(),
            },
            LlmChatRequest {
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
async fn llm_workout_coach_does_not_fail_when_gemini_cache_lookup_errors() {
    let chat_port = Arc::new(CapturingChatPort::default());
    let coach = LlmWorkoutCoach::new(
        chat_port.clone(),
        Arc::new(FixedGeminiConfigProvider),
        FixedClock,
    )
    .with_context_cache_repository(Arc::new(FailingReusableCacheRepository));

    let response = coach
        .reply("user-1", &sample_summary(), "How did I do?")
        .await
        .unwrap();

    assert_eq!(response.message, "Gemini coach reply");

    let requests = chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].reusable_cache_id, None);
}

#[tokio::test]
async fn context_hash_includes_field_boundaries() {
    let first = LlmChatRequest {
        system_prompt: "ab".to_string(),
        stable_context: "c".to_string(),
        ..sample_request()
    };
    let second = LlmChatRequest {
        system_prompt: "a".to_string(),
        stable_context: "bc".to_string(),
        ..sample_request()
    };

    assert_ne!(context_hash(&first), context_hash(&second));
}

#[tokio::test]
async fn openrouter_client_does_not_fallback_cache_discount_to_cost() {
    let server = MockServer::start().await;
    let client = OpenRouterClient::new(reqwest::Client::new())
        .with_base_url(format!("{}/api/v1", server.base_url));

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
async fn openai_client_maps_forbidden_to_credentials_not_configured() {
    let server = MockServer::start().await;
    let client = OpenAiClient::new(reqwest::Client::new())
        .with_base_url(format!("{}/v1-forbidden", server.base_url));

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

#[test]
fn llm_debug_output_redacts_secrets_and_prompt_contents() {
    let config = LlmProviderConfig {
        provider: LlmProvider::OpenAi,
        model: "gpt-4o-mini".to_string(),
        api_key: "sk-secret-value".to_string(),
    };
    let request = sample_request();

    let config_debug = format!("{config:?}");
    let request_debug = format!("{request:?}");

    assert!(!config_debug.contains("sk-secret-value"));
    assert!(config_debug.contains("<redacted:"));
    assert!(!request_debug.contains("How did I do?"));
    assert!(!request_debug.contains("stable_context: \"stable\""));
    assert!(!request_debug.contains("system_prompt: \"system\""));
    assert!(request_debug.contains("conversation_len"));
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

async fn openai_forbidden_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/v1-forbidden/chat/completions", headers, body);
    (StatusCode::FORBIDDEN, "forbidden")
}

async fn openrouter_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    capture_request(&state, "/api/v1/chat/completions", headers, body);
    let usage = if model == "openai/gpt-4o-mini-no-discount" {
        json!({
            "prompt_tokens": 120,
            "completion_tokens": 25,
            "total_tokens": 145,
            "cost": "0.0099",
            "prompt_tokens_details": {
              "cached_tokens": 80,
              "cache_write_tokens": 32
            }
        })
    } else {
        json!({
            "prompt_tokens": 120,
            "completion_tokens": 25,
            "total_tokens": 145,
            "cache_discount": "0.0012",
            "prompt_tokens_details": {
              "cached_tokens": 80,
              "cache_write_tokens": 32
            }
        })
    };
    Json(json!({
        "id": "openrouter-req-1",
        "model": model,
        "choices": [{ "message": { "content": "OpenRouter says hi" } }],
        "usage": usage
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
