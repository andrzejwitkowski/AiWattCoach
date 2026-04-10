use std::sync::{Arc, Mutex};

use aiwattcoach::{
    adapters::llm::{
        gemini::client::GeminiClient, openai::client::OpenAiClient,
        openrouter::client::OpenRouterClient,
    },
    domain::llm::{
        BoxFuture as LlmBoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmContextCache, LlmContextCacheRepository, LlmError, LlmMessageRole, LlmProvider,
        LlmProviderConfig, LlmTokenUsage, UserLlmConfigProvider,
    },
    domain::{
        identity::Clock,
        training_context::{
            IntervalsStatusContext, RenderedTrainingContext, TrainingContext,
            TrainingContextBuildResult, TrainingContextBuilder,
        },
        workout_summary::WorkoutSummary,
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
pub(crate) struct MockServerState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct CapturedRequest {
    pub(crate) path: String,
    pub(crate) authorization: Option<String>,
    pub(crate) referer: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) body: Value,
}

pub(crate) struct MockServer {
    pub(crate) base_url: String,
    state: MockServerState,
}

#[derive(Clone, Default)]
pub(crate) struct CapturingChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
}

impl CapturingChatPort {
    pub(crate) fn requests(&self) -> Vec<LlmChatRequest> {
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
pub(crate) struct FixedGeminiConfigProvider;

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
pub(crate) struct FailingReusableCacheRepository;

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
pub(crate) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(crate) struct StubTrainingContextBuilder;

impl TrainingContextBuilder for StubTrainingContextBuilder {
    fn build(
        &self,
        _user_id: &str,
        workout_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            Ok(TrainingContextBuildResult {
                context: TrainingContext {
                    generated_at_epoch_seconds: 1_700_000_000,
                    focus_workout_id: Some(workout_id),
                    focus_kind: "activity".to_string(),
                    intervals_status: IntervalsStatusContext {
                        activities: "ok".to_string(),
                        events: "ok".to_string(),
                    },
                    profile: Default::default(),
                    races: Vec::new(),
                    future_events: Vec::new(),
                    history: Default::default(),
                    recent_days: Vec::new(),
                    upcoming_days: Vec::new(),
                    projected_days: Vec::new(),
                },
                rendered: RenderedTrainingContext {
                    stable_context: "{\"stable\":true}".to_string(),
                    volatile_context: "{\"volatile\":true}".to_string(),
                    approximate_tokens: 100,
                },
            })
        })
    }

    fn build_athlete_summary_context(
        &self,
        _user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build("user-1", "athlete-summary")
    }
}

impl MockServer {
    pub(crate) async fn start() -> Self {
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

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

pub(crate) fn sample_request() -> LlmChatRequest {
    LlmChatRequest {
        user_id: "user-1".to_string(),
        system_prompt: "system".to_string(),
        stable_context: "stable".to_string(),
        volatile_context: "volatile".to_string(),
        conversation: vec![LlmChatMessage {
            role: LlmMessageRole::User,
            content: "How did I do?".to_string(),
        }],
        cache_scope_key: Some("scope-1".to_string()),
        cache_key: Some("cache-key-1".to_string()),
        reusable_cache_id: None,
    }
}

pub(crate) fn sample_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

pub(crate) fn openai_client(base_url: &str) -> OpenAiClient {
    OpenAiClient::new(reqwest::Client::new()).with_base_url(format!("{base_url}/v1"))
}

pub(crate) fn openai_forbidden_client(base_url: &str) -> OpenAiClient {
    OpenAiClient::new(reqwest::Client::new()).with_base_url(format!("{base_url}/v1-forbidden"))
}

pub(crate) fn openrouter_client(base_url: &str) -> OpenRouterClient {
    OpenRouterClient::new(reqwest::Client::new()).with_base_url(format!("{base_url}/api/v1"))
}

pub(crate) fn gemini_client(base_url: &str) -> GeminiClient {
    GeminiClient::new(reqwest::Client::new()).with_base_url(format!("{base_url}/v1beta"))
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
    if model == "openai/gpt-4o-mini-no-credits" {
        return (
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({ "error": { "message": "Insufficient credits", "code": 402 } })),
        )
            .into_response();
    }
    if model == "google/gemini-3-flash-preview" {
        return Json(json!({
            "id": "openrouter-req-1",
            "model": model,
            "choices": [{
                "message": {
                    "content": [
                        { "type": "text", "text": "OpenRouter says hi from parts" }
                    ]
                }
            }],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 25,
                "total_tokens": 145
            }
        }))
        .into_response();
    }
    if model == "google/gemini-3-flash-preview-numeric-usage" {
        return Json(json!({
            "id": "openrouter-req-1",
            "model": model,
            "choices": [{
                "message": {
                    "content": "OK"
                }
            }],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 25,
                "total_tokens": 145,
                "cost": 0.000014,
                "cache_discount": 0.000014
            }
        }))
        .into_response();
    }
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
    .into_response()
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
        referer: headers
            .get("HTTP-Referer")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        title: headers
            .get("X-OpenRouter-Title")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        body,
    });
}
