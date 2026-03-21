use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::config::AppState;
use crate::domain::identity::IdentityError;

use super::cookies::read_cookie;

#[derive(Serialize)]
pub struct SystemInfoResponse {
    #[serde(rename = "appName")]
    app_name: String,
    #[serde(rename = "mongoDatabase")]
    mongo_database: String,
}

pub async fn system_info(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => Json(SystemInfoResponse {
            app_name: state.app_name,
            mongo_database: state.mongo_database,
        })
        .into_response(),
        Err(IdentityError::Unauthenticated) => StatusCode::UNAUTHORIZED.into_response(),
        Err(crate::domain::identity::IdentityError::Forbidden) => {
            StatusCode::FORBIDDEN.into_response()
        }
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
