use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::config::AppState;
use crate::domain::completed_workouts::CompletedWorkoutError;
use crate::domain::identity::IdentityError;
use crate::domain::wahoo::WahooWebhookError;

use super::cookies::read_cookie;
use super::same_origin::request_has_same_origin;

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

#[derive(Deserialize)]
pub struct CompletedWorkoutMetricsBackfillQuery {
    oldest: Option<String>,
    newest: Option<String>,
}

#[derive(Serialize)]
pub struct CompletedWorkoutBackfillResponse {
    scanned: usize,
    enriched: usize,
    skipped: usize,
    failed: usize,
}

#[derive(Serialize)]
pub struct CompletedWorkoutMetricsBackfillResponse {
    scanned: usize,
    enriched: usize,
    skipped: usize,
    failed: usize,
    #[serde(rename = "recomputedFrom")]
    recomputed_from: Option<String>,
}

#[derive(Serialize)]
pub struct ManualWahooSyncResponse {
    scanned: usize,
    imported: usize,
    skipped: usize,
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
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(service) = state.completed_workout_admin_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    if !request_has_same_origin(&headers, state.trust_proxy_headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match identity_service.require_admin(&session_id).await {
        Ok(_) => {
            if !super::intervals::is_valid_date(&query.oldest)
                || !super::intervals::is_valid_date(&query.newest)
                || query.oldest > query.newest
            {
                return StatusCode::BAD_REQUEST.into_response();
            }

            match service
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
                Err(error) => {
                    let status = map_backfill_error_status(&error);
                    error!(
                        user_id = %path.user_id,
                        oldest = %query.oldest,
                        newest = %query.newest,
                        error = %error,
                        status = status.as_u16(),
                        "backfill_missing_details failed"
                    );
                    status.into_response()
                }
            }
        }
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

pub async fn backfill_completed_workout_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CompletedWorkoutBackfillPath>,
    Query(query): Query<CompletedWorkoutMetricsBackfillQuery>,
) -> impl IntoResponse {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(service) = state.completed_workout_admin_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    if !request_has_same_origin(&headers, state.trust_proxy_headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let valid_range = match (&query.oldest, &query.newest) {
        (None, None) => true,
        (Some(oldest), Some(newest)) => {
            super::intervals::is_valid_date(oldest)
                && super::intervals::is_valid_date(newest)
                && oldest <= newest
        }
        _ => false,
    };

    match identity_service.require_admin(&session_id).await {
        Ok(_) => {
            if !valid_range {
                return StatusCode::BAD_REQUEST.into_response();
            }

            match service
                .backfill_missing_metrics(
                    &path.user_id,
                    query.oldest.as_deref(),
                    query.newest.as_deref(),
                )
                .await
            {
                Ok(result) => Json(CompletedWorkoutMetricsBackfillResponse {
                    scanned: result.scanned,
                    enriched: result.enriched,
                    skipped: result.skipped,
                    failed: result.failed,
                    recomputed_from: result.recomputed_from,
                })
                .into_response(),
                Err(error) => {
                    let status = map_backfill_error_status(&error);
                    error!(
                        user_id = %path.user_id,
                        oldest = ?query.oldest,
                        newest = ?query.newest,
                        error = %error,
                        status = status.as_u16(),
                        "backfill_missing_metrics failed"
                    );
                    status.into_response()
                }
            }
        }
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

pub async fn sync_wahoo_completed_workouts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path): Path<CompletedWorkoutBackfillPath>,
) -> impl IntoResponse {
    let Some(identity_service) = state.identity_service.clone() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let Some(session_id) = read_cookie(&headers, &state.session_cookie_name) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    let Some(service) = state.wahoo_webhook_service.as_ref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    if !request_has_same_origin(&headers, state.trust_proxy_headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match identity_service.require_admin(&session_id).await {
        Ok(_) => match service
            .sync_completed_workouts_for_user(&path.user_id)
            .await
        {
            Ok(result) => Json(ManualWahooSyncResponse {
                scanned: result.scanned,
                imported: result.imported,
                skipped: result.skipped,
            })
            .into_response(),
            Err(error) => {
                let status = map_wahoo_sync_error_status(&error);
                error!(
                    user_id = %path.user_id,
                    error = %error,
                    status = status.as_u16(),
                    "manual Wahoo completed workout sync failed"
                );
                status.into_response()
            }
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

fn map_backfill_error_status(error: &CompletedWorkoutError) -> StatusCode {
    match error {
        CompletedWorkoutError::Repository(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn map_wahoo_sync_error_status(error: &WahooWebhookError) -> StatusCode {
    match error {
        WahooWebhookError::Unauthorized => StatusCode::UNAUTHORIZED,
        WahooWebhookError::InvalidPayload(_) => StatusCode::BAD_REQUEST,
        WahooWebhookError::NotConfigured => StatusCode::SERVICE_UNAVAILABLE,
        WahooWebhookError::Settings(_)
        | WahooWebhookError::Import(_)
        | WahooWebhookError::Queue(_)
        | WahooWebhookError::TrainingLoad(_)
        | WahooWebhookError::Wahoo(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
