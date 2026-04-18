use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};

use crate::{config::AppState, domain::training_load::TrainingLoadDashboardRange};

use super::{dto::TrainingLoadDashboardQuery, mapping::map_dashboard_report_to_dto};

pub(crate) async fn get_training_load_dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TrainingLoadDashboardQuery>,
) -> Response {
    let user_id = match super::super::user_auth::resolve_user_id(&state, &headers).await {
        Ok(user_id) => user_id,
        Err(response) => return response,
    };
    let service = match state.training_load_dashboard_service.as_deref() {
        Some(service) => service,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let range = match parse_range(&query.range) {
        Some(range) => range,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let today = DateTime::<Utc>::from_timestamp(Utc::now().timestamp(), 0)
        .map(|value| value.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| {
            DateTime::<Utc>::UNIX_EPOCH
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        });

    match service.build_report(&user_id, range, &today).await {
        Ok(report) => Json(map_dashboard_report_to_dto(report)).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn parse_range(value: &str) -> Option<TrainingLoadDashboardRange> {
    match value {
        "90d" => Some(TrainingLoadDashboardRange::Last90Days),
        "season" => Some(TrainingLoadDashboardRange::Season),
        "all-time" => Some(TrainingLoadDashboardRange::AllTime),
        _ => None,
    }
}
