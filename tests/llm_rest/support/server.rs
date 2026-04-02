use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[derive(Clone, Debug)]
pub(crate) struct CapturedRequest {
    pub(crate) path: String,
    pub(crate) authorization: Option<String>,
    pub(crate) body: Value,
}

#[derive(Clone, Default)]
struct MockServerState {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

pub(crate) struct TestLlmUpstreamServer {
    address: SocketAddr,
    state: MockServerState,
}

impl TestLlmUpstreamServer {
    pub(crate) async fn start() -> Self {
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

        Self { address, state }
    }

    pub(crate) fn openai_base_url(&self) -> String {
        format!("http://{}/v1", self.address)
    }

    pub(crate) fn openrouter_base_url(&self) -> String {
        format!("http://{}/api/v1", self.address)
    }

    pub(crate) fn gemini_base_url(&self) -> String {
        format!("http://{}/v1beta", self.address)
    }

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

async fn openai_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/v1/chat/completions", headers, body.clone());
    if body.get("model").and_then(Value::as_str) == Some("bad-model") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": { "message": "invalid model" } })),
        )
            .into_response();
    }

    Json(json!({
        "id": "openai-req-1",
        "model": body.get("model").and_then(Value::as_str).unwrap_or("gpt-4o-mini"),
        "choices": [{ "message": { "content": "OpenAI says hi" } }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120,
            "prompt_tokens_details": { "cached_tokens": 42 }
        }
    }))
    .into_response()
}

async fn openrouter_handler(
    State(state): State<MockServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    capture_request(&state, "/api/v1/chat/completions", headers, body.clone());
    Json(json!({
        "id": "openrouter-req-1",
        "model": body.get("model").and_then(Value::as_str).unwrap_or("openai/gpt-4o-mini"),
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
    }))
}

fn capture_request(state: &MockServerState, path: &str, headers: HeaderMap, body: Value) {
    state.requests.lock().unwrap().push(CapturedRequest {
        path: path.to_string(),
        authorization: headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string()),
        body,
    });
}
