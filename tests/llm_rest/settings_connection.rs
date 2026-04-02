use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::support::{get_json, llm_rest_test_context};

#[tokio::test]
async fn ai_settings_test_uses_live_openrouter_adapter_and_auth_header() {
    let context = llm_rest_test_context().await;

    let response = context
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, context.session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"openrouterApiKey":"or-key-123456","selectedProvider":"openrouter","selectedModel":"openai/gpt-4o-mini"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("connected").and_then(Value::as_bool), Some(true));

    let requests = context.server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/chat/completions");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer or-key-123456")
    );
    assert_eq!(requests[0].body["model"], "openai/gpt-4o-mini");
}

#[tokio::test]
async fn ai_settings_test_maps_live_provider_rejection_to_bad_request() {
    let context = llm_rest_test_context().await;

    let response = context
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, context.session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"openaiApiKey":"sk-test-key","selectedProvider":"openai","selectedModel":"bad-model"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("connected").and_then(Value::as_bool), Some(false));
    assert!(body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .contains("invalid model"));

    let requests = context.server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/chat/completions");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer sk-test-key")
    );
}
