use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

use crate::shared::{
    sample_summary, workout_summary_test_app, TestIdentityServiceWithSession,
    TestWorkoutSummaryService,
};

#[tokio::test]
async fn websocket_requires_authentication() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let result = connect_async(format!("ws://{address}/api/workout-summaries/event-1/ws")).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn websocket_sends_typing_then_coach_message() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut request = format!("ws://{address}/api/workout-summaries/event-1/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Legs felt heavy today"}"#
                .to_string()
                .into(),
        ))
        .await
        .unwrap();

    let first = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let second = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    let first_text = first.into_text().unwrap().to_string();
    let second_text = second.into_text().unwrap().to_string();

    assert!(first_text.contains(r#""type":"coach_typing""#));
    assert!(second_text.contains(r#""type":"coach_message""#));
    assert!(second_text.contains(r#""role":"coach""#));
}

#[tokio::test]
async fn websocket_queues_multiple_user_messages_in_order() {
    let app = workout_summary_test_app(
        TestIdentityServiceWithSession::default(),
        TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]),
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut request = format!("ws://{address}/api/workout-summaries/event-1/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"First"}"#.to_string().into(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Second"}"#.to_string().into(),
        ))
        .await
        .unwrap();

    let first = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let second = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let third = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let fourth = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();

    assert!(first.contains(r#""type":"coach_typing""#));
    assert!(second.contains(r#""type":"coach_message""#));
    assert!(third.contains(r#""type":"coach_typing""#));
    assert!(fourth.contains(r#""type":"coach_message""#));
}

#[tokio::test]
async fn websocket_rejects_messages_when_queue_is_full() {
    let service = TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]);
    let app = workout_summary_test_app(TestIdentityServiceWithSession::default(), service).await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut request = format!("ws://{address}/api/workout-summaries/event-1/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();

    for label in ["one", "two", "three", "four", "five", "six"] {
        socket
            .send(Message::Text(
                format!(r#"{{"type":"send_message","content":"{label}"}}"#).into(),
            ))
            .await
            .unwrap();
    }

    let mut saw_queue_error = false;
    for _ in 0..6 {
        let frame = timeout(Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = frame.into_text().unwrap();
        let payload: Value = serde_json::from_str(text.as_ref()).unwrap();

        if payload.get("type").and_then(Value::as_str) == Some("error") {
            assert_eq!(
                payload.get("error").and_then(Value::as_str),
                Some("too many pending workout summary messages")
            );
            saw_queue_error = true;
            break;
        }
    }

    assert!(saw_queue_error);
}

#[tokio::test]
async fn websocket_disconnect_does_not_generate_queued_follow_up_replies() {
    let service = TestWorkoutSummaryService::with_summaries(vec![sample_summary("event-1")]);
    let app =
        workout_summary_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut request = format!("ws://{address}/api/workout-summaries/event-1/ws")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"First"}"#.to_string().into(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Second"}"#.to_string().into(),
        ))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    socket.close(None).await.unwrap();

    tokio::time::sleep(Duration::from_millis(1800)).await;

    let summary = service.summary("user-1", "event-1").unwrap();
    assert_eq!(summary.messages.len(), 1);
    assert_eq!(summary.messages[0].content, "First");
}
