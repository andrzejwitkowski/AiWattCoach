use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::config::AppState;
use crate::domain::admin_prompt_preview::AdminPromptPreviewError;
use crate::domain::identity::IdentityError;

use super::cookies::read_cookie;

#[derive(Deserialize)]
pub struct AdminPromptPreviewPath {
    user_id: String,
}

#[derive(Deserialize)]
pub struct AdminPromptPreviewQuery {
    date: String,
}

pub async fn preview_post_workout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AdminPromptPreviewPath>,
    Query(query): Query<AdminPromptPreviewQuery>,
) -> impl IntoResponse {
    preview_with_surface(
        state,
        headers,
        path,
        query,
        PreviewSurfaceRoute::PostWorkout,
    )
    .await
}

pub async fn preview_calendar_coach(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AdminPromptPreviewPath>,
    Query(query): Query<AdminPromptPreviewQuery>,
) -> impl IntoResponse {
    preview_with_surface(
        state,
        headers,
        path,
        query,
        PreviewSurfaceRoute::CalendarCoach,
    )
    .await
}

pub async fn preview_meso_cycle_coach(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<AdminPromptPreviewPath>,
    Query(query): Query<AdminPromptPreviewQuery>,
) -> impl IntoResponse {
    preview_with_surface(
        state,
        headers,
        path,
        query,
        PreviewSurfaceRoute::MesoCycleCoach,
    )
    .await
}

enum PreviewSurfaceRoute {
    PostWorkout,
    CalendarCoach,
    MesoCycleCoach,
}

async fn preview_with_surface(
    state: AppState,
    headers: HeaderMap,
    path: AdminPromptPreviewPath,
    query: AdminPromptPreviewQuery,
    surface: PreviewSurfaceRoute,
) -> axum::response::Response {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => {}
        Err(IdentityError::Unauthenticated) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(IdentityError::Forbidden) => return StatusCode::FORBIDDEN.into_response(),
        Err(IdentityError::Repository(_) | IdentityError::External(_)) => {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let Some(service) = state.admin_prompt_preview_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let result = match surface {
        PreviewSurfaceRoute::PostWorkout => {
            service
                .preview_post_workout(&path.user_id, &query.date)
                .await
        }
        PreviewSurfaceRoute::CalendarCoach => {
            service
                .preview_calendar_coach(&path.user_id, &query.date)
                .await
        }
        PreviewSurfaceRoute::MesoCycleCoach => {
            service
                .preview_meso_cycle_coach(&path.user_id, &query.date)
                .await
        }
    };

    match result {
        Ok(response) => Json(response).into_response(),
        Err(AdminPromptPreviewError::InvalidDate | AdminPromptPreviewError::FutureDate) => {
            StatusCode::BAD_REQUEST.into_response()
        }
        Err(AdminPromptPreviewError::NoCompletedWorkoutForDate) => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(
            AdminPromptPreviewError::Settings(_)
            | AdminPromptPreviewError::Repository(_)
            | AdminPromptPreviewError::TargetResolution(_)
            | AdminPromptPreviewError::Llm(_)
            | AdminPromptPreviewError::MesoCycle(_),
        ) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
