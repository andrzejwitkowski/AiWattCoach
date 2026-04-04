use crate::{
    config::AppState,
    domain::workout_summary::{validate_message_content, WorkoutSummaryError},
};
use axum::{
    extract::{ws::Message, Path, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures::{future::BoxFuture, FutureExt, SinkExt, StreamExt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, Mutex};

use super::{
    dto::{
        coach_message, coach_typing_message, error_message, system_message, ClientWsMessage,
        WorkoutSummaryPath,
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
    let Some(service) = state.workout_summary_service.clone() else {
        let _ = send_ws_json(
            &sender,
            error_message("workout summary service unavailable"),
        )
        .await;
        return;
    };
    let (queued_messages_tx, mut queued_messages_rx) = mpsc::channel::<String>(MAX_QUEUED_MESSAGES);
    let mut processing_message: Option<BoxFuture<'static, bool>> = None;
    let mut buffered_message: Option<Result<Message, axum::Error>> = None;
    let mut socket_closed = false;

    loop {
        tokio::select! {
            biased;

            should_close = async {
                processing_message
                    .as_mut()
                    .expect("processing future should exist when polled")
                    .await
            }, if processing_message.is_some() => {
                processing_message = None;

                if should_close {
                    connection_open.store(false, Ordering::Relaxed);
                    let _ = close_ws(&sender).await;
                    break;
                }

                tokio::task::yield_now().await;

                if let Some(message_result) = receiver.next().now_or_never() {
                    match message_result {
                        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                            socket_closed = true;
                        }
                        Some(message_result) => {
                            buffered_message = Some(message_result);
                        }
                    }
                }

                if socket_closed {
                    break;
                }
            }

            message_result = async {
                buffered_message
                    .take()
                    .expect("buffered message should exist when polled")
            }, if buffered_message.is_some() => {
                let message = match message_result {
                    Ok(message) => message,
                    Err(_) => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                };

                match handle_socket_message(message, &sender, &queued_messages_tx).await {
                    SocketMessageAction::Continue => {}
                    SocketMessageAction::Close => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                    SocketMessageAction::Break => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                }
            }

            message_result = receiver.next(), if !socket_closed => {
                let Some(message_result) = message_result else {
                    if processing_message.is_some() {
                        socket_closed = true;
                        continue;
                    }

                    break;
                };

                let message = match message_result {
                    Ok(message) => message,
                    Err(_) => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                };

                match handle_socket_message(message, &sender, &queued_messages_tx).await {
                    SocketMessageAction::Continue => {}
                    SocketMessageAction::Close => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                    SocketMessageAction::Break => {
                        if processing_message.is_some() {
                            socket_closed = true;
                            continue;
                        }

                        break;
                    }
                }
            }

            queued_message = queued_messages_rx.recv(), if !socket_closed && processing_message.is_none() => {
                let Some(content) = queued_message else {
                    break;
                };

                // Process one queued user message at a time so typing/reply events stay ordered.
                processing_message = Some(Box::pin(process_send_message(
                    Arc::clone(&sender),
                    Arc::clone(&connection_open),
                    service.clone(),
                    user_id.clone(),
                    workout_id.clone(),
                    content,
                )));
            }
        }
    }

    connection_open.store(false, Ordering::Relaxed);
    drop(queued_messages_tx);
    let _ = close_ws(&sender).await;
}

enum SocketMessageAction {
    Continue,
    Close,
    Break,
}

async fn handle_socket_message(
    message: Message,
    sender: &Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
    queued_messages_tx: &mpsc::Sender<String>,
) -> SocketMessageAction {
    match message {
        Message::Text(text) => {
            let client_message = match serde_json::from_str::<ClientWsMessage>(&text) {
                Ok(message) => message,
                Err(_) => {
                    return if send_ws_json(sender, error_message("invalid websocket payload"))
                        .await
                        .is_err()
                    {
                        SocketMessageAction::Break
                    } else {
                        SocketMessageAction::Continue
                    };
                }
            };

            if client_message.message_type != "send_message" {
                return if send_ws_json(sender, error_message("unsupported websocket message type"))
                    .await
                    .is_err()
                {
                    SocketMessageAction::Break
                } else {
                    SocketMessageAction::Continue
                };
            }

            let Some(content) = client_message.content else {
                return if send_ws_json(sender, error_message("message content is required"))
                    .await
                    .is_err()
                {
                    SocketMessageAction::Break
                } else {
                    SocketMessageAction::Continue
                };
            };

            let content = match validate_message_content(&content) {
                Ok(content) => content,
                Err(error) => {
                    return if send_ws_json(sender, error_message(client_error_message(&error)))
                        .await
                        .is_err()
                    {
                        SocketMessageAction::Break
                    } else {
                        SocketMessageAction::Continue
                    };
                }
            };

            match queued_messages_tx.try_send(content) {
                Ok(()) => SocketMessageAction::Continue,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    if send_ws_json(
                        sender,
                        error_message("too many pending workout summary messages"),
                    )
                    .await
                    .is_err()
                    {
                        SocketMessageAction::Break
                    } else {
                        SocketMessageAction::Continue
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => SocketMessageAction::Break,
            }
        }
        Message::Close(_) => SocketMessageAction::Close,
        Message::Ping(payload) => {
            if sender
                .lock()
                .await
                .send(Message::Pong(payload))
                .await
                .is_err()
            {
                SocketMessageAction::Break
            } else {
                SocketMessageAction::Continue
            }
        }
        _ => SocketMessageAction::Continue,
    }
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

            if persisted.athlete_summary_may_regenerate_before_reply
                && send_ws_json(
                    &sender,
                    system_message("First the summary is being generated - wait a moment"),
                )
                .await
                .is_err()
            {
                return true;
            }

            if send_ws_json(&sender, coach_typing_message()).await.is_err() {
                return true;
            }

            if !connection_open.load(Ordering::Relaxed) {
                return true;
            }

            match service
                .generate_coach_reply(&user_id, &workout_id, persisted.user_message.id.clone())
                .await
            {
                Ok(reply) => {
                    if !connection_open.load(Ordering::Relaxed) {
                        return true;
                    }

                    send_ws_json(
                        &sender,
                        coach_message(
                            map_message_to_dto(reply.coach_message),
                            map_summary_to_dto(reply.summary),
                        ),
                    )
                    .await
                    .is_err()
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
