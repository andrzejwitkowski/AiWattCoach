use std::time::Duration;

use aiwattcoach::domain::llm::LlmProvider;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use tower::util::ServiceExt;

use crate::support::{ai_config, get_json, llm_rest_test_context};

#[tokio::test]
async fn send_message_uses_saved_openrouter_settings_through_live_adapter() {
    let context = llm_rest_test_context().await;
    let mut settings = context.default_settings();
    settings.ai_agents = ai_config(
        LlmProvider::OpenRouter,
        "openai/gpt-4o-mini",
        "or-saved-key",
    );
    context.seed_user_settings(settings);
    context.seed_summary(context.default_summary("workout-1"));

    let response = context
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workout-summaries/workout-1/messages")
                .header(header::COOKIE, context.session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"content":"Legs felt heavy today"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("coachMessage")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_str),
        Some("OpenRouter says hi")
    );

    let requests = context.server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/chat/completions");
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer or-saved-key")
    );
    assert_eq!(requests[0].body["model"], "openai/gpt-4o-mini");
    let messages = requests[0].body["messages"]
        .as_array()
        .expect("messages should be an array");
    assert!(messages.iter().any(|message| {
        message["content"]
            .as_str()
            .is_some_and(|content| content.contains("training_context_stable="))
    }));
    assert!(messages.iter().any(|message| {
        message["content"]
            .as_str()
            .is_some_and(|content| content.contains("training_context_volatile="))
    }));
}

#[tokio::test]
async fn workout_summary_websocket_creates_and_reuses_gemini_cache() {
    let context = llm_rest_test_context().await;
    let mut settings = context.default_settings();
    settings.ai_agents = ai_config(LlmProvider::Gemini, "gemini-2.5-flash", "gemini-key");
    context.seed_user_settings(settings);
    context.seed_summary(context.default_summary("workout-1"));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = context.app.clone();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut request = format!("ws://{address}/api/workout-summaries/workout-1/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Cookie", context.session_cookie("session-1"));

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"First turn"}"#.to_string().into(),
        ))
        .await
        .unwrap();
    let _ = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap();
    let first_reply = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(first_reply.contains(r#""type":"coach_message""#));

    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Second turn"}"#.to_string().into(),
        ))
        .await
        .unwrap();
    let _ = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap();
    let second_reply = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(second_reply.contains(r#""type":"coach_message""#));

    let requests = context.server.requests();
    let cache_creates: Vec<_> = requests
        .iter()
        .filter(|request| request.path == "/v1beta/cachedContents")
        .collect();
    let generates: Vec<_> = requests
        .iter()
        .filter(|request| request.path == "/v1beta/models/gemini-2.5-flash:generateContent")
        .collect();

    assert_eq!(cache_creates.len(), 1);
    assert_eq!(generates.len(), 2);
    assert_eq!(generates[0].body["cachedContent"], "cachedContents/cache-1");
    assert_eq!(generates[1].body["cachedContent"], "cachedContents/cache-1");
}
