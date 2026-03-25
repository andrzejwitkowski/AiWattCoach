use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use tracing::field;

use crate::config::AppState;

use super::cookies::read_cookie;

fn pseudonymize_user_id(user_id: &str) -> String {
    let hash = Sha256::digest(user_id.as_bytes());
    format!("{hash:x}")[..16].to_string()
}

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

    tracing::Span::current().record("user_id", field::display(pseudonymize_user_id(&user.id)));

    Ok(user.id)
}
