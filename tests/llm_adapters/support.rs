use std::sync::{Arc, Mutex};

use aiwattcoach::{
    adapters::llm::{
        gemini::client::GeminiClient,
        openai_compatible::client::OpenAiCompatibleClient as OpenAiClient,
        openrouter::client::OpenRouterClient, zai::client::ZaiClient,
    },
    domain::llm::{
        BoxFuture as LlmBoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmContextCache, LlmContextCacheRepository, LlmError, LlmMessageRole, LlmProvider,
        LlmProviderConfig, LlmTokenUsage, LlmToolChoice, UserLlmConfigProvider,
    },
    domain::{
        identity::Clock,
        training_context::{
            IntervalsStatusContext, RenderedTrainingContext, TrainingContext,
            TrainingContextBuildResult, TrainingContextBuilder, ATHLETE_SUMMARY_FOCUS_ID,
            CALENDAR_OVERVIEW_FOCUS_ID, MESO_CYCLE_FOCUS_ID,
        },
        training_plan::{
            training_plan_llm_envelope_json_schema, TrainingPlanError, WorkoutPlanningLlmConfigPort,
        },
        workout_summary::{WorkoutChatLlmConfigPort, WorkoutSummary, WorkoutSummaryError},
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
use tokio::task::JoinHandle;

#[derive(Clone, Default)]
pub(crate) struct MockServerState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    openrouter_empty_then_success_calls: Arc<Mutex<u32>>,
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
    task: JoinHandle<()>,
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
        let is_training_plan_generation = request
            .system_prompt
            .contains(&training_plan_llm_envelope_json_schema());
        self.requests.lock().unwrap().push(request);
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::Gemini,
                model: "gemini-3.1-pro".to_string(),
                message: if is_training_plan_generation {
                    LlmChatMessage::assistant(
                        r#"{"plan":"2023-11-15\nRest Day","description":"Gemini coach reply"}"#,
                    )
                } else {
                    LlmChatMessage::assistant("Gemini coach reply")
                },
                finish_reason: None,
                provider_request_id: Some("req-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: Default::default(),
            })
        })
    }
}

macro_rules! impl_workout_ports_for_user_llm {
    ($provider:ty) => {
        impl WorkoutChatLlmConfigPort for $provider {
            fn get_workout_chat_config(
                &self,
                user_id: &str,
            ) -> LlmBoxFuture<Result<LlmProviderConfig, WorkoutSummaryError>> {
                let provider = self.clone();
                let user_id = user_id.to_string();
                Box::pin(async move {
                    provider
                        .get_config(&user_id)
                        .await
                        .map_err(WorkoutSummaryError::Llm)
                })
            }
        }

        impl WorkoutPlanningLlmConfigPort for $provider {
            fn get_workout_planning_config(
                &self,
                user_id: &str,
            ) -> LlmBoxFuture<Result<LlmProviderConfig, TrainingPlanError>> {
                let provider = self.clone();
                let user_id = user_id.to_string();
                Box::pin(async move {
                    provider
                        .get_config(&user_id)
                        .await
                        .map_err(|error| TrainingPlanError::Unavailable(error.to_string()))
                })
            }
        }
    };
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

impl_workout_ports_for_user_llm!(FixedGeminiConfigProvider);

#[derive(Clone)]
pub(crate) struct FixedOpenAiConfigProvider;

impl UserLlmConfigProvider for FixedOpenAiConfigProvider {
    fn get_config(&self, _user_id: &str) -> LlmBoxFuture<Result<LlmProviderConfig, LlmError>> {
        Box::pin(async {
            Ok(LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-4o-mini".to_string(),
                api_key: "openai-key".to_string(),
            })
        })
    }
}

impl_workout_ports_for_user_llm!(FixedOpenAiConfigProvider);

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
                focus_date: "2026-05-29".to_string(),
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
                    planned_rest_days: Vec::new(),
                    future_events: Vec::new(),
                    history: Default::default(),
                    recent_days: Vec::new(),
                    recent_workout_recaps: Vec::new(),
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

    fn build_as_of(
        &self,
        user_id: &str,
        workout_id: &str,
        _focus_date: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build(user_id, workout_id)
    }

    fn build_calendar_overview_context(
        &self,
        user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build(user_id, CALENDAR_OVERVIEW_FOCUS_ID)
    }

    fn build_calendar_overview_context_as_of(
        &self,
        user_id: &str,
        _focus_date: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build_calendar_overview_context(user_id)
    }

    fn build_athlete_summary_context(
        &self,
        user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build(user_id, ATHLETE_SUMMARY_FOCUS_ID)
    }

    fn build_meso_cycle_context(
        &self,
        user_id: &str,
        _meso_end: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build(user_id, MESO_CYCLE_FOCUS_ID)
    }
}

impl MockServer {
    pub(crate) async fn start() -> Self {
        let state = MockServerState::default();
        let app = Router::new()
            .route("/v1/chat/completions", post(openai_handler))
            .route("/chat/completions", post(deepseek_handler))
            .route("/api/paas/v4/chat/completions", post(zai_handler))
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
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url: format!("http://{address}"),
            state,
            task,
        }
    }

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
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
            tool_calls: Vec::new(),
            tool_call_id: None,
            reasoning_content: None,
        }],
        cache_scope_key: Some("scope-1".to_string()),
        cache_key: Some("cache-key-1".to_string()),
        reusable_cache_id: None,
        tools: Vec::new(),
        tool_choice: LlmToolChoice::None,
    }
}

pub(crate) fn sample_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        provider_transcript: Vec::new(),
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

pub(crate) fn deepseek_client(base_url: &str) -> OpenAiClient {
    OpenAiClient::new(reqwest::Client::new()).with_base_url(base_url.to_string())
}

pub(crate) fn zai_client(base_url: &str) -> ZaiClient {
    ZaiClient::new(reqwest::Client::new()).with_base_url(format!("{base_url}/api/paas/v4"))
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
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let model = model.to_string();
    capture_request(&state, "/v1/chat/completions", headers, body);
    if model == "gpt-4o-mini-tool-calls" {
        return Json(json!({
            "id": "openai-req-tool-1",
            "model": model,
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "lookupWorkout",
                            "arguments": "{\"workoutId\":\"workout-1\"}"
                        }
                    }]
                }
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 20,
                "total_tokens": 120,
                "prompt_tokens_details": { "cached_tokens": 42 }
            }
        }));
    }
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

async fn deepseek_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/chat/completions", headers, body.clone());
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("deepseek-v4-flash");
    let (content, reasoning_content) = if model == "deepseek-v4-pro" {
        (String::new(), Some("thinking chain".to_string()))
    } else {
        ("DeepSeek says hi".to_string(), None)
    };
    let mut message = json!({ "content": content });
    if let Some(rc) = reasoning_content {
        message["reasoning_content"] = json!(rc);
    }
    Json(json!({
        "id": "deepseek-req-1",
        "model": model,
        "choices": [{ "message": message }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120,
            "prompt_cache_hit_tokens": 80,
            "prompt_cache_miss_tokens": 20
        }
    }))
}

async fn zai_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/api/paas/v4/chat/completions", headers, body);
    Json(json!({
        "id": "zai-req-1",
        "model": "glm-5.2",
        "choices": [{ "message": { "content": "GLM says hi" } }],
        "usage": {
            "prompt_tokens": 120,
            "completion_tokens": 20,
            "total_tokens": 140,
            "prompt_tokens_details": { "cached_tokens": 96 }
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
    if model == "google/gemini-3-flash-preview-multipart" {
        return Json(json!({
            "id": "openrouter-req-2",
            "model": model,
            "choices": [{
                "message": {
                    "content": [
                        { "type": "text", "text": "OpenRouter says" },
                        { "type": "text", "text": "hi from parts" }
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
    if model == "google/gemini-3-flash-preview-empty-then-success" {
        let mut calls = state.openrouter_empty_then_success_calls.lock().unwrap();
        *calls += 1;
        if *calls == 1 {
            return Json(json!({
                "id": "openrouter-req-empty-1",
                "model": model,
                "choices": [{
                    "message": {
                        "content": [
                            { "type": "text" }
                        ]
                    }
                }],
                "usage": {
                    "prompt_tokens": 120,
                    "completion_tokens": 0,
                    "total_tokens": 120
                }
            }))
            .into_response();
        }

        return Json(json!({
            "id": "openrouter-req-empty-2",
            "model": model,
            "choices": [{
                "message": {
                    "content": "Recovered after retry"
                }
            }],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 12,
                "total_tokens": 132
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
