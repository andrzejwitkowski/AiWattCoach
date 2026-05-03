use crate::{
    config::AppState,
    domain::coach_conversation::{validate_conversation_message_content, CoachConversationError},
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
        coach_message, coach_typing_message, error_message, tool_message,
        CalendarCoachConversationPath, ClientWsMessage,
    },
    error::map_calendar_coach_error,
    mapping::{map_conversation_to_dto, map_message_to_dto},
};

const MAX_QUEUED_MESSAGES: usize = 4;

pub async fn calendar_coach_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    Path(path): Path<CalendarCoachConversationPath>,
) -> Response {
    match super::handlers::resolve_user_id(&state, &headers).await {
        Ok(user_id) => {
            let Some(service) = state.calendar_coach_service.clone() else {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            };

            let state = state.clone();
            let conversation_id = path.conversation_id;

            match service.get_conversation(&user_id, &conversation_id).await {
                Ok(_) => ws.on_upgrade(move |socket| {
                    handle_socket(socket, state, user_id, conversation_id)
                }),
                Err(error) => map_calendar_coach_error(&error),
            }
        }
        Err(response) => response,
    }
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    user_id: String,
    conversation_id: String,
) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let connection_open = Arc::new(AtomicBool::new(true));
    let Some(service) = state.calendar_coach_service.clone() else {
        let _ = send_ws_json(&sender, error_message("calendar coach service unavailable")).await;
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

                processing_message = Some(Box::pin(process_send_message(
                    Arc::clone(&sender),
                    Arc::clone(&connection_open),
                    service.clone(),
                    user_id.clone(),
                    conversation_id.clone(),
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

            let content = match validate_conversation_message_content(&content) {
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
                        error_message("too many pending calendar coach messages"),
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
    service: std::sync::Arc<dyn crate::domain::calendar_coach::CalendarCoachUseCases>,
    user_id: String,
    conversation_id: String,
    content: String,
) -> bool {
    if !connection_open.load(Ordering::Relaxed) {
        return true;
    }

    match service
        .append_user_message(&user_id, &conversation_id, content)
        .await
    {
        Ok(persisted) => {
            if !connection_open.load(Ordering::Relaxed) {
                return true;
            }

            if send_ws_json(&sender, coach_typing_message()).await.is_err() {
                return true;
            }

            if !connection_open.load(Ordering::Relaxed) {
                return true;
            }

            match service
                .generate_reply(
                    &user_id,
                    &conversation_id,
                    persisted.user_message.id.clone(),
                )
                .await
            {
                Ok(reply) => {
                    if !connection_open.load(Ordering::Relaxed) {
                        return true;
                    }

                    for message in current_turn_tool_messages(
                        &reply.messages,
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

                    send_ws_json(
                        &sender,
                        coach_message(
                            map_message_to_dto(reply.coach_message),
                            map_conversation_to_dto(reply.conversation),
                            reply.messages.into_iter().map(map_message_to_dto).collect(),
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

fn current_turn_tool_messages(
    messages: &[crate::domain::coach_conversation::CoachConversationMessage],
    user_message_id: &str,
    coach_message_id: &str,
) -> Vec<crate::domain::coach_conversation::CoachConversationMessage> {
    let Some(user_index) = messages
        .iter()
        .position(|message| message.id == user_message_id)
    else {
        return Vec::new();
    };
    let Some(coach_index) = messages
        .iter()
        .position(|message| message.id == coach_message_id)
    else {
        return Vec::new();
    };
    if coach_index <= user_index {
        return Vec::new();
    }

    messages[user_index + 1..coach_index]
        .iter()
        .filter(|message| {
            message.role == crate::domain::coach_conversation::CoachConversationMessageRole::Tool
        })
        .cloned()
        .collect()
}

fn client_error_message(error: &CoachConversationError) -> String {
    match error {
        CoachConversationError::Repository(_) => "calendar coach service unavailable".to_string(),
        _ => error.to_string(),
    }
}

fn should_close_worker(error: &CoachConversationError) -> bool {
    match error {
        CoachConversationError::NotFound
        | CoachConversationError::Archived
        | CoachConversationError::ReplyAlreadyPending => true,
        CoachConversationError::Llm(llm_error) => {
            !matches!(llm_error, crate::domain::llm::LlmError::ContextTooLarge(_))
                && llm_error.is_retryable()
        }
        CoachConversationError::Repository(_) => true,
        CoachConversationError::Validation(_) => false,
    }
}
