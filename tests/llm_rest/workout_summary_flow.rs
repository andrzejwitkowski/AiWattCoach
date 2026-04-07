use std::time::Duration;

use aiwattcoach::domain::athlete_summary::AthleteSummary;
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
    context.seed_activity(context.default_activity("user-1", "workout-1"));

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
    let message_contains = |needle: &str| {
        messages.iter().any(|message| {
            message["content"]
                .as_str()
                .is_some_and(|content| content.contains(needle))
                || message["content"].as_array().is_some_and(|parts| {
                    parts.iter().any(|part| {
                        part["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(needle))
                    })
                })
        })
    };

    assert!(message_contains("training_context_stable="));
    assert!(message_contains("training_context_volatile="));
    assert!(message_contains("\"pc\":"));
    assert!(!message_contains("\"p5\":"));
}

#[tokio::test]
async fn workout_summary_websocket_creates_and_reuses_gemini_cache() {
    let context = llm_rest_test_context().await;
    let mut settings = context.default_settings();
    settings.ai_agents = ai_config(LlmProvider::Gemini, "gemini-2.5-flash", "gemini-key");
    context.seed_user_settings(settings);
    context.seed_summary(context.default_summary("workout-1"));
    context.seed_athlete_summary(
        "user-1",
        Some(AthleteSummary {
            user_id: "user-1".to_string(),
            summary_text: "Fresh athlete summary".to_string(),
            generated_at_epoch_seconds: 1_700_000_000,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            provider: Some("gemini".to_string()),
            model: Some("gemini-2.5-flash".to_string()),
        }),
        false,
    );

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

#[tokio::test]
async fn workout_summary_websocket_sends_system_message_before_reply_when_summary_generation_is_needed(
) {
    let context = llm_rest_test_context().await;
    let mut settings = context.default_settings();
    settings.ai_agents = ai_config(
        LlmProvider::OpenRouter,
        "google/gemini-3-flash-preview",
        "or-saved-key",
    );
    settings.intervals.api_key = Some("intervals-key".to_string());
    settings.intervals.athlete_id = Some("i248035".to_string());
    context.seed_user_settings(settings);
    context.seed_summary(context.default_summary("workout-1"));
    context.seed_athlete_summary("user-1", None, true);

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

    let first = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(first.contains(r#""type":"system_message""#));
    assert!(first.contains("First the summary is being generated - wait a moment"));

    let second = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(second.contains(r#""type":"coach_typing""#));
}

#[tokio::test]
async fn workout_summary_websocket_skips_system_message_when_athlete_summary_is_fresh() {
    let context = llm_rest_test_context().await;
    let mut settings = context.default_settings();
    settings.ai_agents = ai_config(
        LlmProvider::OpenRouter,
        "google/gemini-3-flash-preview",
        "or-saved-key",
    );
    settings.intervals.api_key = Some("intervals-key".to_string());
    settings.intervals.athlete_id = Some("i248035".to_string());
    context.seed_user_settings(settings);
    context.seed_summary(context.default_summary("workout-1"));
    context.seed_athlete_summary(
        "user-1",
        Some(AthleteSummary {
            user_id: "user-1".to_string(),
            summary_text: "Fresh athlete summary".to_string(),
            generated_at_epoch_seconds: 1_700_000_000,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            provider: Some("openrouter".to_string()),
            model: Some("google/gemini-3-flash-preview".to_string()),
        }),
        false,
    );

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

    let typing = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(typing.contains(r#""type":"coach_typing""#));

    let reply = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    assert!(reply.contains(r#""type":"coach_message""#));
    assert!(!reply.contains(r#""type":"system_message""#));
}
