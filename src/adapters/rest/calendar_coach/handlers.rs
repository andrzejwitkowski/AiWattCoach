use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::{config::AppState, domain::calendar_coach::CalendarCoachUseCases};

use super::{
    dto::{CalendarCoachConversationPath, SendMessageRequest},
    error::map_calendar_coach_error,
    mapping::{map_conversation_response, map_send_message_result},
};

pub(super) async fn resolve_user_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, Response> {
    super::super::user_auth::resolve_user_id(state, headers).await
}

async fn resolve_calendar_coach(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(String, Arc<dyn CalendarCoachUseCases>), Response> {
    let user_id = super::super::user_auth::resolve_user_id(state, headers).await?;
    let service = state
        .calendar_coach_service
        .clone()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;
    Ok((user_id, service))
}

pub async fn get_current_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (user_id, service) = match resolve_calendar_coach(&state, &headers).await {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };

    match service.get_current_conversation(&user_id).await {
        Ok((conversation, messages)) => {
            Json(map_conversation_response(conversation, messages)).into_response()
        }
        Err(error) => map_calendar_coach_error(&error),
    }
}

pub async fn start_new_conversation(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (user_id, service) = match resolve_calendar_coach(&state, &headers).await {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };

    match service.start_new_conversation(&user_id).await {
        Ok((conversation, messages)) => (
            StatusCode::CREATED,
            Json(map_conversation_response(conversation, messages)),
        )
            .into_response(),
        Err(error) => map_calendar_coach_error(&error),
    }
}

pub async fn get_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CalendarCoachConversationPath>,
) -> Response {
    let (user_id, service) = match resolve_calendar_coach(&state, &headers).await {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };

    match service
        .get_conversation(&user_id, &path.conversation_id)
        .await
    {
        Ok((conversation, messages)) => {
            Json(map_conversation_response(conversation, messages)).into_response()
        }
        Err(error) => map_calendar_coach_error(&error),
    }
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CalendarCoachConversationPath>,
    Json(body): Json<SendMessageRequest>,
) -> Response {
    let (user_id, service) = match resolve_calendar_coach(&state, &headers).await {
        Ok(resolved) => resolved,
        Err(response) => return response,
    };

    match service
        .send_message(&user_id, &path.conversation_id, body.content)
        .await
    {
        Ok(result) => Json(map_send_message_result(result)).into_response(),
        Err(error) => map_calendar_coach_error(&error),
    }
}
