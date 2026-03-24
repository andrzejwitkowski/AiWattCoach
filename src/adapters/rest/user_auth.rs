use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::field;

use crate::config::AppState;

use super::cookies::read_cookie;

pub(super) async fn resolve_user_id(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, Response> {
    let identity_service = state
        .identity_service
        .as_ref()
        .ok_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())?;

    let session_id = read_cookie(headers, &state.session_cookie_name)
        .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

    let user = identity_service
        .get_current_user(&session_id)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())?
        .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())?;

    tracing::Span::current().record("user_id", field::display(&user.id));

    Ok(user.id)
}
