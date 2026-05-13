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
        coach_message, coach_typing_message, error_message, save_workflow_message, system_message,
        tool_message, ClientWsMessage, WorkoutSummaryPath,
    },
    error::map_workout_summary_error,
    mapping::{map_message_to_dto, map_summary_to_dto},
};

const MAX_QUEUED_MESSAGES: usize = 4;
const COACH_REPLY_KEEPALIVE_INTERVAL_SECONDS: u64 = 15;

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

    if let Some((notifier, rx)) = state.workout_summary_save_notifier.clone().map(|notifier| {
        let rx = notifier.register(&user_id, &workout_id);
        (notifier, rx)
    }) {
        if rx.borrow().is_some() {
            let payload = rx.borrow().clone().unwrap();
            let _ = send_ws_json(&sender, save_workflow_message(payload)).await;
            notifier.unregister(&user_id, &workout_id);
        } else {
            let sender = Arc::clone(&sender);
            let user_id = user_id.clone();
            let workout_id = workout_id.clone();
            tokio::spawn(async move {
                let mut rx = rx;
                if rx.changed().await.is_ok() {
                    let payload_opt = rx.borrow().clone();
                    if let Some(payload) = payload_opt {
                        let _ = send_ws_json(&sender, save_workflow_message(payload)).await;
                        notifier.unregister(&user_id, &workout_id);
                    }
                }
            });
        }
    }

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

enum CoachReplyWaitOutcome<Reply, Error> {
    Completed(Result<Reply, Error>),
    TaskDropped,
    ConnectionClosed,
    KeepaliveSendFailed,
}

async fn wait_for_coach_reply_result<Reply, Error, Keepalive>(
    connection_open: Arc<AtomicBool>,
    result_rx: &mut tokio::sync::oneshot::Receiver<Result<Reply, Error>>,
    mut send_keepalive: Keepalive,
) -> CoachReplyWaitOutcome<Reply, Error>
where
    Keepalive: FnMut() -> BoxFuture<'static, bool>,
{
    loop {
        if !connection_open.load(Ordering::Relaxed) {
            return CoachReplyWaitOutcome::ConnectionClosed;
        }

        match tokio::time::timeout(
            std::time::Duration::from_secs(COACH_REPLY_KEEPALIVE_INTERVAL_SECONDS),
            &mut *result_rx,
        )
        .await
        {
            Ok(Ok(result)) => return CoachReplyWaitOutcome::Completed(result),
            Ok(Err(_)) => return CoachReplyWaitOutcome::TaskDropped,
            Err(_elapsed) => {
                if send_keepalive().await {
                    return CoachReplyWaitOutcome::KeepaliveSendFailed;
                }
            }
        }
    }
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

            let (result_tx, mut result_rx) = tokio::sync::oneshot::channel();
            let service_clone = service.clone();
            let uid = user_id.clone();
            let wid = workout_id.clone();
            let umid = persisted.user_message.id.clone();
            tokio::spawn(async move {
                let reply_result = service_clone.generate_coach_reply(&uid, &wid, umid).await;
                let _ = result_tx.send(reply_result);
            });

            let keepalive_sender = Arc::clone(&sender);
            match wait_for_coach_reply_result(
                Arc::clone(&connection_open),
                &mut result_rx,
                move || {
                    let sender = Arc::clone(&keepalive_sender);
                    Box::pin(
                        async move { send_ws_json(&sender, coach_typing_message()).await.is_err() },
                    )
                },
            )
            .await
            {
                CoachReplyWaitOutcome::Completed(Ok(reply)) => {
                    if !connection_open.load(Ordering::Relaxed) {
                        return true;
                    }

                    for message in current_turn_tool_messages(
                        &reply.summary,
                        &persisted.user_message.id,
                        &reply.coach_message.id,
                    ) {
                        if send_ws_json(&sender, tool_message(map_message_to_dto(message)))
                            .await
                            .is_err()
                        {
                            return true;
                        }
                    }

                    return send_ws_json(
                        &sender,
                        coach_message(
                            map_message_to_dto(reply.coach_message),
                            map_summary_to_dto(reply.summary),
                        ),
                    )
                    .await
                    .is_err();
                }
                CoachReplyWaitOutcome::Completed(Err(error)) => {
                    if send_ws_json(&sender, error_message(client_error_message(&error)))
                        .await
                        .is_err()
                    {
                        return true;
                    }

                    should_close_worker(&error)
                }
                CoachReplyWaitOutcome::TaskDropped => {
                    if send_ws_json(
                        &sender,
                        error_message("workout summary coach reply task failed unexpectedly"),
                    )
                    .await
                    .is_err()
                    {
                        return true;
                    }

                    true
                }
                CoachReplyWaitOutcome::ConnectionClosed
                | CoachReplyWaitOutcome::KeepaliveSendFailed => true,
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

fn current_turn_tool_messages(
    summary: &crate::domain::workout_summary::WorkoutSummary,
    user_message_id: &str,
    coach_message_id: &str,
) -> Vec<crate::domain::workout_summary::ConversationMessage> {
    let Some(user_index) = summary
        .messages
        .iter()
        .position(|message| message.id == user_message_id)
    else {
        return Vec::new();
    };
    let Some(coach_index) = summary
        .messages
        .iter()
        .position(|message| message.id == coach_message_id)
    else {
        return Vec::new();
    };
    if coach_index <= user_index {
        return Vec::new();
    }

    summary.messages[user_index + 1..coach_index]
        .iter()
        .filter(|message| message.role == crate::domain::workout_summary::MessageRole::Tool)
        .cloned()
        .collect()
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use tokio::sync::oneshot;
    use tokio::time::advance;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn wait_for_coach_reply_result_emits_keepalive_before_completed_reply() {
        let connection_open = Arc::new(AtomicBool::new(true));
        let (result_tx, mut result_rx) = oneshot::channel::<Result<&'static str, &'static str>>();
        let keepalive_count = Arc::new(AtomicUsize::new(0));

        let waiter_connection = Arc::clone(&connection_open);
        let waiter_keepalive_count = Arc::clone(&keepalive_count);
        let waiter = tokio::spawn(async move {
            wait_for_coach_reply_result(waiter_connection, &mut result_rx, move || {
                let keepalive_count = Arc::clone(&waiter_keepalive_count);
                Box::pin(async move {
                    keepalive_count.fetch_add(1, Ordering::Relaxed);
                    false
                })
            })
            .await
        });

        tokio::task::yield_now().await;
        advance(Duration::from_secs(COACH_REPLY_KEEPALIVE_INTERVAL_SECONDS)).await;
        tokio::task::yield_now().await;

        assert_eq!(keepalive_count.load(Ordering::Relaxed), 1);

        result_tx.send(Ok("reply")).unwrap();

        let outcome = waiter.await.unwrap();
        match outcome {
            CoachReplyWaitOutcome::Completed(Ok(reply)) => assert_eq!(reply, "reply"),
            _ => panic!("expected completed reply outcome"),
        }
    }
}
