use axum::{
    extract::{ws::Message, Path, State, WebSocketUpgrade},
    http::HeaderMap,
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::config::AppState;

use super::{
    dto::{
        coach_message, coach_typing_message, error_message, ClientWsMessage, WorkoutSummaryPath,
    },
    error::map_workout_summary_error,
    mapping::{map_message_to_dto, map_summary_to_dto},
};

pub async fn workout_summary_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    Path(path): Path<WorkoutSummaryPath>,
) -> Response {
    match super::handlers::resolve_user_id(&state, &headers).await {
        Ok(user_id) => {
            let state = state.clone();
            let event_id = path.event_id;
            ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, event_id))
        }
        Err(response) => response,
    }
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
    user_id: String,
    event_id: String,
) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let Some(service) = state.workout_summary_service.clone() else {
        let _ = send_ws_json(
            &sender,
            error_message("workout summary service unavailable"),
        )
        .await;
        return;
    };

    let (queued_messages_tx, mut queued_messages_rx) = mpsc::unbounded_channel::<String>();
    let worker_sender = Arc::clone(&sender);
    let worker_service = service.clone();
    let worker_user_id = user_id.clone();
    let worker_event_id = event_id.clone();

    tokio::spawn(async move {
        // Process one queued user message at a time so typing/reply events stay ordered.
        while let Some(content) = queued_messages_rx.recv().await {
            if process_send_message(
                Arc::clone(&worker_sender),
                worker_service.clone(),
                worker_user_id.clone(),
                worker_event_id.clone(),
                content,
            )
            .await
            {
                break;
            }
        }
    });

    while let Some(message_result) = receiver.next().await {
        let message = match message_result {
            Ok(message) => message,
            Err(_) => break,
        };

        match message {
            Message::Text(text) => {
                let client_message = match serde_json::from_str::<ClientWsMessage>(&text) {
                    Ok(message) => message,
                    Err(_) => {
                        let _ =
                            send_ws_json(&sender, error_message("invalid websocket payload")).await;
                        continue;
                    }
                };

                if client_message.message_type != "send_message" {
                    let _ =
                        send_ws_json(&sender, error_message("unsupported websocket message type"))
                            .await;
                    continue;
                }

                let Some(content) = client_message.content else {
                    let _ =
                        send_ws_json(&sender, error_message("message content is required")).await;
                    continue;
                };

                if queued_messages_tx.send(content).is_err() {
                    break;
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
}

async fn send_ws_json(
    sender: &Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
    payload: impl serde::Serialize,
) -> Result<(), axum::Error> {
    let json =
        serde_json::to_string(&payload).expect("serializing websocket payload should not fail");
    sender.lock().await.send(Message::Text(json.into())).await
}

async fn process_send_message(
    sender: Arc<Mutex<futures::stream::SplitSink<axum::extract::ws::WebSocket, Message>>>,
    service: std::sync::Arc<dyn crate::domain::workout_summary::WorkoutSummaryUseCases>,
    user_id: String,
    event_id: String,
    content: String,
) -> bool {
    match service
        .append_user_message(&user_id, &event_id, content)
        .await
    {
        Ok(persisted) => {
            let _ = send_ws_json(&sender, coach_typing_message()).await;
            sleep(Duration::from_millis(1500)).await;

            match service
                .generate_coach_reply(&user_id, &event_id, persisted.user_message.content.clone())
                .await
            {
                Ok(reply) => {
                    let _ = send_ws_json(
                        &sender,
                        coach_message(
                            map_message_to_dto(reply.coach_message),
                            map_summary_to_dto(reply.summary),
                        ),
                    )
                    .await;
                    false
                }
                Err(error) => {
                    let _ = send_ws_json(&sender, error_message(error.to_string())).await;
                    matches!(
                        map_workout_summary_error(&error).status().as_u16(),
                        404 | 503
                    )
                }
            }
        }
        Err(error) => {
            let _ = send_ws_json(&sender, error_message(error.to_string())).await;
            matches!(
                map_workout_summary_error(&error).status().as_u16(),
                404 | 503
            )
        }
    }
}
