use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
pub struct CompletedWorkoutBackfillPath {
    user_id: String,
}

#[derive(Deserialize)]
pub struct CompletedWorkoutBackfillQuery {
    oldest: String,
    newest: String,
}

#[derive(Serialize)]
pub struct CompletedWorkoutBackfillResponse {
    scanned: usize,
    enriched: usize,
    skipped: usize,
    failed: usize,
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

pub async fn backfill_completed_workout_details(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CompletedWorkoutBackfillPath>,
    Query(query): Query<CompletedWorkoutBackfillQuery>,
) -> impl IntoResponse {
    if !super::intervals::is_valid_date(&query.oldest)
        || !super::intervals::is_valid_date(&query.newest)
        || query.oldest > query.newest
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(service) = state.completed_workout_admin_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => match service
            .backfill_missing_details(&path.user_id, &query.oldest, &query.newest)
            .await
        {
            Ok(result) => Json(CompletedWorkoutBackfillResponse {
                scanned: result.scanned,
                enriched: result.enriched,
                skipped: result.skipped,
                failed: result.failed,
            })
            .into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
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
