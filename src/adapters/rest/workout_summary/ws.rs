use axum::{
    extract::{ws::Message, Path, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{SinkExt, StreamExt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{sleep, Duration};

use crate::{
    config::AppState,
    domain::workout_summary::{validate_message_content, WorkoutSummaryError},
};

use super::{
    dto::{
        coach_message, coach_typing_message, error_message, ClientWsMessage, WorkoutSummaryPath,
    },
    error::map_workout_summary_error,
    mapping::{map_message_to_dto, map_summary_to_dto},
};

const MAX_QUEUED_MESSAGES: usize = 4;

pub async fn workout_summary_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    Path(path): Path<WorkoutSummaryPath>,
) -> Response {
    match super::handlers::resolve_user_id(&state, &headers).await {
        Ok(user_id) => {
            let Some(service) = state.workout_summary_service.clone() else {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            };

            let state = state.clone();
            let workout_id = path.workout_id;

            match service.get_summary(&user_id, &workout_id).await {
                Ok(_) => {
                    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, workout_id))
                }
                Err(error) => map_workout_summary_error(&error),
            }
        }
        Err(response) => response,
    }
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    user_id: String,
    workout_id: String,
) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let connection_open = Arc::new(AtomicBool::new(true));
    let close_requested = Arc::new(Notify::new());
    let Some(service) = state.workout_summary_service.clone() else {
        let _ = send_ws_json(
            &sender,
            error_message("workout summary service unavailable"),
        )
        .await;
        return;
    };

    let (queued_messages_tx, mut queued_messages_rx) = mpsc::channel::<String>(MAX_QUEUED_MESSAGES);
    let worker_sender = Arc::clone(&sender);
    let worker_service = service.clone();
    let worker_user_id = user_id.clone();
    let worker_workout_id = workout_id.clone();
    let worker_connection_open = Arc::clone(&connection_open);
    let worker_close_requested = Arc::clone(&close_requested);

    tokio::spawn(async move {
        // Process one queued user message at a time so typing/reply events stay ordered.
        while let Some(content) = queued_messages_rx.recv().await {
            if !worker_connection_open.load(Ordering::Relaxed) {
                break;
            }

            if process_send_message(
                Arc::clone(&worker_sender),
                Arc::clone(&worker_connection_open),
                worker_service.clone(),
                worker_user_id.clone(),
                worker_workout_id.clone(),
                content,
            )
            .await
            {
                worker_connection_open.store(false, Ordering::Relaxed);
                let _ = close_ws(&worker_sender).await;
                worker_close_requested.notify_waiters();
                break;
            }

            if !worker_connection_open.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    loop {
        let message_result = tokio::select! {
            _ = close_requested.notified() => break,
            message_result = receiver.next() => message_result,
        };

        let Some(message_result) = message_result else {
            break;
        };

        let message = match message_result {
            Ok(message) => message,
            Err(_) => break,
        };

        match message {
            Message::Text(text) => {
                let client_message = match serde_json::from_str::<ClientWsMessage>(&text) {
                    Ok(message) => message,
                    Err(_) => {
                        if send_ws_json(&sender, error_message("invalid websocket payload"))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                };

                if client_message.message_type != "send_message" {
                    if send_ws_json(&sender, error_message("unsupported websocket message type"))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }

                let Some(content) = client_message.content else {
                    if send_ws_json(&sender, error_message("message content is required"))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    continue;
                };

                let content = match validate_message_content(&content) {
                    Ok(content) => content,
                    Err(error) => {
                        if send_ws_json(&sender, error_message(client_error_message(&error)))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                };

                match queued_messages_tx.try_send(content) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        if send_ws_json(
                            &sender,
                            error_message("too many pending workout summary messages"),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                if sender
                    .lock()
                    .await
                    .send(Message::Pong(payload))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            _ => {}
        }
    }

    connection_open.store(false, Ordering::Relaxed);
    drop(queued_messages_tx);
    let _ = close_ws(&sender).await;
}

async fn send_ws_json(
    sender: &Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
    payload: impl serde::Serialize,
) -> Result<(), axum::Error> {
    let json =
        serde_json::to_string(&payload).expect("serializing websocket payload should not fail");
    sender.lock().await.send(Message::Text(json.into())).await
}

async fn close_ws(
    sender: &Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
) -> Result<(), axum::Error> {
    sender.lock().await.close().await
}

async fn process_send_message(
    sender: Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
    connection_open: Arc<AtomicBool>,
    service: std::sync::Arc<dyn crate::domain::workout_summary::WorkoutSummaryUseCases>,
    user_id: String,
    workout_id: String,
    content: String,
) -> bool {
    if !connection_open.load(Ordering::Relaxed) {
        return true;
    }

    match service
        .append_user_message(&user_id, &workout_id, content)
        .await
    {
        Ok(persisted) => {
            if !connection_open.load(Ordering::Relaxed) {
                return true;
            }

            if send_ws_json(&sender, coach_typing_message()).await.is_err() {
                return true;
            }

            sleep(Duration::from_millis(1500)).await;

            if !connection_open.load(Ordering::Relaxed) {
                return true;
            }

            match service
                .generate_coach_reply(
                    &user_id,
                    &workout_id,
                    persisted.user_message.content.clone(),
                )
                .await
            {
                Ok(reply) => send_ws_json(
                    &sender,
                    coach_message(
                        map_message_to_dto(reply.coach_message),
                        map_summary_to_dto(reply.summary),
                    ),
                )
                .await
                .is_err(),
                Err(error) => {
                    if send_ws_json(&sender, error_message(client_error_message(&error)))
                        .await
                        .is_err()
                    {
                        return true;
                    }

                    should_close_worker(&error)
                }
            }
        }
        Err(error) => {
            if send_ws_json(&sender, error_message(client_error_message(&error)))
                .await
                .is_err()
            {
                return true;
            }

            should_close_worker(&error)
        }
    }
}

fn client_error_message(error: &WorkoutSummaryError) -> String {
    match error {
        WorkoutSummaryError::Repository(_) => "workout summary service unavailable".to_string(),
        _ => error.to_string(),
    }
}

fn should_close_worker(error: &crate::domain::workout_summary::WorkoutSummaryError) -> bool {
    matches!(
        map_workout_summary_error(error).status().as_u16(),
        404 | 409 | 503
    )
}
